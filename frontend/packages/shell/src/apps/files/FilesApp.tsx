import { useEffect, useMemo, useState } from "react";
import { useI18n } from "../../i18n";
import { useOsModePolicy } from "../../preferences/OsMode";
import { browserProfile } from "../../preferences/storage";
import { useShell } from "../../shell/ShellContext";
import { Breadcrumb } from "./Breadcrumb";
import { ContextMenu, type MenuItem } from "./ContextMenu";
import { FileGrid, FileList, type ViewProps } from "./FileViews";
import { Sidebar } from "./Sidebar";
import { Toolbar } from "./Toolbar";
import { MoveIcon, OpenIcon, PencilIcon, TrashIcon } from "./icons";
import { type Item } from "./types";
import { useExplorer } from "./useExplorer";
import { useSelection } from "./useSelection";
import "./files.css";
/**
 * A real file explorer: locations on the left, the current folder on the right
 * with history, breadcrumb, search, grid/list views, selection, context menu
 * and inline rename. Personal files are fully editable; host roots (`/apps`,
 * `/rumahl`) are read-only and appear from advanced mode upwards.
 */
export function FilesApp() {
  const { live } = useShell();
  const { t } = useI18n();
  const { advancedSettings } = useOsModePolicy();
  const device = browserProfile().id;
  const explorer = useExplorer(live ?? null, device, advancedSettings);
  const { selected, select, clear } = useSelection(explorer.entries);
  const [view, setView] = useState<"grid" | "list">("grid");
  const [search, setSearch] = useState("");
  const [renameKey, setRenameKey] = useState<string | null>(null);
  const [draftFolder, setDraftFolder] = useState<string | null>(null);
  const [clipboard, setClipboard] = useState<{ id: string; name: string } | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; item: Item } | null>(null);

  useEffect(() => { clear(); setSearch(""); setRenameKey(null); setDraftFolder(null); setMenu(null); }, [explorer.nav, clear]);
  useEffect(() => { if (explorer.error) clear(); }, [explorer.error, clear]);

  const visible = useMemo(() => {
    const query = search.trim().toLowerCase();
    return query ? explorer.entries.filter((item) => item.name.toLowerCase().includes(query)) : explorer.entries;
  }, [explorer.entries, search]);

  function commitRename(item: Item, name: string) {
    setRenameKey(null);
    const trimmed = name.trim();
    if (trimmed && trimmed !== item.name) void explorer.rename(item.key, trimmed);
  }
  function submitFolder(name: string) { setDraftFolder(null); if (name.trim()) void explorer.createFolder(name.trim()); }

  const viewProps: ViewProps = {
    items: visible,
    selected,
    renameKey,
    onSelect: select,
    onOpen: explorer.open,
    onRenameCommit: commitRename,
    onRenameCancel: () => setRenameKey(null),
    onMenu: (item, x, y) => setMenu({ x, y, item }),
  };
  const menuItems = (item: Item): MenuItem[] => [
    { label: t("files.open"), icon: <OpenIcon />, onSelect: () => explorer.open(item) },
    ...(explorer.isStore
      ? [
          { label: t("files.rename"), icon: <PencilIcon />, onSelect: () => setRenameKey(item.key) },
          { label: t("files.move"), icon: <MoveIcon />, onSelect: () => setClipboard({ id: item.key, name: item.name }) },
          { label: t("files.delete"), icon: <TrashIcon />, danger: true, onSelect: () => explorer.remove(item.key) },
        ]
      : []),
  ];

  return <section className="files-app" onKeyDown={(event) => {
    if (event.key !== "Delete" || !explorer.isStore) return;
    for (const item of visible) if (selected.has(item.key)) explorer.remove(item.key);
  }}>
    <Sidebar nav={explorer.nav} advanced={advancedSettings} onNavigate={explorer.navigate} />
    <div className="files-app__main">
      <Toolbar
        isStore={explorer.isStore} busy={explorer.busy}
        canBack={explorer.canBack} canForward={explorer.canForward} canUp={explorer.canUp}
        canPaste={!!clipboard} view={view} search={search}
        onBack={explorer.back} onForward={explorer.forward} onUp={explorer.up}
        onView={setView} onSearch={setSearch}
        onNewFolder={() => setDraftFolder("")} onUpload={explorer.upload}
        onPaste={() => { if (clipboard) { void explorer.move(clipboard.id, clipboard.name); setClipboard(null); } }}
      />
      <div className="files-address"><Breadcrumb nav={explorer.nav} onNavigate={explorer.navigate} /></div>
      <div className="files-surface">
        {draftFolder !== null ? <form className="files-draft" onSubmit={(event) => { event.preventDefault(); submitFolder(draftFolder); }}>
          <input autoFocus required maxLength={255} value={draftFolder} onChange={(event) => setDraftFolder(event.target.value)} placeholder={t("files.newFolder")} />
          <button type="submit">{t("files.save")}</button>
          <button type="button" onClick={() => setDraftFolder(null)}>{t("workspace.cancel")}</button>
        </form> : null}
        {live ? null : <p role="status">{t("files.demo")}</p>}
        {explorer.error ? <p role="alert">{t(`files.${explorer.error}`)}</p> : null}
        {explorer.loading
          ? <div className="files-skeleton" aria-hidden="true">{Array.from({ length: 8 }).map((_, index) => <div key={index} className="files-skeleton__cell" />)}</div>
          : visible.length === 0 ? <p className="files-empty">{t("files.empty")}</p>
          : view === "grid" ? <FileGrid {...viewProps} /> : <FileList {...viewProps} />}
      </div>
      <div className="files-status">
        <span>{t("files.items", { count: visible.length })}</span>
        {selected.size ? <span>{t("files.selectedCount", { count: selected.size })}</span> : null}
      </div>
    </div>
    {menu ? <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.item)} onClose={() => setMenu(null)} /> : null}
  </section>;
}
