import { useEffect, useMemo, useRef, useState } from "react";
import { useShell } from "../shell/ShellContext";
import { useI18n } from "../i18n";
import { useOsModePolicy } from "../preferences/OsMode";
import { browserProfile } from "../preferences/storage";
import { fetchHostList, hostContentUrl, type HostArea } from "./fsClient";

type Place = { kind: "store" } | { kind: "host"; area: HostArea };
interface StoreCrumb { id: string; name: string }
interface Nav { place: Place; trail: StoreCrumb[]; segments: string[] }
interface Item { key: string; name: string; directory: boolean; size: number; modified: number; host: boolean }

const ROOT: StoreCrumb = { id: "root", name: "" };
const fresh = (): Nav => ({ place: { kind: "store" }, trail: [ROOT], segments: [] });
const host = (area: HostArea): Nav => ({ place: { kind: "host", area }, trail: [ROOT], segments: [] });

/**
 * A real two-pane file explorer: locations on the left, the current folder on
 * the right with navigation, breadcrumb, grid/list view, selection, context
 * menu and inline rename. Personal files are fully editable; the host roots
 * (`/apps`, `/rumahl`) are read-only and only appear in advanced mode.
 */
export function FilesApp() {
  const { live } = useShell();
  const { t } = useI18n();
  const { advancedSettings } = useOsModePolicy();
  const device = browserProfile().id;

  const [history, setHistory] = useState<Nav[]>(() => [fresh()]);
  const [index, setIndex] = useState(0);
  const nav = history[Math.min(index, history.length - 1)]!;
  const [entries, setEntries] = useState<Item[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [view, setView] = useState<"grid" | "list">("grid");
  const [search, setSearch] = useState("");
  const [busy, setBusy] = useState(false);
  const [revision, setRevision] = useState(0);
  const [draft, setDraft] = useState<{ kind: "rename" | "newFolder"; key?: string; name: string } | null>(null);
  const [clipboard, setClipboard] = useState<{ key: string; name: string } | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; key: string } | null>(null);
  const anchor = useRef<string | null>(null);

  const isStore = nav.place.kind === "store";
  const storeParent = () => nav.trail.at(-1)?.id ?? "root";
  const hostPath = () => nav.segments.join("/");
  const placeLabel = (place: Place) => place.kind === "store" ? t("files.root") : place.area === "apps" ? t("files.applications") : t("files.system");

  function failure(status: number) { return status === 409 ? "conflict" : status === 413 ? "limit" : status === 404 ? "missing" : "unavailable"; }
  function clearSelection() { setSelected(new Set()); setMenu(null); setDraft(null); }
  function go(next: Nav) { setHistory((h) => [...h.slice(0, index + 1), next]); setIndex((i) => i + 1); clearSelection(); setSearch(""); }
  const back = () => { if (index > 0) { setIndex(index - 1); clearSelection(); } };
  const forward = () => { if (index < history.length - 1) { setIndex(index + 1); clearSelection(); } };
  function up() { if (isStore) { if (nav.trail.length > 1) go({ ...nav, trail: nav.trail.slice(0, -1) }); } else if (nav.segments.length) go({ ...nav, segments: nav.segments.slice(0, -1) }); }
  function openItem(item: Item) {
    if (item.directory) {
      go(isStore ? { ...nav, trail: [...nav.trail, { id: item.key, name: item.name }] } : { ...nav, segments: [...nav.segments, item.name] });
    } else {
      const link = document.createElement("a");
      link.href = isStore ? `/api/v1/files/content?id=${encodeURIComponent(item.key)}` : hostContentUrl(nav.place.kind === "host" ? nav.place.area : "apps", item.key, device);
      link.download = item.name;
      link.click();
    }
  }

  useEffect(() => {
    if (!live) { setLoading(false); return; }
    if (nav.place.kind === "host" && !advancedSettings) return;
    const controller = new AbortController();
    setLoading(true); setError(null); setEntries([]);
    const request = live.request;
    if (nav.place.kind === "store") {
      void request(`/api/v1/files?parent=${storeParent()}`, { signal: controller.signal, credentials: "same-origin", cache: "no-store" })
        .then(async (response) => {
          if (!response.ok) throw Error(failure(response.status));
          const value = (await response.json()) as { id: string; name: string; directory: boolean; size: number }[];
          if (!Array.isArray(value) || value.length > 1000) throw Error("unavailable");
          if (!controller.signal.aborted) setEntries(value.map((file) => ({ key: file.id, name: file.name, directory: file.directory, size: file.size, modified: 0, host: false })));
        })
        .catch((cause) => { if (!controller.signal.aborted) setError(cause instanceof Error ? cause.message : "unavailable"); })
        .finally(() => { if (!controller.signal.aborted) setLoading(false); });
    } else {
      const area = nav.place.area;
      const base = hostPath();
      void fetchHostList(request, area, base, device, controller.signal)
        .then((list) => { if (!controller.signal.aborted) setEntries(list.map((entry) => ({ key: [base, entry.name].filter(Boolean).join("/"), name: entry.name, directory: entry.directory, size: entry.size, modified: entry.modified, host: true }))); })
        .catch((cause) => { if (!controller.signal.aborted) setError(cause instanceof Error ? cause.message : "unavailable"); })
        .finally(() => { if (!controller.signal.aborted) setLoading(false); });
    }
    return () => controller.abort();
  }, [live, nav, advancedSettings, device, revision]);

  async function storeMutate(method: string, query: Record<string, string>, body?: File) {
    if (!live || busy) return;
    setBusy(true); setError(null);
    try {
      const response = await live.request(`/api/v1/files?${new URLSearchParams(query)}`, { method, credentials: "same-origin", headers: { "Content-Type": "application/octet-stream" }, ...(body ? { body } : {}) });
      if (!response.ok) throw Error(failure(response.status));
      setRevision((value) => value + 1);
    } catch (cause) { setError(cause instanceof Error ? cause.message : "unavailable"); } finally { setBusy(false); }
  }
  async function submitDraft() {
    if (!draft || !isStore || busy || !draft.name.trim()) return;
    if (draft.kind === "newFolder") await storeMutate("POST", { parent: storeParent(), name: draft.name, directory: "true" });
    else if (draft.key) await storeMutate("PUT", { id: draft.key, parent: storeParent(), name: draft.name });
    setDraft(null);
  }
  async function upload(file: File) { if (file.size > 16 * 1024 * 1024) { setError("limit"); return; } await storeMutate("POST", { parent: storeParent(), name: file.name, directory: "false" }, file); }
  async function paste() { if (clipboard && isStore) { await storeMutate("PUT", { id: clipboard.key, parent: storeParent(), name: clipboard.name }); setClipboard(null); } }
  function remove(item: Item) { void storeMutate("DELETE", { id: item.key }); }

  function select(item: Item, event: { ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }) {
    setSelected((previous) => {
      const next = new Set(previous);
      if (event.shiftKey && anchor.current) {
        const names = entries.map((entry) => entry.key);
        const from = names.indexOf(anchor.current), to = names.indexOf(item.key);
        if (from >= 0 && to >= 0) { for (let i = Math.min(from, to); i <= Math.max(from, to); i++) next.add(names[i]!); return next; }
      }
      if (event.ctrlKey || event.metaKey) { if (next.has(item.key)) next.delete(item.key); else next.add(item.key); }
      else { next.clear(); next.add(item.key); }
      anchor.current = item.key;
      return next;
    });
  }

  const visible = useMemo(() => {
    const query = search.trim().toLowerCase();
    return query ? entries.filter((entry) => entry.name.toLowerCase().includes(query)) : entries;
  }, [entries, search]);
  const label = (item: Item) => item.directory ? t("files.folder") : `${item.size.toLocaleString()} B`;
  const selectedItems = entries.filter((entry) => selected.has(entry.key));

  if (nav.place.kind === "host" && !advancedSettings) {
    return <section className="files-app"><p role="status">{t("files.readOnly")}</p></section>;
  }
  return <section className="files-app" onPointerDown={(event) => { if (menu && !(event.target as HTMLElement).closest(".files-menu")) setMenu(null); }}>
    <aside className="files-app__sidebar" aria-label={t("files.locations")}>
      <p className="files-app__heading">{t("files.locations")}</p>
      <button type="button" className={isStore ? "is-active" : ""} aria-current={isStore ? "page" : undefined} onClick={() => go(fresh())}>{t("files.root")}</button>
      {advancedSettings ? <>
        <button type="button" className={nav.place.kind === "host" && nav.place.area === "apps" ? "is-active" : ""} onClick={() => go(host("apps"))}>{t("files.applications")}</button>
        <button type="button" className={nav.place.kind === "host" && nav.place.area === "system" ? "is-active" : ""} onClick={() => go(host("system"))}>{t("files.system")}</button>
      </> : null}
    </aside>
    <div className="files-app__main">
      <div className="files-toolbar" role="toolbar" aria-label={t("files.actions")}>
        <button type="button" disabled={index === 0} onClick={back} aria-label={t("files.back")}>‹</button>
        <button type="button" disabled={index >= history.length - 1} onClick={forward} aria-label={t("files.forward")}>›</button>
        <button type="button" disabled={isStore ? nav.trail.length <= 1 : nav.segments.length === 0} onClick={up} aria-label={t("files.up")}>↑</button>
        <nav className="files-breadcrumb" aria-label={t("files.path")}>
          <button type="button" onClick={() => go(nav.place.kind === "store" ? fresh() : host(nav.place.area))}>{placeLabel(nav.place)}</button>
          {(isStore ? nav.trail.slice(1) : nav.segments).map((crumb, position) => <button key={isStore ? (crumb as StoreCrumb).id : `${crumb}-${position}`} type="button" onClick={() => go(isStore ? { place: { kind: "store" }, trail: nav.trail.slice(0, position + 2), segments: [] } : { place: nav.place, trail: [ROOT], segments: nav.segments.slice(0, position + 1) })}>{isStore ? (crumb as StoreCrumb).name : String(crumb)}</button>)}
        </nav>
        <label className="files-search"><input value={search} onChange={(event) => setSearch(event.target.value)} placeholder={t("files.search")} maxLength={255} /></label>
        <button type="button" aria-pressed={view === "grid"} onClick={() => setView("grid")} aria-label={t("files.grid")}>▦</button>
        <button type="button" aria-pressed={view === "list"} onClick={() => setView("list")} aria-label={t("files.list")}>☰</button>
        {isStore ? <>
          <button type="button" disabled={busy} onClick={() => setDraft({ kind: "newFolder", name: "" })}>{t("files.newFolder")}</button>
          <label className="files-upload">{t("files.upload")}<input type="file" onChange={(event) => { const file = event.target.files?.[0]; event.target.value = ""; if (file) void upload(file); }} /></label>
          {clipboard ? <button type="button" disabled={busy} onClick={() => void paste()}>{t("files.moveHere")}</button> : null}
        </> : <span className="files-readonly">{t("files.readOnly")}</span>}
      </div>
      {draft?.kind === "newFolder" ? <form className="files-draft" onSubmit={(event) => { event.preventDefault(); void submitDraft(); }}><input autoFocus required maxLength={255} value={draft.name} onChange={(event) => setDraft({ ...draft, name: event.target.value })} /><button type="submit">{t("files.save")}</button><button type="button" onClick={() => setDraft(null)}>{t("workspace.cancel")}</button></form> : null}
      {live ? null : <p role="status">{t("files.demo")}</p>}
      {error ? <p role="alert">{t(`files.${["conflict", "limit", "missing"].includes(error) ? (error as "conflict" | "limit" | "missing") : "unavailable"}`)}</p> : null}
      {loading ? <p role="status">{t("files.loading")}</p>
        : visible.length === 0 ? <p className="files-empty">{t("files.empty")}</p>
        : view === "grid" ? <ul className="files-grid">{visible.map((item) => <li key={item.key} className={`files-tile${selected.has(item.key) ? " is-selected" : ""}`}
            onClick={(event) => select(item, event)} onDoubleClick={() => openItem(item)}
            onContextMenu={(event) => { event.preventDefault(); if (!selected.has(item.key)) select(item, { ctrlKey: false, metaKey: false, shiftKey: false }); setMenu({ x: event.clientX, y: event.clientY, key: item.key }); }}>
            <span className="files-tile__icon" aria-hidden="true">{item.directory ? "📁" : "📄"}</span>
            {draft?.kind === "rename" && draft.key === item.key
              ? <input autoFocus defaultValue={item.name} onClick={(event) => event.stopPropagation()} onBlur={(event) => { void storeMutate("PUT", { id: item.key, parent: storeParent(), name: event.target.value }).then(() => setDraft(null)); }} onKeyDown={(event) => { if (event.key === "Enter") (event.target as HTMLInputElement).blur(); if (event.key === "Escape") setDraft(null); }} />
              : <span className="files-tile__name">{item.name}</span>}
          </li>)}</ul>
        : <ul className="file-list files-list">{visible.map((item) => <li key={item.key} className={`files-row${selected.has(item.key) ? " is-selected" : ""}`}
            onClick={(event) => select(item, event)} onDoubleClick={() => openItem(item)}
            onContextMenu={(event) => { event.preventDefault(); if (!selected.has(item.key)) select(item, { ctrlKey: false, metaKey: false, shiftKey: false }); setMenu({ x: event.clientX, y: event.clientY, key: item.key }); }}>
            <span aria-hidden="true">{item.directory ? "📁" : "📄"}</span>
            {draft?.kind === "rename" && draft.key === item.key
              ? <input autoFocus defaultValue={item.name} onClick={(event) => event.stopPropagation()} onBlur={(event) => { void storeMutate("PUT", { id: item.key, parent: storeParent(), name: event.target.value }).then(() => setDraft(null)); }} onKeyDown={(event) => { if (event.key === "Enter") (event.target as HTMLInputElement).blur(); if (event.key === "Escape") setDraft(null); }} />
              : <span className="files-row__name">{item.name}</span>}
            <span className="files-row__meta">{label(item)}</span>
          </li>)}</ul>}
    </div>
    {menu ? (() => {
      const item = entries.find((entry) => entry.key === menu.key);
      if (!item) return null;
      return <div className="files-menu" role="menu" style={{ left: menu.x, top: menu.y }}>
        <button type="button" role="menuitem" onClick={() => { openItem(item); setMenu(null); }}>{t("files.open")}</button>
        {isStore && !item.directory ? null : null}
        {isStore ? <button type="button" role="menuitem" onClick={() => { setDraft({ kind: "rename", key: item.key, name: item.name }); setMenu(null); }}>{t("files.rename")}</button> : null}
        {isStore ? <button type="button" role="menuitem" onClick={() => { setClipboard({ key: item.key, name: item.name }); setMenu(null); }}>{t("files.move")}</button> : null}
        {isStore ? <button type="button" role="menuitem" onClick={() => { remove(item); setMenu(null); }}>{t("files.delete")}</button> : null}
      </div>;
    })() : null}
    {selectedItems.length > 1 ? <p className="visually-hidden" role="status">{`${selectedItems.length}`}</p> : null}
  </section>;
}
