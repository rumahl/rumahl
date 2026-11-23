import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { RumahlDialog, useRegisterMobileNav, type MobileIslandNavItem } from "@rumahl/ui/components";
import { CheckIcon, GridIcon, RefreshIcon, SearchIcon, SettingsIcon } from "../../icons";
import { useI18n } from "../../i18n";
import { useOsModePolicy } from "../../preferences/OsMode";
import { browserProfile } from "../../preferences/storage";
import { isMobile } from "../../shell/device";
import { useShell } from "../../shell/ShellContext";
import { useRegisterTopbarActions } from "../../shell/TopbarActions";
import { useAppTopbarHost } from "../../components/ProtectedWindow";
import { useWindowLocation } from "../../routing/window-location";
import { useShellApps } from "../../apps/useShellApps";
import { useWorkspace } from "../../preferences/Workspace";
import { PathBar, type PathCrumb, type PathOption } from "./PathBar";
import { ContextMenu, type MenuItem } from "./ContextMenu";
import { FileGrid, FileList, type ViewProps } from "./FileViews";
import { SearchField } from "./SearchField";
import { Sidebar } from "./Sidebar";
import { Toolbar } from "./Toolbar";
import { Topbar } from "./Topbar";
import { AlertIcon, ArrowLeftIcon, DesktopArtwork, DownloadIcon, FilePlusIcon, FolderArtwork, FolderIcon, GridViewIcon, ListViewIcon, MoreIcon, MoveIcon, OpenIcon, PencilIcon, PlusIcon, TrashIcon, UploadIcon } from "./icons";
import { freshDesktop, freshHost, freshStore, placeLabelKey, ROOT, type Item } from "./types";
import { extensionOf, fileAssociation } from "./fileTypes";
import { fetchHostList, parseRumahlPath, PathResolutionError, resolveRumahlPath, type PathResolutionReason } from "../fsClient";
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
type Menu = { x: number; y: number; item: Item | null; anchor: HTMLElement | null };

const ADDRESS_ERROR_KEYS = {
  invalid: "files.pathInvalid",
  denied: "files.pathDenied",
  missing: "files.pathMissing",
  unavailable: "files.pathUnavailable"
} as const;

/** Page size for the file grid/list. Parsing stays client-side; pagination keeps
 * the DOM bounded without pretending a very large folder is a short one. */
const PAGE_SIZE = 200;

export function FilesApp() {
  const { live, mode, open: openPath } = useShell();
  const { t } = useI18n();
  const mobile = mode === "mobile";
  const topbarHost = useAppTopbarHost();
  const { advancedSettings } = useOsModePolicy();
  const apps = useShellApps();
  const workspace = useWorkspace();
  // Profile discovery touches storage/cookies; do it once, not on every selection.
  const [device] = useState(() => browserProfile().id);
  const desktopSources = useMemo(() => ({ apps, folders: workspace.effective.folders }), [apps, workspace.effective.folders]);
  const desktopKey = useMemo(() => JSON.stringify({ a: apps.map((app) => app.id), f: workspace.effective.folders.map((folderItem) => [folderItem.id, folderItem.apps]) }), [apps, workspace.effective.folders]);
  const [page, setPage] = useState(0);
  const [search, setSearch] = useState("");
  const [query, setQuery] = useState("");
  const [view, setView] = useState<"grid" | "list">(() => {
    try {
      const stored = typeof localStorage === "undefined" ? null : localStorage.getItem("rumahl.files.view");
      if (stored === "grid" || stored === "list") return stored;
    } catch { /* storage may be unavailable */ }
    return isMobile() ? "list" : "grid";
  });
  const chooseView = (next: "grid" | "list") => {
    setView(next);
    try { localStorage.setItem("rumahl.files.view", next); } catch { /* ignore */ }
  };
  const explorer = useExplorer(live ? live.request : null, device, advancedSettings, {
    desktop: desktopSources,
    openApp: openPath,
    desktopKey,
    page,
    pageSize: PAGE_SIZE,
    search: query
  });
  const { selected, select, clear, selectAll, setSelection, prune } = useSelection(explorer.entries);
  const mobileItems = useMemo<MobileIslandNavItem[]>(() => [
    { id: "store", label: t("files.root"), icon: <FolderIcon /> },
    { id: "desktop", label: t("files.desktop"), icon: <DesktopArtwork /> },
    ...(advancedSettings ? [
      { id: "apps", label: t("files.appsNav"), icon: <GridIcon /> },
      { id: "system", label: t("files.system"), icon: <SettingsIcon /> },
    ] : []),
  ], [advancedSettings, t]);
  const mobileValue = explorer.nav.place.kind === "store"
    ? "store"
    : explorer.nav.place.kind === "desktop"
      ? "desktop"
      : explorer.nav.place.kind === "host"
        ? explorer.nav.place.area
        : "";
  useRegisterMobileNav({
    items: mobileItems,
    value: mobileValue,
    label: t("files.locations"),
    onChange: (id) => {
      if (id === "store") explorer.navigate(freshStore());
      else if (id === "desktop") explorer.navigate(freshDesktop());
      else if (id === "apps") explorer.navigate(freshHost("apps"));
      else if (id === "system") explorer.navigate(freshHost("system"));
    }
  });
  const surface = useRef<HTMLDivElement>(null);
  const marquee = useMarquee(surface, selected, setSelection);
  const [renameKey, setRenameKey] = useState<string | null>(null);
  const [clipboard, setClipboard] = useState<{ id: string; name: string } | null>(null);
  const [menu, setMenu] = useState<Menu | null>(null);
  const [deleteKeys, setDeleteKeys] = useState<string[] | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [mobileSearchOpen, setMobileSearchOpen] = useState(false);
  const [mobileMenu, setMobileMenu] = useState<{ x: number; y: number; anchor: HTMLElement } | null>(null);
  const [draggingKey, setDraggingKey] = useState<string | null>(null);
  const [dragOverKey, setDragOverKey] = useState<string | null>(null);
  const [pendingRename, setPendingRename] = useState<string | null>(null);
  const [mounted, setMounted] = useState(false);
  const [compact, setCompact] = useState(false);
  const [addressError, setAddressError] = useState<PathResolutionReason | null>(null);
  const addressGeneration = useRef(0);
  const windowLocation = useWindowLocation();
  const root = useRef<HTMLElement>(null);
  const uploadInput = useRef<HTMLInputElement>(null);
  const sidebarToggle = useRef<HTMLButtonElement>(null);
  const sidebarClose = useRef<HTMLButtonElement>(null);
  const drawer = useRef<HTMLDivElement>(null);
  const wasSidebarOpen = useRef(false);
  const createdSearch = useRef(false);
  useEffect(() => {
    const timer = window.setTimeout(() => setQuery(search), 180);
    return () => window.clearTimeout(timer);
  }, [search]);
  useEffect(() => setMounted(true), []);
  useEffect(() => {
    // The topbar hides its command surface through a container query on the
    // titlebar. Measure that same container — its content box, not the app
    // root — so `compact` can never disagree with what CSS is showing (the
    // 980–1050px dead band).
    const element = topbarHost?.parentElement ?? root.current;
    if (!element) return;
    const style = typeof getComputedStyle === "function" ? getComputedStyle(element) : null;
    const number = (value: string | undefined) => Number.parseFloat(value ?? "") || 0;
    const padding = style ? number(style.paddingLeft) + number(style.paddingRight) : 0;
    const border = style ? number(style.borderLeftWidth) + number(style.borderRightWidth) : 0;
    const measured = () => element.clientWidth > 0
      ? element.clientWidth - padding
      : element.getBoundingClientRect().width - padding - border;
    const update = (width: number) => setCompact(width <= 980);
    update(measured());
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) update(entry.contentRect.width);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [topbarHost]);
  // The OS topbar carries the focused app's quick actions, macOS-style.
  useRegisterTopbarActions(mounted ? [
    { id: "new-folder", label: t("files.newFolder"), disabled: !explorer.isWritable || explorer.busy, onSelect: () => void newFolder() },
    { id: "new-file", label: t("files.newFile"), disabled: !explorer.isWritable || explorer.busy, onSelect: () => void newFile() },
    { id: "upload", label: t("files.upload"), disabled: !explorer.isWritable || explorer.busy, onSelect: () => uploadInput.current?.click() },
    { id: "refresh", label: t("files.refresh"), disabled: explorer.busy, onSelect: explorer.refresh }
  ] : null);

  // A refresh, delete or move can remove entries while they are still selected.
  useEffect(() => { prune(explorer.entries.map((entry) => entry.key)); }, [explorer.entries, prune]);
  useEffect(() => {
    if (sidebarOpen) sidebarClose.current?.focus();
    else if (wasSidebarOpen.current) sidebarToggle.current?.focus();
    wasSidebarOpen.current = sidebarOpen;
  }, [sidebarOpen]);

  useEffect(() => { clear(); setSearch(""); setQuery(""); setRenameKey(null); setPendingRename(null); setMenu(null); setMobileMenu(null); setMobileSearchOpen(false); setSidebarOpen(false); createdSearch.current = false; }, [explorer.nav, clear]);
  useEffect(() => { if (explorer.error) clear(); }, [explorer.error, clear]);

  const addressPath = useMemo(() => {
    const place = explorer.nav.place;
    if (place.kind === "desktop") return "rumahl://desktop";
    if (place.kind === "store") {
      const names = explorer.nav.trail.slice(1).map((crumb) => crumb.name);
      return names.length ? `rumahl://home/${names.join("/")}` : "rumahl://home";
    }
    const segments = explorer.nav.segments;
    return segments.length ? `rumahl://${place.area}/${segments.join("/")}` : `rumahl://${place.area}`;
  }, [explorer.nav]);

  /** Crumbs from the area root to the current folder, for the path bar. */
  const crumbs = useMemo<PathCrumb[]>(() => {
    const place = explorer.nav.place;
    if (place.kind === "desktop") return [{ key: "desktop", label: t("files.desktop") }];
    if (place.kind === "store") return [
      { key: ROOT.id, label: t("files.root") },
      ...explorer.nav.trail.slice(1).map((crumb) => ({ key: crumb.id, label: crumb.name }))
    ];
    return [
      { key: `area:${place.area}`, label: t(placeLabelKey(place)) },
      ...explorer.nav.segments.map((name, index) => ({ key: `${index}:${name}`, label: name }))
    ];
  }, [explorer.nav, t]);

  /** Navigates to the crumb at `index`; the last crumb is where we already are. */
  const navigateCrumb = useCallback((index: number) => {
    const nav = explorer.nav;
    if (nav.place.kind === "desktop") {
      if (index === 0) explorer.navigate(freshDesktop());
      return;
    }
    if (nav.place.kind === "store") {
      if (index < nav.trail.length) explorer.navigate({ ...nav, trail: nav.trail.slice(0, index + 1), segments: [] });
      return;
    }
    explorer.navigate({ ...nav, trail: [ROOT], segments: nav.segments.slice(0, index) });
  }, [explorer]);

  /** The subfolders of the crumb at `index`, for its Windows-style dropdown. */
  const crumbOptions = useCallback(async (index: number): Promise<readonly PathOption[]> => {
    const nav = explorer.nav;
    const request = live?.request;
    if (!request || nav.place.kind === "desktop") return [];
    if (nav.place.kind === "store") {
      const parent = nav.trail[index]?.id ?? "root";
      const response = await request(`/api/v1/files?parent=${encodeURIComponent(parent)}&limit=200&offset=0`, { credentials: "same-origin", cache: "no-store" });
      if (!response.ok) return [];
      const entries = (await response.json()) as { id: string; name: string; directory: boolean }[];
      if (!Array.isArray(entries)) return [];
      return entries
        .filter((entry) => entry.directory)
        .map((entry) => ({
          label: entry.name,
          onSelect: () => explorer.navigate({
            place: { kind: "store" },
            trail: [...nav.trail.slice(0, index + 1), { id: entry.id, name: entry.name }],
            segments: []
          })
        }));
    }
    const parent = nav.segments.slice(0, index).join("/");
    const entries = await fetchHostList(request, nav.place.area, parent, device, new AbortController().signal);
    return entries
      .filter((entry) => entry.directory)
      .map((entry) => ({
        label: entry.name,
        onSelect: () => explorer.navigate({ ...nav, trail: [ROOT], segments: [...nav.segments.slice(0, index), entry.name] })
      }));
  }, [explorer, live, device]);

  // The address line addresses a rumahl area, never a host path. The service
  // resolves it with the same gates as browsing and decides folder vs file.
  const submitAddress = useCallback(async (input: string) => {
    setAddressError(null);
    const parsed = parseRumahlPath(input);
    if (!parsed) {
      setAddressError("invalid");
      return;
    }
    if (!live) {
      setAddressError("unavailable");
      return;
    }
    const generation = ++addressGeneration.current;
    try {
      const target = await resolveRumahlPath(live.request, parsed, device, new AbortController().signal);
      if (generation !== addressGeneration.current) return;
      if (target.directory) {
        if (target.area === "desktop") {
          explorer.navigate(freshDesktop());
        } else if (target.area === "home" && target.trail) {
          // The home and the personal store are one tree: address the path,
          // land in the stable-id store view instead of a second home.
          explorer.navigate({
            place: { kind: "store" },
            trail: [{ id: ROOT.id, name: "" }, ...target.trail.map((entry) => ({ id: entry.id, name: entry.name }))],
            segments: []
          });
        } else {
          explorer.navigate({ place: { kind: "host", area: target.area }, trail: [ROOT], segments: target.relative ? target.relative.split("/") : [] });
        }
        return;
      }
      if (target.area === "home" && target.id) {
        // Personal files open by their stable id, exactly like the list does.
        explorer.open({ key: target.id, name: target.name, directory: false, size: 0, modified: 0, host: false });
        return;
      }
      const extension = extensionOf(target.name);
      const handler = fileAssociation(target.name);
      if (!handler) {
        explorer.reportOpenError({ name: target.name, extension });
        return;
      }
      if (!handler.installed || !handler.appPath) {
        explorer.reportOpenError({ name: target.name, extension, program: handler.label });
        return;
      }
      const params = new URLSearchParams({ name: target.name, source: "host", area: target.area, path: target.relative });
      openPath(`${handler.appPath}?${params.toString()}`);
    } catch (cause) {
      if (generation !== addressGeneration.current) return;
      setAddressError(cause instanceof PathResolutionError ? cause.reason : "unavailable");
    }
  }, [live, device, explorer, openPath]);

  // A deep link (`/app/files?path=rumahl://home/Documents`) opens once, after
  // the live request exists, and never fights later navigation.
  const deepLink = useMemo(() => {
    const search = (windowLocation ?? "").split("?")[1] ?? "";
    return new URLSearchParams(search).get("path");
  }, [windowLocation]);
  const openedDeepLink = useRef<string | null>(null);
  useEffect(() => {
    if (!deepLink || !live || openedDeepLink.current === deepLink) return;
    openedDeepLink.current = deepLink;
    void submitAddress(deepLink);
  }, [deepLink, live, submitAddress]);

  const pageCount = Math.max(1, Math.ceil(explorer.total / PAGE_SIZE));
  const safePage = Math.min(page, pageCount - 1);
  useEffect(() => { setPage(0); }, [explorer.nav, search, view]);
  useEffect(() => {
    // The page fetch clears the total while the next page is loading; do not
    // clamp against that temporary zero or the page would bounce back to 1.
    if (!explorer.loading && page > pageCount - 1) setPage(Math.max(0, pageCount - 1));
  }, [explorer.loading, page, pageCount]);

  function commitRename(item: Item, name: string) {
    setRenameKey(null);
    const trimmed = name.trim();
    if (trimmed && trimmed !== item.name) void explorer.rename(item.key, trimmed);
    if (createdSearch.current) {
      createdSearch.current = false;
      setSearch("");
    }
  }
  useEffect(() => {
    if (!pendingRename) return;
    const item = explorer.entries.find((entry) => entry.name === pendingRename);
    if (!item) return;
    setPage(0);
    select(item, { ctrlKey: false, metaKey: false, shiftKey: false });
    setRenameKey(item.key);
    setPendingRename(null);
  }, [explorer.entries, pendingRename, select]);
  useEffect(() => { if (explorer.error) { setPendingRename(null); createdSearch.current = false; } }, [explorer.error]);
  function uniqueName(base: string, extension: string) {
    const names = new Set(explorer.entries.map((entry) => entry.name.toLocaleLowerCase()));
    let candidate = `${base}${extension}`;
    for (let index = 2; names.has(candidate.toLocaleLowerCase()); index += 1) candidate = `${base} ${index}${extension}`;
    return candidate;
  }
  const newFolder = async () => {
    setSearch("");
    const name = uniqueName(t("files.newFolder"), "");
    if (await explorer.createFolder(name)) {
      createdSearch.current = true;
      setSearch(name);
      setPendingRename(name);
    }
  };
  const newFile = async () => {
    setSearch("");
    const name = uniqueName(t("files.newFile"), ".txt");
    if (await explorer.createFile(name)) {
      createdSearch.current = true;
      setSearch(name);
      setPendingRename(name);
    }
  };

  function openDesktop() {
    explorer.navigate(freshDesktop());
  }

  async function paste() {
    const item = clipboard;
    if (!item || explorer.busy) return;
    if (await explorer.move(item.id, item.name)) setClipboard(current => current === item ? null : current);
  }

  function focusItem(key: string) {
    root.current?.querySelectorAll<HTMLElement>("[data-file-key]").forEach((element) => {
      if (element.dataset.fileKey === key) element.focus({ preventScroll: true });
    });
  }

  function requestDelete(keys: Iterable<string>) {
    if (!explorer.isWritable || explorer.busy) return;
    const allowed = [...new Set(keys)].filter((key) => {
      const item = explorer.entries.find((entry) => entry.key === key);
      return item && !item.appPath && !item.appFolder;
    });
    if (allowed.length) setDeleteKeys(allowed);
  }

  function confirmDelete() {
    const keys = deleteKeys;
    if (!keys) return;
    setDeleteKeys(null);
    void explorer.removeMany(keys).then((removed) => {
      if (!removed) return;
      prune(explorer.entries.filter((entry) => !keys.includes(entry.key)).map((entry) => entry.key));
      root.current?.focus();
    });
  }

  function sidebarKeys(event: React.KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      setSidebarOpen(false);
      return;
    }
    if (event.key !== "Tab") return;
    const focusables = drawer.current?.querySelectorAll<HTMLElement>(
      'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])'
    );
    if (!focusables?.length) return;
    const first = focusables[0]!;
    const last = focusables[focusables.length - 1]!;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  const viewProps: ViewProps = {
    items: explorer.entries,
    selected,
    renameKey,
    draggingKey,
    dragOverKey,
    writable: explorer.isWritable && !explorer.busy,
    hostView: explorer.nav.place.kind === "host",
    openOnTap: mobile,
    onSelect: select,
    onOpen: explorer.open,
    onSelectAll: selectAll,
    onRename: (item) => setRenameKey(item.key),
    onDelete: () => requestDelete(selected),
    onRenameCommit: commitRename,
    onRenameCancel: () => {
      setRenameKey(null);
      if (createdSearch.current) { createdSearch.current = false; setSearch(""); }
    },
    onMenu: (item, x, y, anchor) => {
      if (!selected.has(item.key)) select(item, { ctrlKey: false, metaKey: false, shiftKey: false });
      setMenu({ x, y, item, anchor });
    },
    onDragStart: (item) => setDraggingKey(item.appPath || item.appFolder ? null : item.key),
    onDragEnd: () => { setDraggingKey(null); setDragOverKey(null); },
    onDragOver: (key) => setDragOverKey(key),
    onDropOn: (folder) => {
      if (draggingKey && !folder.appFolder && !folder.appPath) {
        const item = explorer.entries.find((entry) => entry.key === draggingKey);
        if (item && explorer.canMove && !item.appPath && !item.appFolder && item.key !== folder.key) void explorer.relocate(item.key, item.name, folder.key);
      }
      setDraggingKey(null);
      setDragOverKey(null);
    },
  };
  const menuItems = (item: Item | null): MenuItem[] => {
    if (item) {
      return [
        { label: t("files.open"), icon: <OpenIcon />, onSelect: () => explorer.open(item) },
        ...(!item.directory && !item.appPath && !item.appFolder
          ? [{ label: t("files.download"), icon: <DownloadIcon />, onSelect: () => explorer.download(item) }]
          : []),
        ...(explorer.isWritable && !item.appPath && !item.appFolder
          ? [
              { label: t("files.rename"), icon: <PencilIcon />, disabled: explorer.busy, onSelect: () => setRenameKey(item.key) },
              ...(explorer.canMove ? [{ label: t("files.move"), icon: <MoveIcon />, disabled: explorer.busy, onSelect: () => setClipboard({ id: item.key, name: item.name }) }] : []),
              {
                label: t("files.delete"),
                icon: <TrashIcon />,
                danger: true,
                disabled: explorer.busy,
                onSelect: () => requestDelete(selected.has(item.key) ? selected : [item.key])
              },
            ]
          : []),
      ];
    }
    // Empty space: actions for the current folder.
    const items: MenuItem[] = [];
    if (explorer.isWritable) {
      items.push(
        { label: t("files.newFolder"), icon: <PlusIcon />, disabled: explorer.busy, onSelect: newFolder },
        { label: t("files.newFile"), icon: <FilePlusIcon />, disabled: explorer.busy, onSelect: newFile },
        { label: t("files.upload"), icon: <UploadIcon />, disabled: explorer.busy, onSelect: () => uploadInput.current?.click() },
      );
      if (clipboard && explorer.canMove) items.push({ label: t("files.moveHere"), icon: <MoveIcon />, disabled: explorer.busy, onSelect: paste });
    }
    items.push(
      { label: t("files.selectAll"), onSelect: selectAll },
      { label: t("files.refresh"), onSelect: explorer.refresh },
    );
    return items;
  };

  const mobileAtRoot = explorer.nav.place.kind === "desktop"
    ? explorer.nav.trail.length === 0
    : explorer.nav.place.kind === "store"
      ? explorer.nav.trail.length <= 1
      : explorer.nav.segments.length === 0;
  const locationLabel = t(placeLabelKey(explorer.nav.place));
  const mobileTitle = explorer.nav.place.kind === "desktop"
    ? explorer.nav.trail.at(-1)?.name ?? t("files.desktop")
    : explorer.nav.place.kind === "store"
      ? explorer.nav.trail.length > 1 ? explorer.nav.trail.at(-1)!.name : t("files.root")
      : explorer.nav.segments.at(-1) ?? locationLabel;
  const mobileActions: MenuItem[] = [
    ...(explorer.isWritable ? [
      { label: t("files.newFolder"), icon: <PlusIcon />, disabled: explorer.busy, onSelect: newFolder },
      { label: t("files.newFile"), icon: <FilePlusIcon />, disabled: explorer.busy, onSelect: newFile },
      { label: t("files.upload"), icon: <UploadIcon />, disabled: explorer.busy, onSelect: () => uploadInput.current?.click() },
    ] : []),
    {
      label: view === "list" ? t("files.gridView") : t("files.listView"),
      icon: view === "list" ? <GridViewIcon /> : <ListViewIcon />,
      onSelect: () => chooseView(view === "list" ? "grid" : "list")
    },
    ...(clipboard && explorer.canMove ? [{
      label: t("files.moveHere"),
      icon: <MoveIcon />,
      disabled: explorer.busy,
      onSelect: paste
    }] : []),
    { label: t("files.refresh"), icon: <RefreshIcon />, onSelect: explorer.refresh }
  ];

  function handleKeys(event: React.KeyboardEvent) {
    const target = event.target as HTMLElement;
    // Form fields and item/action widgets own their keyboard input. File items
    // run the listbox keyboard model in FileViews; buttons keep native Enter /
    // Space activation instead of opening the selected file.
    if (target.closest("input, textarea, select, [contenteditable='true']")) return;
    if (target.closest("[data-file-key]")) return;
    if (target.closest("button, a, label, [role='menuitem'], [role='button'], [role='option'], [role='row']")) return;
    const single = selected.size === 1 ? explorer.entries.find((item) => selected.has(item.key)) : undefined;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") { event.preventDefault(); selectAll(); return; }
    if (event.key === "Escape") { if (menu) setMenu(null); else clear(); return; }
    if ((event.key === "ArrowDown" || event.key === "ArrowUp" || event.key === "Home" || event.key === "End") && explorer.entries.length) {
      event.preventDefault();
      const keys = explorer.entries.map((item) => item.key);
      const current = single ? keys.indexOf(single.key) : -1;
      const next = event.key === "Home" ? explorer.entries[0]
        : event.key === "End" ? explorer.entries[explorer.entries.length - 1]
          : explorer.entries[event.key === "ArrowDown" ? Math.min(keys.length - 1, current + 1) : Math.max(0, current - 1)];
      if (next) { select(next, { ctrlKey: event.ctrlKey, metaKey: event.metaKey, shiftKey: event.shiftKey }); focusItem(next.key); }
      return;
    }
    if ((event.key === "Enter" || event.key === " ") && single) { event.preventDefault(); explorer.open(single); return; }
    if (!explorer.isWritable || explorer.busy) return;
    if (event.key === "F2" && single && !single.appPath && !single.appFolder) { event.preventDefault(); setRenameKey(single.key); return; }
    if (event.key === "Delete") { event.preventDefault(); requestDelete(selected); }
  }

  const topbar = <Topbar
    nav={explorer.nav} addressPath={addressPath} crumbs={crumbs} advanced={advancedSettings && !mobile} compact={compact} writable={explorer.isWritable} busy={explorer.busy}
    canBack={explorer.canBack} canForward={explorer.canForward} canUp={explorer.canUp}
    canPaste={!!clipboard} view={view} search={search}
    onNavigate={explorer.navigate} onAddress={submitAddress} onNavigateCrumb={navigateCrumb} onCrumbOptions={crumbOptions}
    onBack={explorer.back} onForward={explorer.forward} onUp={explorer.up}
    onView={chooseView} onSearch={setSearch}
    onNewFolder={newFolder} onNewFile={newFile} onUpload={explorer.upload}
    onPaste={paste}
  />;
  const fallbackChrome = compact || !topbarHost;

  return <section ref={root} className="files-app" data-view={view} data-mobile={mobile ? "true" : undefined} tabIndex={-1} onKeyDown={handleKeys}
    onPointerDown={(event) => { if (!(event.target as HTMLElement).closest("[data-file-key], input, textarea, button, label, .files-menu")) root.current?.focus(); }}>
    {mobile ? null : <h1 className="visually-hidden">{t(placeLabelKey(explorer.nav.place))}</h1>}
    <Sidebar nav={explorer.nav} advanced={advancedSettings} onNavigate={explorer.navigate} onOpenDesktop={openDesktop} inert={sidebarOpen} />
    <div className="files-app__main" inert={sidebarOpen}>
      {mobile ? <header className="files-mobile-head">
        <div className="files-mobile-head__row">
          {mobileAtRoot ? null : <button type="button" className="files-mobile-head__back" aria-label={t("files.back")} disabled={!explorer.canBack} onClick={explorer.back}><ArrowLeftIcon /></button>}
          <div className="files-mobile-head__copy">
            <span className="files-mobile-head__context">{mobileAtRoot ? t("files.title") : locationLabel}</span>
            <h1 className="files-mobile-head__title" data-large={mobileAtRoot ? "true" : undefined}>{mobileTitle}</h1>
          </div>
          <button type="button" className={`files-mobile-head__action${mobileSearchOpen ? " is-active" : ""}`} aria-label={t("files.search")} aria-expanded={mobileSearchOpen} onClick={() => {
            if (mobileSearchOpen) { setSearch(""); setMobileSearchOpen(false); }
            else setMobileSearchOpen(true);
          }}><SearchIcon /></button>
          <button type="button" className="files-mobile-head__action" aria-label={t("files.actions")} aria-haspopup="menu" aria-expanded={mobileMenu !== null} onClick={(event) => {
            const bounds = event.currentTarget.getBoundingClientRect();
            setMobileMenu({ x: bounds.right - 8, y: bounds.bottom + 8, anchor: event.currentTarget });
          }}><MoreIcon /></button>
        </div>
        {mobileSearchOpen || search.trim() ? <SearchField className="files-search files-mobile-search" value={search} onChange={setSearch} autoFocus={mobileSearchOpen} /> : null}
        <div className="files-address files-address--mobile">
          <FolderIcon className="files-address__icon" />
          <PathBar path={addressPath} crumbs={crumbs} advanced={false} busy={explorer.busy} onNavigate={navigateCrumb} onOptions={crumbOptions} onSubmit={submitAddress} />
        </div>
      </header> : fallbackChrome ? <>
        <div className="files-address">
          <button ref={sidebarToggle} className="files-sidebar-toggle" type="button" aria-label={t("files.locations")} aria-expanded={sidebarOpen} aria-controls="files-locations-drawer" onClick={() => setSidebarOpen(true)}>
            <FolderIcon /> <span>{t("files.locations")}</span>
          </button>
          <FolderIcon className="files-address__icon" />
          <PathBar path={addressPath} crumbs={crumbs} advanced={advancedSettings} busy={explorer.busy} onNavigate={navigateCrumb} onOptions={crumbOptions} onSubmit={submitAddress} />
        </div>
        <Toolbar
          writable={explorer.isWritable} busy={explorer.busy}
          canBack={explorer.canBack} canForward={explorer.canForward} canUp={explorer.canUp}
          canPaste={!!clipboard} view={view} search={search}
          onBack={explorer.back} onForward={explorer.forward} onUp={explorer.up}
          onView={chooseView} onSearch={setSearch}
          onNewFolder={newFolder} onNewFile={newFile} onUpload={explorer.upload}
          onPaste={paste}
        />
      </> : null}
      {addressError ? <div className="files-alert" role="alert">
        <AlertIcon className="files-alert__icon" />
        <span className="files-alert__text">{t(ADDRESS_ERROR_KEYS[addressError])}</span>
        <button type="button" className="files-alert__close" aria-label={t("files.dismiss")} onClick={() => setAddressError(null)}>×</button>
      </div> : null}
      {explorer.error ? <div className="files-alert" role="alert">
        <AlertIcon className="files-alert__icon" />
        <span className="files-alert__text">{t(`files.${explorer.error}`)}</span>
        <button type="button" className="files-alert__close" aria-label={t("files.dismiss")} onClick={explorer.clearError}>×</button>
      </div> : null}
      {explorer.openError ? <div className="files-alert" role="alert">
        <AlertIcon className="files-alert__icon" />
        <span className="files-alert__text">
          {explorer.openError.program
            ? t("files.programMissing", { name: explorer.openError.name, program: explorer.openError.program })
            : t("files.noAssociation", { name: explorer.openError.name, type: explorer.openError.extension ? `.${explorer.openError.extension}` : t("files.noExtension") })}
        </span>
        <button type="button" className="files-alert__close" aria-label={t("files.dismiss")} onClick={explorer.clearOpenError}>×</button>
      </div> : null}
      <div className="files-surface" ref={surface} aria-busy={explorer.loading || explorer.busy || explorer.refreshing} tabIndex={0}
        {...marquee.handlers}
        onContextMenu={(event) => {
          if ((event.target as HTMLElement).closest("[data-file-key], .files-menu")) return;
          event.preventDefault();
          setMenu({ x: event.clientX, y: event.clientY, item: null, anchor: root.current });
        }}>
        <input ref={uploadInput} type="file" hidden onChange={(event) => { const file = event.target.files?.[0]; event.target.value = ""; if (file) explorer.upload(file); }} />
        {marquee.rect ? <span className="files-marquee" aria-hidden="true" style={{ left: marquee.rect.x, top: marquee.rect.y, width: marquee.rect.width, height: marquee.rect.height }} /> : null}
        {mounted && !live ? <p className="files-demo" role="status">{t("files.demo")}</p> : null}
        {explorer.loading
          ? <div className="files-skeleton" aria-hidden="true">{Array.from({ length: 8 }).map((_, index) => <div key={index} className="files-skeleton__cell" />)}</div>
          : explorer.entries.length === 0 ? <div className="files-empty" role="status" aria-live="polite">
              <span className="files-empty__art" aria-hidden="true"><FolderArtwork /></span>
              <p>{search.trim() ? t("files.noMatches") : t("files.empty")}</p>
            </div>
          : view === "grid" ? <FileGrid {...viewProps} /> : <FileList {...viewProps} />}
      </div>
      <div className="files-status" role="status" aria-live="polite" aria-atomic="true">
        {explorer.operation ? <span className="files-status__operation" data-phase={explorer.operation.phase}>
          {explorer.operation.phase === "pending" ? <RefreshIcon /> : <CheckIcon />}
          <span>{t(`files.operation.${explorer.operation.kind}.${explorer.operation.phase}`, { name: explorer.operation.name, count: explorer.operation.count })}</span>
        </span> : explorer.refreshing ? <span className="files-status__operation" data-phase="pending"><RefreshIcon /><span>{t("files.refreshing")}</span></span> : null}
        {explorer.loading
          ? <span className="files-status__count">{t("files.loading")}</span>
          : <>
              <span className="files-status__count">{t("files.items", { count: explorer.total })}</span>
              {selected.size ? <>
                <span className="files-status__sep">·</span>
                <span className="files-status__selection">{t("files.selectedCount", { count: selected.size })}</span>
              </> : null}
            </>}
      </div>
      {pageCount > 1 ? <nav className="files-pager" aria-label={t("files.pages")}>
        <button type="button" className="files-pager__button" disabled={safePage === 0} onClick={() => setPage(Math.max(0, safePage - 1))}>{t("files.previousPage")}</button>
        <span className="files-pager__status">{t("files.pageOf", { page: safePage + 1, pages: pageCount })}</span>
        <button type="button" className="files-pager__button" disabled={safePage >= pageCount - 1} onClick={() => setPage(Math.min(pageCount - 1, safePage + 1))}>{t("files.nextPage")}</button>
      </nav> : null}
    </div>
    {sidebarOpen ? <div className="files-sidebar-scrim" role="presentation" onClick={() => setSidebarOpen(false)}>
      <div ref={drawer} id="files-locations-drawer" className="files-sidebar-drawer" role="dialog" aria-modal="true" aria-label={t("files.locations")} onClick={(event) => event.stopPropagation()} onKeyDown={sidebarKeys}>
        <header className="files-sidebar-drawer__head">
          <h2 className="files-sidebar-drawer__title">{t("files.locations")}</h2>
          <button ref={sidebarClose} className="files-sidebar-close" type="button" aria-label={t("files.dismiss")} onClick={() => setSidebarOpen(false)}>×</button>
        </header>
        <Sidebar nav={explorer.nav} advanced={advancedSettings} onNavigate={(next) => { explorer.navigate(next); setSidebarOpen(false); }} onOpenDesktop={() => { openDesktop(); setSidebarOpen(false); }} />
      </div>
    </div> : null}
    {menu ? <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.item)} label={t("files.actions")} restoreFocus={menu.anchor} onClose={() => setMenu(null)} /> : null}
    {mobileMenu ? <ContextMenu x={mobileMenu.x} y={mobileMenu.y} items={mobileActions} label={t("files.actions")} restoreFocus={mobileMenu.anchor} onClose={() => setMobileMenu(null)} /> : null}
    {deleteKeys ? <div className="files-delete-confirm" onKeyDown={(event) => event.stopPropagation()}>
      <RumahlDialog
        open
        onClose={() => setDeleteKeys(null)}
        onConfirm={confirmDelete}
        title={t("files.delete")}
        description={deleteKeys.length === 1 ? t("files.confirmDelete") : t("files.confirmDeleteMany", { count: deleteKeys.length })}
        confirmLabel={t("files.delete")}
        cancelLabel={t("files.dismiss")}
        application={t("files.title")}
        destructive
      />
    </div> : null}
    {topbarHost ? createPortal(topbar, topbarHost) : null}
  </section>;
}
