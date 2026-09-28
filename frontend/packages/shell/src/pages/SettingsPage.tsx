import { useTheme } from "@rumahl/ui";
import { themes } from "@rumahl/ui/themes";
import type { MessageKey } from "../i18n/locales/en";
import { useShellPreferences } from "../preferences/ShellPreferences";
import { useThemeTuning } from "../preferences/theme-tuning";
import { WorkspaceSettings } from "../preferences/WorkspaceSettings";
import { useRoutes } from "react-router";
import { SectionPlaceholder } from "../components/SectionPlaceholder";
import { useI18n } from "../i18n";
import { ShellLink } from "../routing/ShellLink";
import { UnavailablePage } from "./UnavailablePage";

export function SettingsPage() {
  const { t } = useI18n();
  const content = useRoutes([
    { index: true, element: <SectionPlaceholder section="settings" /> },
    { path: "display", Component: DisplaySettings },
    { path: "workspace", Component: WorkspaceSettings },
    { path: "*", Component: UnavailablePage }
  ]);
  return <div><nav aria-label={t("nav.settings")} className="settings-tabs">
    <ShellLink to="/settings">{t("nav.settings")}</ShellLink>
    <ShellLink to="/settings/display">{t("mode.label")}</ShellLink>
    <ShellLink to="/settings/workspace">{t("workspace.title")}</ShellLink>
  </nav>{content}</div>;
}
function DisplaySettings() {
  const { t } = useI18n();
  const settings = useShellPreferences();
  const { scope, setScope } = settings;
  const selected = settings.preferences?.[scope].shellMode;
  const selectedTheme = settings.preferences?.[scope].shellTheme;
  const effectiveTheme = themes.find((item) => item.id === settings.theme);
  const { tokens, theme: activeTheme } = useTheme();
  const { tuning, setSeed, setMode, setToken, reset } = useThemeTuning();
  return <section><h1>{t("mode.label")}</h1>
    <p>{t("preferences.precedence")}</p>
    <label>{t("preferences.scope")} <select aria-label={t("preferences.scope")} value={scope} onChange={(event) => setScope(event.target.value === "device" ? "device" : "user")}>
      <option value="user">{t("preferences.user")}</option><option value="device">{t("preferences.device")}</option>
    </select></label>
    <fieldset disabled={!settings.ready || settings.saving}>
      <legend>{t("mode.label")}</legend>
      <button type="button" aria-pressed={selected === "desktop"} onClick={() => settings.save(scope, "desktop")}>{t("mode.desktop")}</button>
      <button type="button" aria-pressed={selected === "launcher"} onClick={() => settings.save(scope, "launcher")}>{t("mode.launcher")}</button>
      {scope === "device" ? <button type="button" aria-pressed={selected === null} onClick={() => settings.save("device", null)}>{t("preferences.inherit")}</button> : null}
    </fieldset>
    <fieldset disabled={!settings.ready || settings.saving}>
      <legend>{t("theme.title")}</legend>
      <p>{t("theme.help")}</p>
      {themes.map((theme) => <button key={theme.id} type="button" aria-pressed={selectedTheme === theme.id} onClick={() => settings.saveTheme(scope, theme.id)}>{theme.name}</button>)}
      {scope === "device" ? <button type="button" aria-pressed={selectedTheme === null} onClick={() => settings.saveTheme("device", null)}>{t("preferences.inherit")}</button> : null}
    </fieldset>
    {activeTheme.parameters.length > 0 ? <fieldset disabled={!settings.ready}>
      <legend>{t("theme.customize")}</legend>
      {activeTheme.parameters.map((parameter) => {
        const label = t(parameter.labelKey as MessageKey);
        const current = tokens[parameter.token];
        if (parameter.kind === "range") {
          const value = parameter.parse(current);
          return <label key={parameter.id} className="theme-parameter">
            <span>{label}</span>
            <input type="range" aria-label={label} min={parameter.min} max={parameter.max} step={parameter.step} value={value}
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
              <button key={preset.id} type="button" className={`theme-swatch theme-swatch--${preset.id}`}
                aria-label={t(preset.labelKey as MessageKey)} title={t(preset.labelKey as MessageKey)}
                aria-pressed={toHex(current).toLowerCase() === preset.value.toLowerCase()} onClick={() => assign(preset.value)} />
            ))}</span> : null}
            <output>{toHex(current)}</output>
          </label>;
        }
        return <label key={parameter.id} className="theme-parameter">
          <span>{label}</span>
          <select aria-label={label} value={current} onChange={(event) => setToken(parameter.token, event.target.value)}>
            {parameter.options.map((option) => <option key={option.value} value={option.value}>{t(option.labelKey as MessageKey)}</option>)}
          </select>
        </label>;
      })}
      <button type="button" onClick={reset}>{t("theme.reset")}</button>
    </fieldset> : null}
    {activeTheme.modes.includes("dark") ? <fieldset disabled={!settings.ready}>
      <legend>{t("theme.parameter.mode")}</legend>
      <button type="button" aria-pressed={(tuning.mode ?? "light") === "light"} onClick={() => setMode("light")}>{t("theme.mode.light")}</button>
      <button type="button" aria-pressed={tuning.mode === "dark"} onClick={() => setMode("dark")}>{t("theme.mode.dark")}</button>
    </fieldset> : null}
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
