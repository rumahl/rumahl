import { RumahlButtonGroup } from "../components/RumahlButtonGroup";
import { useEffect, useState } from "react";
import { useTheme } from "@rumahl/ui";
import { themes } from "@rumahl/ui/themes";
import type { ColorParameter } from "@rumahl/ui/themes";
import type { MessageKey } from "../i18n/locales/en";
import { useShellPreferences } from "../preferences/ShellPreferences";
import { useThemeTuning } from "../preferences/theme-tuning";
import { useI18n } from "../i18n";
import { setTestMedia } from "../shell/test-media";
import { RumahlSelect } from "../components/RumahlSelect";
import { RumahlColorPicker } from "../components/RumahlColorPicker";
import { Button } from "../components/Button";
import { SettingsHeading } from "../components/SettingsHeading";
import { setGlassAdjust, svgRefractionSupported, webglSupported } from "../glass-engine/useGlassEngine";

/** Resting/middle value of the glass adjustment sliders (a soft detent). */
const REFRACTION_DEFAULT = 50;
const CHROMA_DEFAULT = 30;
const TINT_DEFAULT = 50;
/** Tint is a milky rgba overlay; the slider scales its alpha around the default. */
const TINT_MAX_ALPHA = 0.1;
const tintColor = (percent: number): string => `rgba(30,37,66,${((percent / 100) * TINT_MAX_ALPHA).toFixed(3)})`;

/** Magnetic snap: within `threshold` of the target, pull back to it. */
function withSnap(value: number, target: number, threshold: number): number {
  return Math.abs(value - target) <= threshold ? target : value;
}

export function DisplaySettings() {
  const { t } = useI18n();
  const settings = useShellPreferences();
  const { scope, setScope } = settings;
  const selected = settings.preferences?.[scope].shellMode;
  const selectedTheme = settings.preferences?.[scope].shellTheme;
  const effectiveTheme = themes.find((item) => item.id === settings.theme);
  const { tokens, theme: activeTheme } = useTheme();
  const { tuning, setSeed, setMode, setToken, clearToken, autoColors, setTransparency, setAutoColor, setWallpaperTint, setWallpaperMotion, setGlassEnabled, setGlassBackend, setGlassQuality, setGlassReduced, setAnimations, setPerformanceMode, setMaterialPreset, reset } = useThemeTuning();
  const [refraction, setRefraction] = useState(REFRACTION_DEFAULT);
  const [chromaPercent, setChromaPercent] = useState(CHROMA_DEFAULT);
  const [tintPercent, setTintPercent] = useState(TINT_DEFAULT);
  // Capabilities are browser-only, so resolve them after mount (hydration-safe).
  const [capabilities, setCapabilities] = useState({ svg: true, webgl: true });
  useEffect(() => { setCapabilities({ svg: svgRefractionSupported(), webgl: webglSupported() }); }, []);
  const backendWarning: Partial<Record<"svg" | "webgl", string>> = {
    ...(capabilities.svg ? {} : { svg: t("theme.glass.backend.svgUnsupported") }),
    ...(capabilities.webgl ? {} : { webgl: t("theme.glass.backend.webglUnsupported") }),
  };
  const performanceMode = tuning.performanceMode === true;
  const transparencyOff = performanceMode || Number(tokens["material.opacity"]) >= 1 || Number(tokens["material.morphism"]) <= 0;
  const reduced = tuning.glassReduced === true || performanceMode;
  const presetValues = { clear: [0.72, 12, 1.3, 1], soft: [0.9, 16, 1.1, 0.65], bold: [0.62, 22, 1.45, 1], solid: [1, 0, 1, 0] } as const;
  const closeTo = (value: string | undefined, target: number) => Math.abs((Number.parseFloat(String(value)) || 0) - target) < 0.01;
  const activePreset = (["clear", "soft", "bold", "solid"] as const).find((preset) => {
    const [opacity, blur, saturation, morphism] = presetValues[preset];
    return closeTo(tokens["material.opacity"], opacity) && closeTo(tokens["material.blur"], blur) && closeTo(tokens["material.saturation"], saturation) && closeTo(tokens["material.morphism"], morphism);
  });
  const ranges = activeTheme.parameters.filter((parameter) => parameter.kind === "range");
  const choices = activeTheme.parameters.filter((parameter) => parameter.kind !== "range");
  const seedParams = choices.filter((parameter) => parameter.kind === "choice" || (parameter.kind === "color" && parameter.seed));
  const overrideColors = choices.filter((parameter): parameter is ColorParameter => parameter.kind === "color" && !parameter.seed);
  const overridesActive = overrideColors.some((parameter) => tuning.tokens[parameter.token] !== undefined) || tuning.mode !== null;
  const renderParameter = (parameter: (typeof activeTheme.parameters)[number]) => {
    const label = t(parameter.labelKey as MessageKey);
    const current = tokens[parameter.token];
    if (parameter.kind === "range") {
      const value = parameter.parse(current);
      return <label key={parameter.id} className="theme-parameter">
        <span>{label}</span>
        <input className="range" type="range" aria-label={label} min={parameter.min} max={parameter.max} step={parameter.step} value={value}
          onChange={(event) => setToken(parameter.token, parameter.format(Number(event.target.value)))} />
        <output>{parameter.format(value)}</output>
      </label>;
    }
    if (parameter.kind === "color") {
      const assign = (value: string) => parameter.seed ? setSeed(value) : setToken(parameter.token, value);
      return <div key={parameter.id} className="theme-parameter theme-parameter--color">
        <span>{label}</span>
        <RumahlColorPicker label={label} value={toHex(current)} onChange={assign}
          presets={parameter.presets?.map((preset) => ({ value: preset.value, label: t(preset.labelKey as MessageKey) }))} />
      </div>;
    }
    return <label key={parameter.id} className="theme-parameter">
      <span>{label}</span>
      <RumahlSelect label={label} value={current} onChange={value => setToken(parameter.token, value)}
        options={parameter.options.map(option => ({ value: option.value, label: t(option.labelKey as MessageKey) }))} />
    </label>;
  };
  const renderOverride = (parameter: ColorParameter) => {
    const label = t(parameter.labelKey as MessageKey);
    const overridden = tuning.tokens[parameter.token] !== undefined;
    return <div key={parameter.id} className="theme-parameter theme-parameter--color">
      <span>{label}<em className="theme-parameter__auto">{overridden ? t("theme.customized") : t("theme.auto")}</em></span>
      <RumahlColorPicker label={label} value={toHex(tokens[parameter.token])} onChange={(value) => setToken(parameter.token, value)}
        presets={parameter.presets?.map((preset) => ({ value: preset.value, label: t(preset.labelKey as MessageKey) }))} />
      {overridden ? <Button size="sm" variant="ghost" onClick={() => clearToken(parameter.token)}>{t("theme.useAuto")}</Button> : null}
    </div>;
  };
  return <section className="display-settings">
    <SettingsHeading title={t("settings.personalization")} help={t("settings.displayHelp")} back="/settings" />

    <h3 className="section-title">{t("theme.title")}</h3>
    <div className="settings-group">
      <div className="setting">
        <div className="copy"><strong>{t("preferences.scope")}</strong><p>{t("preferences.precedence")}</p></div>
        <div className="visual"><RumahlSelect label={t("preferences.scope")} value={scope} onChange={value => setScope(value === "device" ? "device" : "user")}
          options={[{ value: "user", label: t("preferences.user") }, { value: "device", label: t("preferences.device") }]} /></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("mode.label")}</strong><p>{t("settings.displayHelp")}</p></div>
        <div className="visual"><RumahlButtonGroup<"desktop" | "launcher" | "inherit"> label={t("mode.label")} value={selected ?? "inherit"}
          onChange={value => settings.save(scope, value === "inherit" ? null : value)}
          options={[{ value: "desktop", label: t("mode.desktop") }, { value: "launcher", label: t("mode.launcher") },
            ...(scope === "device" ? [{ value: "inherit" as const, label: t("preferences.inherit") }] : [])]} /></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.title")}</strong><p>{t("theme.help")}</p></div>
        <div className="visual"><div className="segmented" role="group" aria-label={t("theme.title")}>
          {themes.map((theme) => <button key={theme.id} className="segment" type="button" aria-pressed={selectedTheme === theme.id} onClick={() => settings.saveTheme(scope, theme.id)}>{theme.name}</button>)}
          {scope === "device" ? <button className="segment" type="button" aria-pressed={selectedTheme === null} onClick={() => settings.saveTheme("device", null)}>{t("preferences.inherit")}</button> : null}
        </div></div>
      </div>
    </div>

    <h3 className="section-title">{t("theme.materials")}</h3>
    <div className="settings-group">
      <div className="setting">
        <div className="copy"><strong>{t("theme.presets")}</strong></div>
        <div className="visual material-presets" role="group" aria-label={t("theme.presets")}>
          {(["clear", "soft", "bold", "solid"] as const).map(preset => (
            <button type="button" key={preset} className={`material-card material-card--${preset}`} aria-pressed={activePreset === preset} onClick={() => setMaterialPreset(preset)}>
              <span className="material-card__preview" aria-hidden="true" />
              <span className="material-card__label">{t(`theme.preset.${preset}`)}</span>
            </button>
          ))}
        </div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.autoColor")}</strong><p>{t("theme.autoColorHelp")}</p></div>
        <div className="visual"><input type="checkbox" role="switch" aria-label={t("theme.autoColor")} checked={tuning.autoColor === true} onChange={event => setAutoColor(event.target.checked)} /></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.transparencyEffects")}</strong><p>{t("theme.transparencyHelp")}</p></div>
        <div className="visual"><input type="checkbox" role="switch" aria-label={t("theme.transparencyEffects")} checked={!transparencyOff} disabled={performanceMode} onChange={event => setTransparency(event.target.checked)} /></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.wallpaperTint")}</strong><p>{t("theme.wallpaperTintHelp")}</p></div>
        <div className="visual"><input type="checkbox" role="switch" aria-label={t("theme.wallpaperTint")} checked={tuning.wallpaperTint !== false} onChange={event => setWallpaperTint(event.target.checked)} /></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.wallpaperMotion")}</strong><p>{t("theme.wallpaperMotionHelp")}</p></div>
        <div className="visual"><input type="checkbox" role="switch" aria-label={t("theme.wallpaperMotion")} checked={tuning.wallpaperMotion !== false} onChange={event => setWallpaperMotion(event.target.checked)} /></div>
      </div>
    </div>

    <h3 className="section-title">{t("theme.glass.title")}</h3>
    <div className="settings-group" aria-disabled={performanceMode}>
      {reduced ? <div className="setting glass-banner" role="status">
        <div className="copy"><strong>{t("theme.glass.reducedBanner")}</strong></div>
        <div className="visual"><Button variant="primary" size="sm" onClick={() => setGlassReduced(false)}>{t("theme.glass.undo")}</Button></div>
      </div> : null}
      <div className="setting">
        <div className="copy"><strong>{t("theme.glass.enabled")}</strong><p>{t("theme.glass.enabledHelp")}</p></div>
        <div className="visual"><input type="checkbox" role="switch" aria-label={t("theme.glass.enabled")} disabled={performanceMode} checked={tuning.glassEnabled !== false && !transparencyOff && !reduced} onChange={event => setGlassEnabled(event.target.checked)} /></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.glass.backend")}</strong><p>{t("theme.glass.backendHelp")}</p></div>
        <div className="visual"><RumahlSelect label={t("theme.glass.backend")} disabled={reduced || transparencyOff} value={tuning.glassBackend ?? "auto"} onChange={value => setGlassBackend(value as "auto" | "svg" | "webgl" | "css")}
          options={(["auto", "svg", "webgl", "css"] as const).map(backend => {
            const warning = backend === "svg" || backend === "webgl" ? backendWarning[backend] : undefined;
            return { value: backend, label: t(`theme.glass.backend.${backend}`), disabled: warning !== undefined, warning };
          })} /></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.glass.quality")}</strong><p>{t("theme.glass.qualityHelp")}</p></div>
        <div className="visual"><RumahlSelect label={t("theme.glass.quality")} disabled={reduced || transparencyOff} value={tuning.glassQuality ?? "auto"} onChange={value => setGlassQuality(value as "auto" | "high" | "balanced" | "low")}
          options={(["auto", "high", "balanced", "low"] as const).map(quality => ({ value: quality, label: t(`theme.glass.quality.${quality}`) }))} /></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.glass.refraction")}</strong><p>{t("theme.glass.refractionHelp")}</p></div>
        <div className="visual visual--range"><input className="range" type="range" aria-label={t("theme.glass.refraction")} disabled={reduced || transparencyOff} min={0} max={100} value={refraction} onChange={event => { const value = withSnap(Number(event.target.value), REFRACTION_DEFAULT, 4); setRefraction(value); setGlassAdjust({ refraction: value }); }} /><output>{refraction}%</output></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.glass.chroma")}</strong><p>{t("theme.glass.chromaHelp")}</p></div>
        <div className="visual visual--range"><input className="range" type="range" aria-label={t("theme.glass.chroma")} disabled={reduced || transparencyOff} min={0} max={60} value={chromaPercent} onChange={event => { const value = withSnap(Number(event.target.value), CHROMA_DEFAULT, 2); setChromaPercent(value); setGlassAdjust({ chroma: value / 100 }); }} /><output>{chromaPercent}%</output></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.glass.tint")}</strong><p>{t("theme.glass.tintHelp")}</p></div>
        <div className="visual visual--range"><input className="range" type="range" aria-label={t("theme.glass.tint")} disabled={reduced || transparencyOff} min={0} max={100} value={tintPercent} onChange={event => { const value = withSnap(Number(event.target.value), TINT_DEFAULT, 4); setTintPercent(value); setGlassAdjust({ tint: tintColor(value) }); }} /><output>{tintPercent}%</output></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.glass.testImage")}</strong><p>{t("theme.glass.testImageHelp")}</p></div>
        <div className="visual"><input type="file" accept="image/*" aria-label={t("theme.glass.testImage")} onChange={event => { const file = event.target.files?.[0]; if (file) setTestMedia({ image: URL.createObjectURL(file), video: null }); }} /></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.glass.testVideo")}</strong><p>{t("theme.glass.testVideoHelp")}</p></div>
        <div className="visual"><input type="file" accept="video/*" aria-label={t("theme.glass.testVideo")} onChange={event => { const file = event.target.files?.[0]; if (file) setTestMedia({ video: URL.createObjectURL(file), image: null }); }} /></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.glass.testReset")}</strong><p>{t("theme.glass.testResetHelp")}</p></div>
        <div className="visual"><Button size="sm" onClick={() => setTestMedia({ image: null, video: null })}>{t("theme.glass.testReset")}</Button></div>
      </div>
    </div>

    <h3 className="section-title">{t("theme.performance.title")}</h3>
    <div className="settings-group">
      <div className="setting">
        <div className="copy"><strong>{t("theme.performance.mode")}</strong><p>{t("theme.performance.modeHelp")}</p></div>
        <div className="visual"><input type="checkbox" role="switch" aria-label={t("theme.performance.mode")} checked={tuning.performanceMode === true} onChange={event => setPerformanceMode(event.target.checked)} /></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.performance.animations")}</strong><p>{t("theme.performance.animationsHelp")}</p></div>
        <div className="visual"><input type="checkbox" role="switch" aria-label={t("theme.performance.animations")} disabled={performanceMode} checked={tuning.animations !== false} onChange={event => setAnimations(event.target.checked)} /></div>
      </div>
    </div>

    {activeTheme.parameters.length > 0 ? <>
      <h3 className="section-title">{t("theme.customize")}</h3>
      <div className="settings-group">
        {seedParams.map(renderParameter)}
        {ranges.length > 0 ? <details className="theme-custom-variant">
          <summary>{t("theme.customVariant")}</summary>
          <p className="theme-custom-variant__help">{t("theme.customVariantHelp")}</p>
          <div className="theme-range-params">{ranges.map(renderParameter)}</div>
        </details> : null}
        <div className="setting"><div className="copy" /><div className="visual"><Button variant="danger" onClick={reset}>{t("theme.reset")}</Button></div></div>
      </div>
    </> : null}

    {overrideColors.length > 0 || activeTheme.modes.includes("dark") ? <>
      <h3 className="section-title">{t("theme.overrides")}</h3>
      <div className="settings-group">
        <details className="theme-custom-variant">
          <summary>{t("theme.overrides")}<span className="theme-custom-variant__state">{overridesActive ? t("theme.customized") : t("theme.auto")}</span></summary>
          <p className="theme-custom-variant__help">{t("theme.overridesHelp")}</p>
          {activeTheme.modes.includes("dark") ? <div className="theme-parameter">
            <span>{t("theme.parameter.mode")}</span>
            <div className="segmented" role="group" aria-label={t("theme.parameter.mode")}>
              <button className="segment" type="button" aria-pressed={tuning.mode === null} onClick={() => setMode(null)}>{t("theme.mode.auto")}</button>
              <button className="segment" type="button" aria-pressed={tuning.mode === "light"} onClick={() => setMode("light")}>{t("theme.mode.light")}</button>
              <button className="segment" type="button" aria-pressed={tuning.mode === "dark"} onClick={() => setMode("dark")}>{t("theme.mode.dark")}</button>
            </div>
          </div> : null}
          {overrideColors.map(renderOverride)}
          <div className="setting"><div className="copy" /><div className="visual"><Button onClick={autoColors}>{t("theme.resetAuto")}</Button></div></div>
        </details>
      </div>
    </> : null}

    <p>{t("preferences.effective")}: {t(`mode.${settings.mode}`)} · {effectiveTheme?.name ?? settings.theme}</p>
    <p role="status">{settings.saving ? t("preferences.saving") : settings.error ? t(`preferences.${settings.error}`) : settings.ready ? t("preferences.synced") : t("preferences.loading")}</p>
    {settings.persistence !== "local" ? <p>{t("preferences.temporary")}</p> : null}
  </section>;
}

/** Normalizes a token colour to the `#rrggbb` form an `<input type="color">` needs. */
function toHex(value: string): string {
  const match = /^#([0-9a-fA-F]{6})/.exec(value);
  return match ? `#${match[1]}` : "#000000";
}
