import { useI18n } from "../i18n";
import { WindowStackIcon } from "../icons";
import { SettingsHeading } from "../components/SettingsHeading";
import { useShell } from "../shell/ShellContext";
import { useDeviceViewport } from "../shell/device";
import { openMobileTour } from "../shell/MobileTutorial";
import { useThemeTuning } from "../preferences/theme-tuning";

/** Phone-only settings. The desktop option stays tucked inside Advanced. */
export function MobileSettings() {
  const { t } = useI18n();
  const { mode, setMode, dispatch } = useShell();
  const viewport = useDeviceViewport();
  const { tuning, setMobileNavSvgGlass } = useThemeTuning();
  return <section className="display-settings mobile-settings">
    <SettingsHeading title={t("settings.mobile")} help={t("mobile.help")} back="/settings" />

    <h3 className="section-title">{t("mobile.shell")}</h3>
    <div className="settings-group">
      <div className="setting setting--value">
        <div className="copy"><strong>{t("mobile.mode")}</strong><p>{t("mobile.modeHelp")}</p></div>
        <div className="visual"><span>{t(`mode.${mode}`)}</span></div>
      </div>
      <label className="setting setting--toggle">
        <span className="copy"><strong>{t("mobile.navGlass")}</strong><p>{t("mobile.navGlassHelp")}</p></span>
        <span className="visual">
          <input type="checkbox" role="switch" aria-label={t("mobile.navGlass")} checked={tuning.mobileNavSvgGlass === true}
            onChange={(event) => setMobileNavSvgGlass(event.target.checked)} />
        </span>
      </label>
      <div className="setting">
        <div className="copy"><strong>{t("mobile.tour")}</strong><p>{t("mobile.tourHelp")}</p></div>
        <div className="visual"><button type="button" className="mobile-settings__action" onClick={openMobileTour}>{t("mobile.tourStart")}</button></div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("mobile.switcher")}</strong><p>{t("mobile.switcherHelp")}</p></div>
        <div className="visual"><button type="button" className="mobile-settings__action" onClick={() => dispatch({ type: "toggle-app-switcher" })}><WindowStackIcon aria-hidden="true" />{t("mobile.switcherOpen")}</button></div>
      </div>
    </div>

    <details className="mobile-advanced">
      <summary>{t("mobile.advanced")}</summary>
      <div className="mobile-advanced__body">
        <p>{t("mobile.advancedHelp")}</p>
        <div className="setting">
          <div className="copy"><strong>{t("mobile.desktopTitle")}</strong><p>{t("mobile.desktopHelp")}</p></div>
          <div className="visual"><button type="button" className="mobile-settings__action" onClick={() => setMode("desktop")}>{t("mobile.desktopEnable")}</button></div>
        </div>
        {!viewport.landscape ? <p className="settings-footnote">{t("orientation.body")}</p> : null}
      </div>
    </details>
  </section>;
}
