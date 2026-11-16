import { LoadingStatus } from "../components/LoadingStatus";
import { SettingsHeading } from "../components/SettingsHeading";
import { useI18n } from "../i18n";
import { useWorkspace } from "./Workspace";

/**
 * Account-scoped chrome preferences. Kept separate from the workspace page:
 * these switches live with the shell appearance, not with the saved window
 * arrangement (which has its own device/account scope).
 */
export function FullscreenChromeSettings() {
  const { t } = useI18n();
  const workspace = useWorkspace();
  const chrome = workspace.userWorkspace?.fullscreenChrome ?? workspace.effective.fullscreenChrome;
  const busy = !workspace.ready || workspace.busy;
  const updateChrome = (next: Partial<{ hideTopbar: boolean; hideDock: boolean }>) =>
    // Creating the account record must not wipe an arrangement that only
    // exists as a device override: seed it from the visible workspace.
    void workspace.save("user", {
      ...(workspace.userWorkspace ?? workspace.effective),
      fullscreenChrome: { ...chrome, ...next }
    });
  return <section className="display-settings">
    <SettingsHeading title={t("workspace.fullscreen")} help={t("workspace.fullscreenPageHelp")} back="/settings/display" />

    <div className="settings-group">
      <div className="setting">
        <div className="copy"><strong>{t("workspace.hideTopbar")}</strong><p>{t("workspace.hideTopbarHelp")}</p></div>
        <div className="visual">
          <input type="checkbox" role="switch" aria-label={t("workspace.hideTopbar")} checked={chrome.hideTopbar} disabled={busy}
            onChange={(event) => updateChrome({ hideTopbar: event.target.checked })} />
        </div>
      </div>
      <div className="setting">
        <div className="copy"><strong>{t("workspace.hideDock")}</strong><p>{t("workspace.hideDockHelp")}</p></div>
        <div className="visual">
          <input type="checkbox" role="switch" aria-label={t("workspace.hideDock")} checked={chrome.hideDock} disabled={busy}
            onChange={(event) => updateChrome({ hideDock: event.target.checked })} />
        </div>
      </div>
    </div>

    <p className="settings-section-help">{t("workspace.fullscreenAccount")}</p>

    {workspace.error
      ? <p role="status">{t(`preferences.${workspace.error === "conflict" ? "conflict" : "unavailable"}`)}</p>
      : workspace.busy
        ? <LoadingStatus className="loading-status--inline" label={t("preferences.saving")} />
        : null}
  </section>;
}
