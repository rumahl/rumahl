import { useEffect, useMemo, useRef, useState } from "react";
import { useI18n } from "../../i18n";
import { useOsModePolicy } from "../../preferences/OsMode";
import { browserProfile } from "../../preferences/storage";
import { useShell } from "../../shell/ShellContext";
import { Breadcrumb } from "./Breadcrumb";
import { ContextMenu, type MenuItem } from "./ContextMenu";
import { FileGrid, FileList, type ViewProps } from "./FileViews";
import { Sidebar } from "./Sidebar";
import { Toolbar } from "./Toolbar";
import { AlertIcon, FilePlusIcon, MoveIcon, OpenIcon, PencilIcon, PlusIcon, TrashIcon, UploadIcon } from "./icons";
import { type Item } from "./types";
import { useExplorer } from "./useExplorer";
import { useMarquee } from "./useMarquee";
import { useSelection } from "./useSelection";
import "./files.css";
/**
 * A real file explorer: locations on the left, the current folder on the right
 * with history, breadcrumb, search, grid/list views, selection, a context menu
 * (on items and on empty space), inline rename and new folders/files. Personal
 * files are fully editable; host roots (`/apps`, `/rumahl`) are read-only and
 * appear from advanced mode upwards.
 */
type Draft = { kind: "folder" | "file"; name: string };
type Menu = { x: number; y: number; item: Item | null };

export function FilesApp() {
  const { live } = useShell();
  const { t } = useI18n();
  const { advancedSettings } = useOsModePolicy();
  const device = browserProfile().id;
  const explorer = useExplorer(live ? live.request : null, device, advancedSettings);
  const { selected, select, clear, selectAll, setSelection } = useSelection(explorer.entries);
  const surface = useRef<HTMLDivElement>(null);
  const marquee = useMarquee(surface, selected, setSelection);
  const [view, setView] = useState<"grid" | "list">("grid");
  const [search, setSearch] = useState("");
  const [renameKey, setRenameKey] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [clipboard, setClipboard] = useState<{ id: string; name: string } | null>(null);
  const [menu, setMenu] = useState<Menu | null>(null);
  const [draggingKey, setDraggingKey] = useState<string | null>(null);
  const [dragOverKey, setDragOverKey] = useState<string | null>(null);
  const [mounted, setMounted] = useState(false);
  const root = useRef<HTMLElement>(null);
  const uploadInput = useRef<HTMLInputElement>(null);
  useEffect(() => setMounted(true), []);

  useEffect(() => { clear(); setSearch(""); setRenameKey(null); setDraft(null); setMenu(null); }, [explorer.nav, clear]);
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
  function submitDraft(value: Draft) {
    setDraft(null);
    const name = value.name.trim();
    if (!name) return;
    if (value.kind === "folder") void explorer.createFolder(name);
    else void explorer.createFile(name);
  }
  const newFolder = () => setDraft({ kind: "folder", name: "" });
  const newFile = () => setDraft({ kind: "file", name: "untitled.txt" });

  const viewProps: ViewProps = {
    items: visible,
    selected,
    renameKey,
    draggingKey,
    dragOverKey,
    onSelect: select,
    onOpen: explorer.open,
    onRenameCommit: commitRename,
    onRenameCancel: () => setRenameKey(null),
    onMenu: (item, x, y) => {
      if (!selected.has(item.key)) select(item, { ctrlKey: false, metaKey: false, shiftKey: false });
      setMenu({ x, y, item });
    },
    onDragStart: (item) => setDraggingKey(item.key),
    onDragEnd: () => { setDraggingKey(null); setDragOverKey(null); },
    onDragOver: (key) => setDragOverKey(key),
    onDropOn: (folder) => {
      if (draggingKey) {
        const item = explorer.entries.find((entry) => entry.key === draggingKey);
        if (item && item.key !== folder.key) void explorer.relocate(item.key, item.name, folder.key);
      }
      setDraggingKey(null);
      setDragOverKey(null);
    },
  };
  const menuItems = (item: Item | null): MenuItem[] => {
    if (item) {
      return [
        { label: t("files.open"), icon: <OpenIcon />, onSelect: () => explorer.open(item) },
        ...(explorer.isStore
          ? [
              { label: t("files.rename"), icon: <PencilIcon />, onSelect: () => setRenameKey(item.key) },
              { label: t("files.move"), icon: <MoveIcon />, onSelect: () => setClipboard({ id: item.key, name: item.name }) },
              { label: t("files.delete"), icon: <TrashIcon />, danger: true, onSelect: () => explorer.remove(item.key) },
            ]
          : []),
      ];
    }
    // Empty space: actions for the current folder.
    const items: MenuItem[] = [];
    if (explorer.isStore) {
      items.push(
        { label: t("files.newFolder"), icon: <PlusIcon />, onSelect: newFolder },
        { label: t("files.newFile"), icon: <FilePlusIcon />, onSelect: newFile },
        { label: t("files.upload"), icon: <UploadIcon />, onSelect: () => uploadInput.current?.click() },
      );
      if (clipboard) items.push({ label: t("files.moveHere"), icon: <MoveIcon />, onSelect: () => { void explorer.move(clipboard.id, clipboard.name); setClipboard(null); } });
    }
    items.push(
      { label: t("files.selectAll"), onSelect: selectAll },
      { label: t("files.refresh"), onSelect: explorer.refresh },
    );
    return items;
  };

  function handleKeys(event: React.KeyboardEvent) {
    if ((event.target as HTMLElement).closest("input, textarea, select")) return;
    const single = selected.size === 1 ? visible.find((item) => selected.has(item.key)) : undefined;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") { event.preventDefault(); selectAll(); return; }
    if (event.key === "Escape") { if (menu) setMenu(null); else clear(); return; }
    if ((event.key === "ArrowDown" || event.key === "ArrowUp") && visible.length) {
      event.preventDefault();
      const keys = visible.map((item) => item.key);
      const current = single ? keys.indexOf(single.key) : -1;
      const next = visible[event.key === "ArrowDown" ? Math.min(keys.length - 1, current + 1) : Math.max(0, current - 1)];
      if (next) select(next, { ctrlKey: false, metaKey: false, shiftKey: false });
      return;
    }
    if (!explorer.isStore) return;
    if (event.key === "F2" && single) { event.preventDefault(); setRenameKey(single.key); return; }
    if (event.key === "Enter" && single) { event.preventDefault(); explorer.open(single); return; }
    if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      for (const item of selected) explorer.remove(item);
    }
  }

  return <section ref={root} className="files-app" tabIndex={-1} onKeyDown={handleKeys}
    onPointerDown={(event) => { if (!(event.target as HTMLElement).closest("input, textarea, button, label, .files-menu")) root.current?.focus(); }}>
    <Sidebar nav={explorer.nav} advanced={advancedSettings} onNavigate={explorer.navigate} />
    <div className="files-app__main">
      <Toolbar
        isStore={explorer.isStore} busy={explorer.busy}
        canBack={explorer.canBack} canForward={explorer.canForward} canUp={explorer.canUp}
        canPaste={!!clipboard} view={view} search={search}
        onBack={explorer.back} onForward={explorer.forward} onUp={explorer.up}
        onView={setView} onSearch={setSearch}
        onNewFolder={newFolder} onNewFile={newFile} onUpload={explorer.upload}
        onPaste={() => { if (clipboard) { void explorer.move(clipboard.id, clipboard.name); setClipboard(null); } }}
      />
      <div className="files-address"><Breadcrumb nav={explorer.nav} onNavigate={explorer.navigate} /></div>
      {explorer.error ? <div className="files-alert" role="alert">
        <AlertIcon className="files-alert__icon" />
        <span className="files-alert__text">{t(`files.${explorer.error}`)}</span>
        <button type="button" className="files-alert__close" aria-label={t("files.dismiss")} onClick={explorer.clearError}>×</button>
      </div> : null}
      <div className="files-surface" ref={surface}
        {...marquee.handlers}
        onContextMenu={(event) => {
          if ((event.target as HTMLElement).closest("[data-file-key], .files-menu")) return;
          event.preventDefault();
          setMenu({ x: event.clientX, y: event.clientY, item: null });
        }}>
        <input ref={uploadInput} type="file" hidden onChange={(event) => { const file = event.target.files?.[0]; event.target.value = ""; if (file) explorer.upload(file); }} />
        {marquee.rect ? <span className="files-marquee" aria-hidden="true" style={{ left: marquee.rect.x, top: marquee.rect.y, width: marquee.rect.width, height: marquee.rect.height }} /> : null}
        {draft !== null ? <form className="files-draft" onSubmit={(event) => { event.preventDefault(); submitDraft(draft); }}>
          <input className="rumahl-input" autoFocus required maxLength={255} value={draft.name} onChange={(event) => setDraft({ ...draft, name: event.target.value })} placeholder={t(draft.kind === "folder" ? "files.newFolder" : "files.newFile")} />
          <button className="btn btn--primary" type="submit">{t("files.save")}</button>
          <button className="btn" type="button" onClick={() => setDraft(null)}>{t("workspace.cancel")}</button>
        </form> : null}
        {mounted && !live ? <p role="status">{t("files.demo")}</p> : null}
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
