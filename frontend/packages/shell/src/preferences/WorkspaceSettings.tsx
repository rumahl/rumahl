import { isInternalTarget } from "../routing/internal";
import { RumahlInput, RumahlCheckbox } from "../components/RumahlInputs";
import { RumahlSelect } from "../components/RumahlSelect";
import { Button } from "../components/Button";
import { SettingsHeading } from "../components/SettingsHeading";
import { useState } from "react";
import { useWorkspace, type AppFolder } from "./Workspace";
import { useShell } from "../shell/ShellContext";
import { useShellPreferences } from "./ShellPreferences";
import { useI18n } from "../i18n";
import { useAppCatalog } from "../apps/AppCatalog";
import { firstPartyApps } from "../apps/registry";

export function WorkspaceSettings() {
  const { t } = useI18n();
  const workspace = useWorkspace();
  const settings = useShellPreferences();
  const { state } = useShell();
  const catalog = useAppCatalog();
  const [name, setName] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const folders = workspace.effective.folders;
  const busy = !workspace.ready || workspace.busy;
  const apps = [...firstPartyApps.map(a => ({ id: a.id, title: t(a.title) })), { id: "settings", title: t("nav.settings") }, ...catalog.apps.filter(a => !firstPartyApps.some(f => f.id === a.id))];
  const updateFolders = async (next: AppFolder[]) => workspace.save(settings.scope, { ...workspace.effective, folders: next });
  const beginEdit = (folder: AppFolder) => { setEditing(folder.id); setName(folder.name); setSelected(folder.apps); };
  const cancelEdit = () => { setEditing(null); setName(""); setSelected([]); };
  return <section className="display-settings">
    <SettingsHeading title={t("workspace.title")} help={t("workspace.help")} back="/settings" />

    <h3 className="section-title">{t("preferences.scope")}</h3>
    <div className="settings-group">
      <div className="setting">
        <div className="copy"><strong>{t("preferences.scope")}</strong><p>{t("preferences.precedence")}</p></div>
        <div className="visual"><RumahlSelect label={t("preferences.scope")} value={settings.scope} onChange={value => settings.setScope(value === "user" ? "user" : "device")}
          options={[{ value: "user", label: t("preferences.user") }, { value: "device", label: t("preferences.device") }]} /></div>
      </div>
    </div>

    <h3 className="section-title">{t("workspace.layout")}</h3>
    <div className="settings-group">
      <div className="setting">
        <div className="copy"><strong>{t("workspace.layout")}</strong><p>{t("workspace.help")}</p></div>
        <div className="visual workspace-actions">
          <Button variant="primary" disabled={busy}
            onClick={() => void workspace.save(settings.scope, { ...workspace.effective, windows: state.windows.filter(w => w.location && !w.streamId && !isInternalTarget(w.location)).slice(0, 32).map(w => ({ location: w.location!, rect: w.rect ?? { x: 36, y: 24, width: 760, height: 540 }, placement: w.placement ?? "floating", minimized: w.minimized })) })}>{t("workspace.save")}</Button>
          <Button disabled={busy} onClick={workspace.requestRestore}>{t("workspace.restore")}</Button>
          {settings.scope === "device" ? <Button variant="ghost" disabled={busy} onClick={() => void workspace.save("device", null)}>{t("preferences.inherit")}</Button> : null}
        </div>
      </div>
    </div>

    <h3 className="section-title">{t("workspace.folders")}</h3>
    <div className="settings-group">
      {folders.length === 0 ? <p className="settings-empty">{t("workspace.folders")} – {t("workspace.create")}</p> : folders.map(folder => (
        <div className="setting" key={folder.id}>
          <div className="copy"><strong>{folder.name}</strong><p>{folder.apps.length}</p></div>
          <div className="visual workspace-actions">
            <Button size="sm" onClick={() => beginEdit(folder)}>{t("workspace.edit")}</Button>
            <Button size="sm" variant="danger" disabled={workspace.busy} onClick={() => void updateFolders(folders.filter(v => v.id !== folder.id))}>{t("workspace.remove")}</Button>
          </div>
        </div>
      ))}
    </div>

    <details className="theme-custom-variant" open={editing !== null}>
      <summary>{t(editing ? "workspace.edit" : "workspace.create")}</summary>
      <p className="theme-custom-variant__help">{t("workspace.help")}</p>
      <form onSubmit={event => {
        event.preventDefault();
        void updateFolders([...folders.filter(f => f.id !== editing).map(f => ({ ...f, apps: f.apps.filter(id => !selected.includes(id)) })), { id: editing ?? crypto.randomUUID(), name: name.trim(), apps: selected }]).then(ok => { if (ok) cancelEdit(); });
      }}>
        <div className="settings-group">
          <label className="setting">
            <span className="copy"><strong>{t("workspace.name")}</strong></span>
            <span className="visual"><RumahlInput required maxLength={64} value={name} onChange={event => setName(event.target.value)} aria-label={t("workspace.name")} /></span>
          </label>
        </div>
        <div className="folder-members">{apps.map(app => <RumahlCheckbox key={app.id} label={app.title} checked={selected.includes(app.id)} onChange={event => setSelected(event.target.checked ? [...selected, app.id] : selected.filter(id => id !== app.id))} />)}</div>
        <div className="workspace-actions">
          <Button type="submit" variant="primary" disabled={busy}>{t("workspace.saveFolder")}</Button>
          {editing ? <Button variant="ghost" onClick={cancelEdit}>{t("workspace.cancel")}</Button> : null}
        </div>
      </form>
    </details>

    <p role="status">{workspace.error ? t(`preferences.${workspace.error === "conflict" ? "conflict" : "unavailable"}`) : workspace.busy ? t("preferences.saving") : workspace.ready ? t("preferences.synced") : t("preferences.loading")}</p>
  </section>;
}
