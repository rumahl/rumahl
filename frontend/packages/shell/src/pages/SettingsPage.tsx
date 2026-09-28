import { themes } from "@rumahl/ui/themes";
import { WorkspaceSettings } from "../preferences/WorkspaceSettings";
import { useShellPreferences } from "../preferences/ShellPreferences";
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
  const effectiveTheme = themes.find((theme) => theme.id === settings.theme);
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
    <p>{t("preferences.effective")}: {t(`mode.${settings.mode}`)} · {effectiveTheme?.name ?? settings.theme}</p>
    <p role="status">{settings.saving ? t("preferences.saving") : settings.error ? t(`preferences.${settings.error}`) : settings.ready ? t("preferences.synced") : t("preferences.loading")}</p>
    {settings.persistence !== "local" ? <p>{t("preferences.temporary")}</p> : null}
  </section>;
}
