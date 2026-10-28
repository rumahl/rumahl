import { RumahlButtonGroup } from "../components/RumahlButtonGroup";
import { useEffect, useRef, useState } from "react";
import { useTheme } from "@rumahl/ui";
import type { ColorParameter } from "@rumahl/ui/themes";
import {
  installThemeFile,
  useThemeRegistry
} from "../preferences/installed-themes";
import { ThemeSafetyError } from "../preferences/theme-safety";
import type { MessageKey } from "../i18n/locales/en";
import { useShellPreferences } from "../preferences/ShellPreferences";
import { useThemeTuning } from "../preferences/theme-tuning";
import { appearancePresets, isLookActive } from "../preferences/appearance-presets";
import { useI18n } from "../i18n";
import { useDeviceViewport } from "../shell/device";
import { useAdaptiveTypeScale } from "../preferences/adaptive-type";
import { setTestMedia } from "../shell/test-media";
import { RumahlSelect } from "../components/RumahlSelect";
import { RumahlColorPicker } from "../components/RumahlColorPicker";
import { Button } from "../components/Button";
import { SettingsHeading } from "../components/SettingsHeading";
import { dismissToast, showToast } from "../shell/toasts";
import {
  setGlassAdjust,
  svgRefractionSupported,
  webglSupported,
} from "../glass-engine/useGlassEngine";

/** Resting/middle value of the glass adjustment sliders (a soft detent). */
const REFRACTION_DEFAULT = 50;
const CHROMA_DEFAULT = 30;
/** Saturation is a multiplier where 100% is neutral (no background tint). */
const SATURATION_DEFAULT = 100;

/** Magnetic snap: within `threshold` of the target, pull back to it. */
function withSnap(value: number, target: number, threshold: number): number {
  return Math.abs(value - target) <= threshold ? target : value;
}

function AppearanceChoice({ label, moreLabel, options, value, onChange }: {
  label: string;
  moreLabel: string;
  options: readonly { value: string; label: string }[];
  value: string;
  onChange: (value: string) => void;
}) {
  const visible = options.length > 3 ? options.slice(0, 3) : options;
  const overflow = options.length > 3 ? options.slice(3) : [];
  const selectedOverflow = overflow.find((option) => option.value === value);
  const [overflowEngaged, setOverflowEngaged] = useState(Boolean(selectedOverflow));
  useEffect(() => {
    setOverflowEngaged(Boolean(selectedOverflow));
  }, [selectedOverflow?.value]);
  return <RumahlButtonGroup className="appearance-choice__group" label={label} value={visible.some((option) => option.value === value) ? value : ""} onChange={onChange} options={visible}
    activeTrailing={Boolean(selectedOverflow) || (overflowEngaged && overflow.length > 0)}
    trailing={overflow.length ? <span className="appearance-choice__overflow" data-active={(selectedOverflow || overflowEngaged) ? "" : undefined}>
      <span className="appearance-choice__more-label" aria-hidden="true">{selectedOverflow?.label ?? moreLabel}</span>
      <select className="appearance-choice__more" aria-label={moreLabel}
        value={selectedOverflow ? value : ""}
        onPointerDown={() => setOverflowEngaged(true)}
        onFocus={() => setOverflowEngaged(true)}
        onBlur={() => setOverflowEngaged(Boolean(selectedOverflow))}
        onChange={(event) => {
          if (event.target.value) onChange(event.target.value);
          setOverflowEngaged(true);
        }}>
        <option value="" disabled hidden>{moreLabel}</option>
        {overflow.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
      </select>
    </span> : undefined} />;
}

export function DisplaySettings() {
  const { t } = useI18n();
  const settings = useShellPreferences();
  const viewport = useDeviceViewport();
  const { scope, setScope } = settings;
  const selected = settings.preferences?.[scope].shellMode;
  const selectedMode = selected === "mobile" ? "launcher" : selected;
  const selectedTheme = settings.preferences?.[scope].shellTheme;
  const themes = useThemeRegistry();
  const effectiveTheme = themes.find((item) => item.id === settings.theme);
  const { tokens, theme: activeTheme } = useTheme();
  const installRef = useRef<HTMLInputElement>(null);
  const [installError, setInstallError] = useState<string | null>(null);
  const [pendingTheme, setPendingTheme] = useState<{ scope: "user" | "device"; value: string | null } | null>(null);
  function chooseTheme(value: string | null) {
    if (settings.preferences?.[scope].shellTheme === value) return;
    settings.saveTheme(scope, value);
    setPendingTheme({ scope, value });
  }
  // Ask only after the preference was persisted, so an immediate reload cannot
  // discard the newly selected theme.
  useEffect(() => {
    if (!pendingTheme || settings.preferences?.[pendingTheme.scope].shellTheme !== pendingTheme.value) return;
    setPendingTheme(null);
    showToast({
      id: "theme-reload",
      title: t("theme.reloadTitle"),
      message: t("theme.reloadBody"),
      variant: "info",
      duration: null,
      buttons: [
        { label: t("reload.now"), variant: "primary", onClick: () => window.location.reload() },
        { label: t("reload.later"), onClick: () => dismissToast("theme-reload") }
      ]
    });
  }, [pendingTheme, settings.preferences, t]);
  const installTheme = async (file: File) => {
    try {
      const theme = await installThemeFile(file);
      chooseTheme(theme.id);
      setInstallError(null);
    } catch (error) {
      setInstallError(
        error instanceof ThemeSafetyError
          ? error.message
          : "This theme could not be installed."
      );
    }
  };
  const {
    tuning,
    applyPreset,
    setSeed,
    setMode,
    setToken,
    clearToken,
    autoColors,
    setTransparency,
    setAutoColor,
    setWallpaperTint,
    setWallpaperMotion,
    setGlassEnabled,
    setGlassColor,
    setGlassTint,
    setGlassBackend,
    setGlassQuality,
    setGlassReduced,
    setAnimations,
    setPerformanceMode,
    setMaterialPreset,
    setAdaptiveType,
    setFontScale,
    reset,
  } = useThemeTuning();
  const [refraction, setRefraction] = useState(REFRACTION_DEFAULT);
  const [chromaPercent, setChromaPercent] = useState(CHROMA_DEFAULT);
  const [saturationPercent, setSaturationPercent] = useState(SATURATION_DEFAULT);
  const [glassColorDraft, setGlassColorDraft] = useState<string | null>(null);
  const typeScale = Number.parseFloat(tuning.tokens["typography.scale"] ?? "1") || 1;
  const fontSizePercent = Math.round(typeScale * 100);
  const adaptiveEnabled = tuning.adaptiveType === true;
  const adaptiveScale = useAdaptiveTypeScale(adaptiveEnabled);
  const adaptivePercent = Math.round(adaptiveScale * 100);
  const [fontSizeDraft, setFontSizeDraft] = useState(String(fontSizePercent));
  // Capabilities are browser-only, so resolve them after mount (hydration-safe).
  const [capabilities, setCapabilities] = useState({ svg: true, webgl: true });
  useEffect(() => {
    setCapabilities({ svg: svgRefractionSupported(), webgl: webglSupported() });
  }, []);
  // The colour picker emits continuously while dragging. Persist once the
  // value settles instead of dropping writes while the workspace is busy.
  useEffect(() => {
    if (glassColorDraft === null) return;
    const timer = window.setTimeout(() => setGlassTint(glassColorDraft), 300);
    return () => window.clearTimeout(timer);
  }, [glassColorDraft, setGlassTint]);
  useEffect(() => {
    if (glassColorDraft !== null && tuning.glassTint === glassColorDraft) setGlassColorDraft(null);
  }, [glassColorDraft, tuning.glassTint]);
  useEffect(() => { setFontSizeDraft(String(fontSizePercent)); }, [fontSizePercent]);
  const applyFontSize = (raw: string) => {
    const parsed = Number.parseFloat(raw);
    const value = Number.isFinite(parsed) ? Math.min(200, Math.max(75, Math.round(parsed / 5) * 5)) : 100;
    setFontSizeDraft(String(value));
    setFontScale(value / 100);
  };
  const backendWarning: Partial<Record<"svg" | "webgl", string>> = {
    ...(capabilities.svg
      ? {}
      : { svg: t("theme.glass.backend.svgUnsupported") }),
    ...(capabilities.webgl
      ? {}
      : { webgl: t("theme.glass.backend.webglUnsupported") }),
  };
  const performanceMode = tuning.performanceMode === true;
  const transparencyOff =
    performanceMode ||
    Number(tokens["material.opacity"]) >= 1 ||
    Number(tokens["material.morphism"]) <= 0;
  const reduced = tuning.glassReduced === true || performanceMode;
  const presetValues = {
    clear: [0.72, 12, 1.3, 1],
    soft: [0.9, 16, 1.1, 0.65],
    bold: [0.62, 22, 1.45, 1],
    solid: [1, 0, 1, 0],
  } as const;
  const closeTo = (value: string | undefined, target: number) =>
    Math.abs((Number.parseFloat(String(value)) || 0) - target) < 0.01;
  const activePreset = (["clear", "soft", "bold", "solid"] as const).find(
    (preset) => {
      const [opacity, blur, saturation, morphism] = presetValues[preset];
      return (
        closeTo(tokens["material.opacity"], opacity) &&
        closeTo(tokens["material.blur"], blur) &&
        closeTo(tokens["material.saturation"], saturation) &&
        closeTo(tokens["material.morphism"], morphism)
      );
    },
  );
  const ranges = activeTheme.parameters.filter(
    (parameter) => parameter.kind === "range",
  );
  const choices = activeTheme.parameters.filter(
    (parameter) => parameter.kind !== "range",
  );
  const seedParams = choices.filter(
    (parameter) =>
      parameter.kind === "choice" ||
      (parameter.kind === "color" && parameter.seed),
  );
  const overrideColors = choices.filter(
    (parameter): parameter is ColorParameter =>
      parameter.kind === "color" && !parameter.seed,
  );
  const overridesActive =
    overrideColors.some(
      (parameter) => tuning.tokens[parameter.token] !== undefined,
    ) || tuning.mode !== null;
  const renderParameter = (
    parameter: (typeof activeTheme.parameters)[number],
  ) => {
    const label = t(parameter.labelKey as MessageKey);
    const current = tokens[parameter.token];
    if (parameter.kind === "range") {
      const value = parameter.parse(current);
      return (
        <label key={parameter.id} className="theme-parameter">
          <span>{label}</span>
          <input
            className="range"
            type="range"
            aria-label={label}
            min={parameter.min}
            max={parameter.max}
            step={parameter.step}
            value={value}
            onChange={(event) =>
              setToken(
                parameter.token,
                parameter.format(Number(event.target.value)),
              )
            }
          />
          <output>{parameter.format(value)}</output>
        </label>
      );
    }
    if (parameter.kind === "color") {
      const assign = (value: string) =>
        parameter.seed ? setSeed(value) : setToken(parameter.token, value);
      return (
        <div
          key={parameter.id}
          className="theme-parameter theme-parameter--color"
        >
          <span>{label}</span>
          <RumahlColorPicker
            label={label}
            value={toHex(current)}
            onChange={assign}
            presets={parameter.presets?.map((preset) => ({
              value: preset.value,
              label: t(preset.labelKey as MessageKey),
            }))}
          />
        </div>
      );
    }
    return (
      <label key={parameter.id} className="theme-parameter">
        <span>{label}</span>
        <RumahlSelect
          label={label}
          value={current}
          onChange={(value) => setToken(parameter.token, value)}
          options={parameter.options.map((option) => ({
            value: option.value,
            label: t(option.labelKey as MessageKey),
            // Typography pickers preview each face in its own font.
            font:
              parameter.token === "typography.family" ||
              parameter.token === "typography.display"
                ? option.value
                : undefined,
          }))}
        />
      </label>
    );
  };
  const renderOverride = (parameter: ColorParameter) => {
    const label = t(parameter.labelKey as MessageKey);
    const overridden = tuning.tokens[parameter.token] !== undefined;
    return (
      <div
        key={parameter.id}
        className="theme-parameter theme-parameter--color"
      >
        <span>
          {label}
          <em className="theme-parameter__auto">
            {overridden ? t("theme.customized") : t("theme.auto")}
          </em>
        </span>
        <RumahlColorPicker
          label={label}
          value={toHex(tokens[parameter.token])}
          onChange={(value) => setToken(parameter.token, value)}
          presets={parameter.presets?.map((preset) => ({
            value: preset.value,
            label: t(preset.labelKey as MessageKey),
          }))}
        />
        {overridden ? (
          <Button
            size="sm"
            variant="ghost"
            onClick={() => clearToken(parameter.token)}
          >
            {t("theme.useAuto")}
          </Button>
        ) : null}
      </div>
    );
  };
  const activeLookPreset = appearancePresets.find((preset) => isLookActive(tuning, preset.appearance));
  const supportsLookPresets = activeTheme.presets?.look === true;
  const supportsMaterialPresets = activeTheme.presets?.material === true;
  return (
    <section className="display-settings">
      <SettingsHeading
        title={t("settings.personalization")}
        help={t("settings.displayHelp")}
        back="/settings"
      />

      <h3 className="section-title">{t("theme.title")}</h3>
      <div className="settings-group">
        <div className="setting">
          <div className="copy">
            <strong>{t("preferences.scope")}</strong>
            <p>{t("preferences.precedence")}</p>
          </div>
          <div className="visual">
            <RumahlSelect
              label={t("preferences.scope")}
              value={scope}
              onChange={(value) =>
                setScope(value === "device" ? "device" : "user")
              }
              options={[
                { value: "user", label: t("preferences.user") },
                { value: "device", label: t("preferences.device") },
              ]}
            />
          </div>
        </div>
        <div className="setting">
          <div className="copy">
            <strong>{t("mode.label")}</strong>
            <p>{t("settings.shellModeHelp")}</p>
          </div>
          <div className="visual">
            {viewport.phone
              ? <span>{t("mode.mobile")}</span>
              : <RumahlButtonGroup<"desktop" | "launcher" | "inherit">
                  label={t("mode.label")}
                  value={selectedMode ?? "inherit"}
                  onChange={(value) =>
                    settings.save(scope, value === "inherit" ? null : value)
                  }
                  options={[
                    { value: "desktop", label: t("mode.desktop") },
                    { value: "launcher", label: t("mode.launcher") },
                    ...(scope === "device"
                      ? [
                          {
                            value: "inherit" as const,
                            label: t("preferences.inherit"),
                          },
                        ]
                      : []),
                  ]}
                />}
          </div>
        </div>
        <div className="setting">
          <div className="copy">
            <strong>{t("theme.title")}</strong>
            <p>{t("theme.help")}</p>
          </div>
          <div className="visual">
            <AppearanceChoice
              label={t("theme.title")}
              moreLabel={t("theme.more")}
              value={selectedTheme ?? "__inherit"}
              onChange={(value) => chooseTheme(value === "__inherit" ? null : value)}
              options={[
                ...themes.map((theme) => ({ value: theme.id, label: theme.name })),
                ...(scope === "device" ? [{ value: "__inherit", label: t("preferences.inherit") }] : [])
              ]}
            />
            <div className="theme-preview" aria-hidden="true">
              <div className="theme-preview__window">
                <i /><i /><i />
              </div>
              <div className="theme-preview__dock"><span /><span /><span /></div>
            </div>
          </div>
        </div>
        <div className="setting">
          <div className="copy">
            <strong>{t("theme.install")}</strong>
            <p>{t("theme.installHelp")}</p>
            {installError ? (
              <p role="alert" style={{ color: "var(--rumahl-ui-color-danger, #e5484d)" }}>
                {installError}
              </p>
            ) : null}
          </div>
          <div className="visual">
            <input
              ref={installRef}
              type="file"
              accept="application/json,.json"
              hidden
              onChange={(event) => {
                const file = event.target.files?.[0];
                event.target.value = "";
                if (file) void installTheme(file);
              }}
            />
            <Button onClick={() => installRef.current?.click()}>
              {t("theme.installAction")}
            </Button>
          </div>
        </div>
      </div>

      <h3 className="section-title">{t("theme.textSize")}</h3>
      <div className="settings-group">
        <div className="setting">
          <div className="copy">
            <strong>{t("theme.textSizePreset")}</strong>
            <p>{t("theme.textSizePresetHelp")}</p>
          </div>
          <div className="visual">
            <RumahlButtonGroup
              label={t("theme.textSizePreset")}
              value={[90, 100, 110, 125, 140].includes(fontSizePercent) ? String(fontSizePercent) : "custom"}
              onChange={(value) => applyFontSize(value)}
              options={[
                { value: "90", label: "90%" },
                { value: "100", label: "100%" },
                { value: "110", label: "110%" },
                { value: "125", label: "125%" },
                { value: "140", label: "140%" }
              ]}
            />
          </div>
        </div>
        <div className="setting">
          <div className="copy">
            <strong>{t("theme.textSizeCustom")}</strong>
            <p>{t("theme.textSizeCustomHelp")}</p>
          </div>
          <div className="visual theme-text-size-custom">
            <input
              type="number"
              min={75}
              max={200}
              step={5}
              value={fontSizeDraft}
              aria-label={t("theme.textSizeCustom")}
              onChange={(event) => setFontSizeDraft(event.target.value)}
              onBlur={(event) => applyFontSize(event.target.value)}
              onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); }}
            />
            <span>%</span>
          </div>
        </div>
        <div className="setting">
          <div className="copy">
            <strong>{t("theme.textSizeAdaptive")}</strong>
            <p>{t("theme.textSizeAdaptiveHelp")}</p>
          </div>
          <div className="visual">
            <Button
              variant={adaptiveEnabled ? "primary" : "secondary"}
              active={adaptiveEnabled}
              aria-pressed={adaptiveEnabled}
              onClick={() => setAdaptiveType(!adaptiveEnabled)}
            >
              {adaptiveEnabled ? t("theme.textSizeAdaptiveActive", { percent: adaptivePercent }) : t("theme.textSizeAdaptive")}
            </Button>
          </div>
        </div>
      </div>

      {supportsLookPresets || supportsMaterialPresets ? (
        <>
          <h3 className="section-title">{t("theme.auraPresets")}</h3>
          <p className="settings-section-help">{t("theme.auraPresetsHelp")}</p>
          <div className="settings-group">
            {supportsLookPresets ? (
              <div className="setting aura-preset-row">
                <div className="copy">
                  <strong>{t("theme.lookPresets")}</strong>
                  <p>{t("theme.lookPresetsHelp")}</p>
                </div>
                <div className="visual">
                  <div className="look-preset-picker">
                    <RumahlButtonGroup
                      label={t("theme.lookPresets")}
                      value={activeLookPreset?.id ?? ""}
                      onChange={(value) => {
                        const preset = appearancePresets.find((item) => item.id === value);
                        if (preset) applyPreset(preset.appearance);
                      }}
                      options={appearancePresets.map((preset) => ({ value: preset.id, label: t(preset.labelKey) }))}
                    />
                    <select
                      className="look-preset-select"
                      aria-label={t("theme.lookPresets")}
                      value={activeLookPreset?.id ?? ""}
                      onChange={(event) => {
                        const preset = appearancePresets.find((item) => item.id === event.target.value);
                        if (preset) applyPreset(preset.appearance);
                      }}
                    >
                      {!activeLookPreset ? <option value="">{t("theme.lookCustom")}</option> : null}
                      {appearancePresets.map((preset) => <option key={preset.id} value={preset.id}>{t(preset.labelKey)}</option>)}
                    </select>
                  </div>
                </div>
              </div>
            ) : null}
            {supportsMaterialPresets && activeTheme.material !== "solid" ? (
              <div className="setting aura-preset-row">
                <div className="copy">
                  <strong>{t("theme.presets")}</strong>
                  <p>{t("theme.presetsHelp")}</p>
                </div>
                <div className="visual aura-material-presets">
                  <div className="material-presets" role="group" aria-label={t("theme.presets")}>
                    {(["clear", "soft", "bold", "solid"] as const).map((preset) => (
                      <button
                        type="button"
                        key={preset}
                        className={`material-card material-card--${preset}`}
                        aria-pressed={activePreset === preset}
                        onClick={() => setMaterialPreset(preset)}
                      >
                        <span className="material-card__preview" aria-hidden="true" />
                        <span className="material-card__label">
                          {t(`theme.preset.${preset}`)}
                        </span>
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            ) : null}
          </div>
        </>
      ) : null}

      <h3 className="section-title">{t("theme.materials")}</h3>
      <div className="settings-group">
        <div className="setting">
          <div className="copy">
            <strong>{t("theme.autoColor")}</strong>
            <p>{t("theme.autoColorHelp")}</p>
          </div>
          <div className="visual">
            <input
              type="checkbox"
              role="switch"
              aria-label={t("theme.autoColor")}
              checked={tuning.autoColor === true}
              onChange={(event) => setAutoColor(event.target.checked)}
            />
          </div>
        </div>
        {activeTheme.material !== "solid" ? (
          <>
            <div className="setting">
              <div className="copy">
                <strong>{t("theme.transparencyEffects")}</strong>
                <p>{t("theme.transparencyHelp")}</p>
              </div>
              <div className="visual">
                <input
                  type="checkbox"
                  role="switch"
                  aria-label={t("theme.transparencyEffects")}
                  checked={!transparencyOff}
                  disabled={performanceMode}
                  onChange={(event) => setTransparency(event.target.checked)}
                />
              </div>
            </div>
          </>
        ) : null}
        {activeTheme.material !== "solid" ? (
          <>
            <div className="setting">
              <div className="copy">
                <strong>{t("theme.wallpaperTint")}</strong>
                <p>{t("theme.wallpaperTintHelp")}</p>
              </div>
              <div className="visual">
                <input
                  type="checkbox"
                  role="switch"
                  aria-label={t("theme.wallpaperTint")}
                  checked={tuning.wallpaperTint !== false}
                  onChange={(event) => setWallpaperTint(event.target.checked)}
                />
              </div>
            </div>
          </>
        ) : null}
        <div className="setting">
          <div className="copy">
            <strong>{t("theme.wallpaperMotion")}</strong>
            <p>{t("theme.wallpaperMotionHelp")}</p>
          </div>
          <div className="visual">
            <input
              type="checkbox"
              role="switch"
              aria-label={t("theme.wallpaperMotion")}
              checked={tuning.wallpaperMotion !== false}
              onChange={(event) => setWallpaperMotion(event.target.checked)}
            />
          </div>
        </div>
      </div>

      {activeTheme.material !== "solid" ? (
        <>
          <h3 className="section-title">{t("theme.glass.title")}</h3>
          <div className="settings-group" aria-disabled={performanceMode}>
            {reduced ? (
              <div className="setting glass-banner" role="status">
                <div className="copy">
                  <strong>{t("theme.glass.reducedBanner")}</strong>
                </div>
                <div className="visual">
                  <Button
                    variant="primary"
                    size="sm"
                    onClick={() => setGlassReduced(false)}
                  >
                    {t("theme.glass.undo")}
                  </Button>
                </div>
              </div>
            ) : null}
            <div className="setting">
              <div className="copy">
                <strong>{t("theme.glass.enabled")}</strong>
                <p>{t("theme.glass.enabledHelp")}</p>
              </div>
              <div className="visual">
                <input
                  type="checkbox"
                  role="switch"
                  aria-label={t("theme.glass.enabled")}
                  disabled={performanceMode}
                  checked={
                    tuning.glassEnabled !== false &&
                    !transparencyOff &&
                    !reduced
                  }
                  onChange={(event) => setGlassEnabled(event.target.checked)}
                />
              </div>
            </div>
            <div className="setting">
              <div className="copy">
                <strong>{t("theme.glass.colorEnabled")}</strong>
                <p>{t("theme.glass.colorEnabledHelp")}</p>
              </div>
              <div className="visual">
                <input
                  type="checkbox"
                  role="switch"
                  aria-label={t("theme.glass.colorEnabled")}
                  disabled={reduced || transparencyOff}
                  checked={tuning.glassColor !== false}
                  onChange={(event) => {
                    if (!event.target.checked) setGlassColorDraft(null);
                    setGlassColor(event.target.checked);
                  }}
                />
              </div>
            </div>
            <div className="setting">
              <div className="copy">
                <strong>{t("theme.glass.color")}</strong>
                <p>{t("theme.glass.colorHelp")}</p>
              </div>
              <div className="visual">
                <input
                  className="glass-color-input"
                  type="color"
                  aria-label={t("theme.glass.color")}
                  disabled={reduced || transparencyOff || tuning.glassColor === false}
                  value={glassColorDraft ?? tuning.glassTint ?? tokens["color.accent"] ?? "#beff2d"}
                  onChange={(event) => setGlassColorDraft(event.target.value)}
                  onBlur={() => { if (glassColorDraft !== null) setGlassTint(glassColorDraft); }}
                />
              </div>
            </div>
            <div className="setting">
              <div className="copy">
                <strong>{t("theme.glass.backend")}</strong>
                <p>{t("theme.glass.backendHelp")}</p>
              </div>
              <div className="visual">
                <RumahlSelect
                  label={t("theme.glass.backend")}
                  disabled={reduced || transparencyOff}
                  value={tuning.glassBackend ?? "auto"}
                  onChange={(value) =>
                    setGlassBackend(value as "auto" | "svg" | "webgl" | "css")
                  }
                  options={(["auto", "svg", "webgl", "css"] as const).map(
                    (backend) => {
                      const warning =
                        backend === "svg" || backend === "webgl"
                          ? backendWarning[backend]
                          : undefined;
                      return {
                        value: backend,
                        label: t(`theme.glass.backend.${backend}`),
                        disabled: warning !== undefined,
                        warning,
                      };
                    },
                  )}
                />
              </div>
            </div>
            <div className="setting">
              <div className="copy">
                <strong>{t("theme.glass.quality")}</strong>
                <p>{t("theme.glass.qualityHelp")}</p>
              </div>
              <div className="visual">
                <RumahlSelect
                  label={t("theme.glass.quality")}
                  disabled={reduced || transparencyOff}
                  value={tuning.glassQuality ?? "auto"}
                  onChange={(value) =>
                    setGlassQuality(
                      value as "auto" | "high" | "balanced" | "low",
                    )
                  }
                  options={(["auto", "high", "balanced", "low"] as const).map(
                    (quality) => ({
                      value: quality,
                      label: t(`theme.glass.quality.${quality}`),
                    }),
                  )}
                />
              </div>
            </div>
            <div className="setting">
              <div className="copy">
                <strong>{t("theme.glass.refraction")}</strong>
                <p>{t("theme.glass.refractionHelp")}</p>
              </div>
              <div className="visual visual--range">
                <input
                  className="range"
                  type="range"
                  aria-label={t("theme.glass.refraction")}
                  disabled={reduced || transparencyOff}
                  min={0}
                  max={100}
                  value={refraction}
                  onChange={(event) => {
                    const value = withSnap(
                      Number(event.target.value),
                      REFRACTION_DEFAULT,
                      4,
                    );
                    setRefraction(value);
                    setGlassAdjust({ refraction: value });
                  }}
                />
                <output>{refraction}%</output>
              </div>
            </div>
            <div className="setting">
              <div className="copy">
                <strong>{t("theme.glass.chroma")}</strong>
                <p>{t("theme.glass.chromaHelp")}</p>
              </div>
              <div className="visual visual--range">
                <input
                  className="range"
                  type="range"
                  aria-label={t("theme.glass.chroma")}
                  disabled={reduced || transparencyOff}
                  min={0}
                  max={60}
                  value={chromaPercent}
                  onChange={(event) => {
                    const value = withSnap(
                      Number(event.target.value),
                      CHROMA_DEFAULT,
                      2,
                    );
                    setChromaPercent(value);
                    setGlassAdjust({ chroma: value / 100 });
                  }}
                />
                <output>{chromaPercent}%</output>
              </div>
            </div>
            <div className="setting">
              <div className="copy">
                <strong>{t("theme.glass.saturation")}</strong>
                <p>{t("theme.glass.saturationHelp")}</p>
              </div>
              <div className="visual visual--range">
                <input
                  className="range"
                  type="range"
                  aria-label={t("theme.glass.saturation")}
                  disabled={reduced || transparencyOff}
                  min={0}
                  max={250}
                  value={saturationPercent}
                  onChange={(event) => {
                    const value = withSnap(
                      Number(event.target.value),
                      SATURATION_DEFAULT,
                      8,
                    );
                    setSaturationPercent(value);
                    setGlassAdjust({ saturation: value / 100 });
                  }}
                />
                <output>{saturationPercent}%</output>
              </div>
            </div>
            <div className="setting">
              <div className="copy">
                <strong>{t("theme.glass.testImage")}</strong>
                <p>{t("theme.glass.testImageHelp")}</p>
              </div>
              <div className="visual">
                <input
                  type="file"
                  accept="image/*"
                  aria-label={t("theme.glass.testImage")}
                  onChange={(event) => {
                    const file = event.target.files?.[0];
                    if (file)
                      setTestMedia({
                        image: URL.createObjectURL(file),
                        video: null,
                      });
                  }}
                />
              </div>
            </div>
            <div className="setting">
              <div className="copy">
                <strong>{t("theme.glass.testVideo")}</strong>
                <p>{t("theme.glass.testVideoHelp")}</p>
              </div>
              <div className="visual">
                <input
                  type="file"
                  accept="video/*"
                  aria-label={t("theme.glass.testVideo")}
                  onChange={(event) => {
                    const file = event.target.files?.[0];
                    if (file)
                      setTestMedia({
                        video: URL.createObjectURL(file),
                        image: null,
                      });
                  }}
                />
              </div>
            </div>
            <div className="setting">
              <div className="copy">
                <strong>{t("theme.glass.testReset")}</strong>
                <p>{t("theme.glass.testResetHelp")}</p>
              </div>
              <div className="visual">
                <Button
                  size="sm"
                  onClick={() => setTestMedia({ image: null, video: null })}
                >
                  {t("theme.glass.testReset")}
                </Button>
              </div>
            </div>
          </div>
        </>
      ) : null}
      <h3 className="section-title">{t("theme.performance.title")}</h3>
      <div className="settings-group">
        <div className="setting">
          <div className="copy">
            <strong>{t("theme.performance.mode")}</strong>
            <p>{t("theme.performance.modeHelp")}</p>
          </div>
          <div className="visual">
            <input
              type="checkbox"
              role="switch"
              aria-label={t("theme.performance.mode")}
              checked={tuning.performanceMode === true}
              onChange={(event) => setPerformanceMode(event.target.checked)}
            />
          </div>
        </div>
        <div className="setting">
          <div className="copy">
            <strong>{t("theme.performance.animations")}</strong>
            <p>{t("theme.performance.animationsHelp")}</p>
          </div>
          <div className="visual">
            <input
              type="checkbox"
              role="switch"
              aria-label={t("theme.performance.animations")}
              disabled={performanceMode}
              checked={tuning.animations !== false}
              onChange={(event) => setAnimations(event.target.checked)}
            />
          </div>
        </div>
      </div>

      {activeTheme.parameters.length > 0 ? (
        <>
          <h3 className="section-title">{t("theme.customize")}</h3>
          <div className="settings-group">
            {seedParams.map(renderParameter)}
            {ranges.length > 0 ? (
              <details className="theme-custom-variant">
                <summary>{t("theme.customVariant")}</summary>
                <p className="theme-custom-variant__help">
                  {t("theme.customVariantHelp")}
                </p>
                <div className="theme-range-params">
                  {ranges.map(renderParameter)}
                </div>
              </details>
            ) : null}
            <div className="setting">
              <div className="copy" />
              <div className="visual">
                <Button variant="danger" onClick={reset}>
                  {t("theme.reset")}
                </Button>
              </div>
            </div>
          </div>
        </>
      ) : null}

      {overrideColors.length > 0 || activeTheme.modes.includes("dark") ? (
        <>
          <h3 className="section-title">{t("theme.overrides")}</h3>
          <div className="settings-group">
            <details className="theme-custom-variant">
              <summary>
                {t("theme.overrides")}
                <span className="theme-custom-variant__state">
                  {overridesActive ? t("theme.customized") : t("theme.auto")}
                </span>
              </summary>
              <p className="theme-custom-variant__help">
                {t("theme.overridesHelp")}
              </p>
              {activeTheme.modes.includes("dark") ? (
                <div className="theme-parameter">
                  <span>{t("theme.parameter.mode")}</span>
                  <RumahlButtonGroup<"auto" | "light" | "dark">
                    label={t("theme.parameter.mode")}
                    value={tuning.mode === null ? "auto" : tuning.mode}
                    onChange={(value) => setMode(value === "auto" ? null : value)}
                    options={[
                      { value: "auto", label: t("theme.mode.auto") },
                      { value: "light", label: t("theme.mode.light") },
                      { value: "dark", label: t("theme.mode.dark") }
                    ]}
                  />
                </div>
              ) : null}
              {overrideColors.map(renderOverride)}
              <div className="setting">
                <div className="copy" />
                <div className="visual">
                  <Button onClick={autoColors}>{t("theme.resetAuto")}</Button>
                </div>
              </div>
            </details>
          </div>
        </>
      ) : null}

      <p>
        {t("preferences.effective")}: {t(`mode.${settings.mode}`)} ·{" "}
        {effectiveTheme?.name ?? settings.theme}
      </p>
      <p role="status">
        {settings.saving
          ? t("preferences.saving")
          : settings.error
            ? t(`preferences.${settings.error}`)
            : settings.ready
              ? t("preferences.synced")
              : t("preferences.loading")}
      </p>
      {settings.persistence !== "local" ? (
        <p>{t("preferences.temporary")}</p>
      ) : null}
    </section>
  );
}

/** Normalizes a token colour to the `#rrggbb` form an `<input type="color">` needs. */
function toHex(value: string): string {
  const match = /^#([0-9a-fA-F]{6})/.exec(value);
  return match ? `#${match[1]}` : "#000000";
}
