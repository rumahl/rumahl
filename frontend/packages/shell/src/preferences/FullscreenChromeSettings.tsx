import { LoadingStatus } from "../components/LoadingStatus";
import { SettingsHeading } from "../components/SettingsHeading";
import { useI18n } from "../i18n";
import { defaultFullscreenChrome, useWorkspace } from "./Workspace";
import { useId } from "react";
import { GroupContainer, GroupRow } from "@rumahl/ui/components";
import { SettingsReset } from "../components/SettingsReset";

/**
 * Account-scoped chrome preferences. Kept separate from the workspace page:
 * these switches live with the shell appearance, not with the saved window
 * arrangement (which has its own device/account scope).
 */
export function FullscreenChromeSettings() {
  const { t } = useI18n();
  const workspace = useWorkspace();
  const id = useId();
  const chrome = workspace.userWorkspace?.fullscreenChrome ?? workspace.effective.fullscreenChrome;
  const busy = !workspace.ready || workspace.busy;
  const updateChrome = (next: Partial<typeof chrome>) =>
    // Creating the account record must not wipe an arrangement that only
    // exists as a device override: seed it from the visible workspace.
    void workspace.save("user", {
      ...(workspace.userWorkspace ?? workspace.effective),
      fullscreenChrome: { ...chrome, ...next }
    });
  return <section className="display-settings">
    <SettingsHeading title={t("workspace.fullscreen")} help={t("workspace.fullscreenPageHelp")} back="/settings/display" />

    <h3 className="section-title">{t("workspace.chromeGroup")}</h3>
    <GroupContainer>
      <GroupRow title={t("workspace.hideTopbar")} description={t("workspace.hideTopbarHelp")} controlId={`${id}-topbar`}>
        <input id={`${id}-topbar`} type="checkbox" role="switch" checked={chrome.hideTopbar} disabled={busy}
          onChange={(event) => updateChrome({ hideTopbar: event.target.checked })} />
      </GroupRow>
      <GroupRow title={t("workspace.hideDock")} description={t("workspace.hideDockHelp")} controlId={`${id}-dock`}>
        <input id={`${id}-dock`} type="checkbox" role="switch" checked={chrome.hideDock} disabled={busy}
          onChange={(event) => updateChrome({ hideDock: event.target.checked })} />
      </GroupRow>
      <SettingsReset label={t("workspace.resetChrome")} description={t("workspace.resetGroupHelp")}
        disabled={busy || (chrome.hideTopbar === defaultFullscreenChrome.hideTopbar && chrome.hideDock === defaultFullscreenChrome.hideDock)}
        onReset={() => updateChrome({ hideTopbar: defaultFullscreenChrome.hideTopbar, hideDock: defaultFullscreenChrome.hideDock })} />
    </GroupContainer>
    <h3 className="section-title">{t("workspace.spaceGroup")}</h3>
    <GroupContainer>
      <GroupRow title={t("workspace.dockClearance")} description={t("workspace.dockClearanceHelp")} controlId={`${id}-space`}>
        <input id={`${id}-space`} type="checkbox" role="switch" checked={chrome.dockClearance} disabled={busy || chrome.hideDock}
          onChange={(event) => updateChrome({ dockClearance: event.target.checked })} />
      </GroupRow>
      <SettingsReset label={t("workspace.resetSpace")} description={t("workspace.resetGroupHelp")}
        disabled={busy || chrome.dockClearance === defaultFullscreenChrome.dockClearance}
        onReset={() => updateChrome({ dockClearance: defaultFullscreenChrome.dockClearance })} />
    </GroupContainer>

    <p className="settings-section-help">{t("workspace.fullscreenAccount")}</p>

    {workspace.error
      ? <p role="status">{t(`preferences.${workspace.error === "conflict" ? "conflict" : "unavailable"}`)}</p>
      : workspace.busy
        ? <LoadingStatus className="loading-status--inline" label={t("preferences.saving")} />
        : null}
  </section>;
}
