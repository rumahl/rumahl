import { useTheme } from "@rumahl/ui";
import { getTheme } from "@rumahl/ui/sdk";
import { Button } from "../components/Button";
import { SettingsHeading } from "../components/SettingsHeading";
import { ThemeParameterControl } from "../components/ThemeParameterControl";
import { useI18n } from "../i18n";
import { useThemeTuning } from "../preferences/theme-tuning";

/** Options of the theme that is currently active. */
export function ThemeSettings() {
  const { t } = useI18n();
  const { theme, tokens } = useTheme();
  const tuning = useThemeTuning();
  const name = getTheme(theme.id)?.name ?? theme.name;

  return <section className="theme-manager">
    <SettingsHeading
      title={t("themes.settingsFor", { name })}
      help={t("themes.settingsPageHelp")}
      back="/settings/display/themes"
    />
    {theme.parameters.length === 0
      ? <p className="settings-footnote">{t("themes.noSettings")}</p>
      : <div className="settings-group">
          {theme.parameters.map((parameter) => <ThemeParameterControl
            key={parameter.id}
            parameter={parameter}
            tokens={tokens}
            onChange={(token, value) => tuning.setToken(token, value)}
            onSeed={tuning.setSeed}
          />)}
        </div>}
    <div className="setting">
      <div className="copy" />
      <div className="visual">
        <Button variant="danger" onClick={tuning.reset}>{t("theme.reset")}</Button>
      </div>
    </div>
  </section>;
}
