import { useState } from "react";
import { useTheme } from "@rumahl/ui";
import { themes } from "@rumahl/ui/themes";
import type { MessageKey } from "../i18n/locales/en";
import { useShellPreferences } from "../preferences/ShellPreferences";
import { useThemeTuning } from "../preferences/theme-tuning";
import { useI18n } from "../i18n";
import { setTestMedia } from "../shell/test-media";
import { setGlassAdjust } from "../glass-engine/useGlassEngine";

export function DisplaySettings() {
  const { t } = useI18n();
  const settings = useShellPreferences();
  const { scope, setScope } = settings;
  const selected = settings.preferences?.[scope].shellMode;
  const selectedTheme = settings.preferences?.[scope].shellTheme;
  const effectiveTheme = themes.find((item) => item.id === settings.theme);
  const { tokens, theme: activeTheme } = useTheme();
  const { tuning, setSeed, setMode, setToken, setTransparency, setAutoColor, setWallpaperTint, setWallpaperMotion, setGlassEnabled, setGlassBackend, setGlassQuality, setMaterialPreset, reset } = useThemeTuning();
  const [refraction, setRefraction] = useState(56);
  const [chromaPercent, setChromaPercent] = useState(28);
  const ranges = activeTheme.parameters.filter((parameter) => parameter.kind === "range");
  const choices = activeTheme.parameters.filter((parameter) => parameter.kind !== "range");
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
      return <label key={parameter.id} className="theme-parameter">
        <span>{label}</span>
        <input type="color" aria-label={label} value={toHex(current)} onChange={(event) => assign(event.target.value)} />
        {parameter.presets ? <span className="theme-swatches">{parameter.presets.map((preset) => (
          <button key={preset.id} type="button" className={`theme-swatch swatch theme-swatch--${preset.id}`}
            aria-label={t(preset.labelKey as MessageKey)} title={t(preset.labelKey as MessageKey)}
            aria-pressed={toHex(current).toLowerCase() === preset.value.toLowerCase()} onClick={() => assign(preset.value)} />
        ))}</span> : null}
        <output>{toHex(current)}</output>
      </label>;
    }
    return <label key={parameter.id} className="theme-parameter">
      <span>{label}</span>
      <select className="select" aria-label={label} value={current} onChange={(event) => setToken(parameter.token, event.target.value)}>
        {parameter.options.map((option) => <option key={option.value} value={option.value}>{t(option.labelKey as MessageKey)}</option>)}
      </select>
    </label>;
  };
  return <section className="display-settings">
    <header className="settings-page-heading">
      <div className="heading-flex"><h1 className="pagetitle">{t("mode.label")}</h1><span className="badge">{t("nav.settings")}</span></div>
      <p className="pagedesc">{t("settings.displayHelp")}</p>
    </header>

    <h3 className="section-title">{t("theme.title")}</h3>
    <div className="settings-group">
      <div className="setting">
        <div className="copy"><strong>{t("preferences.scope")}</strong><p>{t("preferences.precedence")}</p></div>
        <div className="visual"><select aria-label={t("preferences.scope")} value={scope} onChange={(event) => setScope(event.target.value === "device" ? "device" : "user")}>
          <option value="user">{t("preferences.user")}</option><option value="device">{t("preferences.device")}</option>
        </select></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("mode.label")}</strong><p>{t("settings.displayHelp")}</p></div>
        <div className="visual"><div className="segmented" role="group" aria-label={t("mode.label")}>
          <button className="segment" type="button" aria-pressed={selected === "desktop"} onClick={() => settings.save(scope, "desktop")}>{t("mode.desktop")}</button>
          <button className="segment" type="button" aria-pressed={selected === "launcher"} onClick={() => settings.save(scope, "launcher")}>{t("mode.launcher")}</button>
          {scope === "device" ? <button className="segment" type="button" aria-pressed={selected === null} onClick={() => settings.save("device", null)}>{t("preferences.inherit")}</button> : null}
        </div></div>
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
          {(["clear", "soft", "solid"] as const).map(preset => (
            <button type="button" key={preset} className={`material-card material-card--${preset}`} onClick={() => setMaterialPreset(preset)}>
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
        <div className="visual"><input type="checkbox" role="switch" aria-label={t("theme.transparencyEffects")} checked={Number(tokens["material.opacity"]) < 1 && Number(tokens["material.morphism"]) > 0} onChange={event => setTransparency(event.target.checked)} /></div>
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
    <div className="settings-group">
      <div className="setting">
        <div className="copy"><strong>{t("theme.glass.enabled")}</strong><p>{t("theme.glass.enabledHelp")}</p></div>
        <div className="visual"><input type="checkbox" role="switch" aria-label={t("theme.glass.enabled")} checked={tuning.glassEnabled !== false} onChange={event => setGlassEnabled(event.target.checked)} /></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.glass.backend")}</strong><p>{t("theme.glass.backendHelp")}</p></div>
        <div className="visual"><select className="select" aria-label={t("theme.glass.backend")} value={tuning.glassBackend ?? "auto"} onChange={event => setGlassBackend(event.target.value as "auto" | "svg" | "webgl" | "css")}>
          {(["auto", "svg", "webgl", "css"] as const).map(backend => <option key={backend} value={backend}>{t(`theme.glass.backend.${backend}`)}</option>)}
        </select></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.glass.quality")}</strong><p>{t("theme.glass.qualityHelp")}</p></div>
        <div className="visual"><select className="select" aria-label={t("theme.glass.quality")} value={tuning.glassQuality ?? "auto"} onChange={event => setGlassQuality(event.target.value as "auto" | "high" | "balanced" | "low")}>
          {(["auto", "high", "balanced", "low"] as const).map(quality => <option key={quality} value={quality}>{t(`theme.glass.quality.${quality}`)}</option>)}
        </select></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.glass.refraction")}</strong><p>{t("theme.glass.refractionHelp")}</p></div>
        <div className="visual"><input className="range" type="range" aria-label={t("theme.glass.refraction")} min={0} max={100} value={refraction} onChange={event => { const value = Number(event.target.value); setRefraction(value); setGlassAdjust({ refraction: value }); }} /></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("theme.glass.chroma")}</strong><p>{t("theme.glass.chromaHelp")}</p></div>
        <div className="visual"><input className="range" type="range" aria-label={t("theme.glass.chroma")} min={0} max={60} value={chromaPercent} onChange={event => { const value = Number(event.target.value); setChromaPercent(value); setGlassAdjust({ chroma: value / 100 }); }} /></div>
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
        <div className="visual"><button type="button" className="segment" onClick={() => setTestMedia({ image: null, video: null })}>{t("theme.glass.testReset")}</button></div>
      </div>
    </div>

    {activeTheme.parameters.length > 0 ? <>
      <h3 className="section-title">{t("theme.customize")}</h3>
      <div className="settings-group">
        {choices.map(renderParameter)}
        {ranges.length > 0 ? <details className="theme-custom-variant">
          <summary>{t("theme.customVariant")}</summary>
          <p className="theme-custom-variant__help">{t("theme.customVariantHelp")}</p>
          <div className="theme-range-params">{ranges.map(renderParameter)}</div>
        </details> : null}
        <div className="setting"><div className="copy" /><div className="visual"><button type="button" className="theme-reset" onClick={reset}>{t("theme.reset")}</button></div></div>
      </div>
    </> : null}

    {activeTheme.modes.includes("dark") ? <>
      <h3 className="section-title">{t("theme.parameter.mode")}</h3>
      <div className="settings-group">
        <div className="setting">
          <div className="copy"><strong>{t("theme.parameter.mode")}</strong></div>
          <div className="visual"><div className="segmented" role="group" aria-label={t("theme.parameter.mode")}>
            <button className="segment" type="button" aria-pressed={(tuning.mode ?? "light") === "light"} onClick={() => setMode("light")}>{t("theme.mode.light")}</button>
            <button className="segment" type="button" aria-pressed={tuning.mode === "dark"} onClick={() => setMode("dark")}>{t("theme.mode.dark")}</button>
          </div></div>
        </div>
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
