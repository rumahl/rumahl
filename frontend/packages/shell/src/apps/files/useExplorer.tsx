import { useCallback, useEffect, useRef, useState } from "react";
import type { ShellRequest } from "../../snapshot-client";
import type { ShellApp } from "../../apps/useShellApps";
import { AppIcon } from "../../apps/AppTile";
import type { AppFolder } from "../../preferences/Workspace";
import { ensureDesktopFolder } from "../../shell/desktop-files";
import {
  createHostEntry, fetchHostList, hostContentUrl, removeHostEntry, renameHostEntry, writeHostFile
} from "../fsClient";
import { extensionOf, fileAssociation } from "./fileTypes";
import { freshStore, ROOT, type Item, type Nav } from "./types";

export type ExplorerError = "conflict" | "folderNotEmpty" | "limit" | "missing" | "unavailable";
export interface OpenError { name: string; extension: string; program?: string }
export type FileOperationKind = "createFolder" | "createFile" | "upload" | "rename" | "move" | "remove";
export interface FileOperation {
  kind: FileOperationKind;
  name: string;
  count: number;
  phase: "pending" | "complete";
}

export interface DesktopSources { apps: readonly ShellApp[]; folders: readonly AppFolder[] }
export interface ExplorerOptions {
  desktop?: DesktopSources;
  openApp?: (path: string) => void;
  desktopKey?: string;
  /** Zero-based page for the personal store (server-side) and local fallbacks. */
  page?: number;
  pageSize?: number;
  /** Name filter; the personal store applies it server-side. */
  search?: string;
}

function fail(status: number): ExplorerError {
  return status === 409 ? "conflict" : status === 413 ? "limit" : status === 404 ? "missing" : "unavailable";
}
function mapError(cause: unknown): ExplorerError {
  const message = cause instanceof Error ? cause.message : "";
  return message === "conflict" || message === "folderNotEmpty" || message === "limit" || message === "missing" ? message : "unavailable";
}

/**
 * Navigation, listing and personal-store mutations for the file explorer.
 * Host roots are read-only; all writes go to the personal store. The virtual
 * "desktop" place mirrors the OS desktop: app shortcuts, app-collection folders
 * and the real files in the desktop store folder.
 */
export function useExplorer(request: ShellRequest | null, device: string, advanced: boolean, options: ExplorerOptions = {}) {
  const { desktop, openApp, desktopKey } = options;
  const page = options.page ?? 0;
  const pageSize = options.pageSize ?? 200;
  const search = options.search ?? "";
  // The desktop sources are read through a ref so the listing effect can key on
  // the stable `desktopKey` string instead of the object identity (avoids loops).
  const desktopRef = useRef(desktop);
  desktopRef.current = desktop;
  const [history, setHistory] = useState<Nav[]>(() => [freshStore()]);
  const [index, setIndex] = useState(0);
  const nav = history[Math.min(index, history.length - 1)]!;
  const [entries, setEntries] = useState<Item[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ExplorerError | null>(null);
  const [openError, setOpenError] = useState<OpenError | null>(null);
  const [busy, setBusy] = useState(false);
  const [operation, setOperation] = useState<FileOperation | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const mutationLock = useRef(false);
  const [revision, setRevision] = useState(0);
  const [desktopFolderId, setDesktopFolderId] = useState<string | null>(null);
  const loadedView = useRef("");

  const isStore = nav.place.kind === "store";
  const isDesktopRoot = nav.place.kind === "desktop" && nav.trail.length === 0;
  // The user's own home is the only writable host area: installed apps and the
  // system tree stay read-only for the shell.
  const isHome = nav.place.kind === "host" && nav.place.area === "home";
  const isWritable = isStore || isDesktopRoot || isHome;
  // Moving entries between folders is a personal-store operation; the host
  // home only renames within a directory.
  const canMove = isStore || isDesktopRoot;
  const storeParent = nav.trail.at(-1)?.id ?? "root";

  const go = useCallback((next: Nav) => {
    setHistory((current) => [...current.slice(0, index + 1), next]);
    setIndex((current) => current + 1);
    setError(null);
    setOpenError(null);
    setOperation(current => current?.phase === "pending" ? current : null);
  }, [index]);
  const back = useCallback(() => setIndex((current) => Math.max(0, current - 1)), []);
  const forward = useCallback(() => setIndex((current) => Math.min(history.length - 1, current + 1)), [history.length]);
  const up = useCallback(() => {
    if (nav.place.kind === "store") {
      if (nav.trail.length > 1) go({ ...nav, trail: nav.trail.slice(0, -1) });
    } else if (nav.place.kind === "host") {
      if (nav.segments.length) go({ ...nav, segments: nav.segments.slice(0, -1) });
    } else if (nav.trail.length) {
      go({ ...nav, trail: nav.trail.slice(0, -1) });
    }
  }, [nav, go]);

  useEffect(() => {
    if (nav.place.kind === "host" && nav.place.area !== "home" && !advanced) {
      // Demoting out of advanced mode must never leave privileged host entries
      // on screen. Rewind to the personal store root without adding history.
      setEntries([]);
      setTotal(0);
      setError(null);
      setOpenError(null);
      setLoading(true);
      loadedView.current = "";
      setHistory([freshStore()]);
      setIndex(0);
      return;
    }
    if (!request) {
      setLoading(false);
      return;
    }
    const controller = new AbortController();
    // A revision bump is an automatic background refresh (desktop polling or
    // a completed mutation). Keep the current listing visible and only swap it
    // once the fresh data arrives; only real view changes show the loader.
    const view = `${JSON.stringify(nav)}|${advanced}|${device}|${desktopKey ?? ""}|${page}|${pageSize}|${search}`;
    const background = loadedView.current === view;
    setRefreshing(background);
    if (!background) {
      setLoading(true);
      setError(null);
      setEntries([]);
      setTotal(0);
    }
    if (nav.place.kind === "store") {
      const parent = nav.trail.at(-1)?.id ?? "root";
      const params = new URLSearchParams({
        parent,
        limit: String(pageSize),
        offset: String(page * pageSize),
      });
      const query = search.trim();
      if (query) params.set("q", query);
      void request(`/api/v1/files?${params}`, { signal: controller.signal, credentials: "same-origin", cache: "no-store" })
        .then(async (response) => {
          if (!response.ok) throw new Error(fail(response.status));
          const value = (await response.json()) as { id: string; name: string; directory: boolean; size: number; children?: number }[];
          if (!Array.isArray(value)) throw new Error("unavailable");
          if (!controller.signal.aborted) {
            loadedView.current = view;
            const header = response.headers?.get("x-total-count") ?? null;
            const parsed = header === null ? Number.NaN : Number(header);
            setTotal(Number.isFinite(parsed) ? parsed : page * pageSize + value.length);
            setEntries(value.map((file) => ({ key: file.id, name: file.name, directory: file.directory, size: file.size, children: file.children ?? 0, modified: 0, host: false })));
          }
        })
        .catch((cause) => { if (!controller.signal.aborted) setError(mapError(cause)); })
        .finally(() => { if (!controller.signal.aborted) { setLoading(false); setRefreshing(false); } });
    } else if (nav.place.kind === "host") {
      const area = nav.place.area;
      const base = nav.segments.join("/");
      const appById = new Map((desktopRef.current?.apps ?? []).map((app) => [app.id, app]));
      void fetchHostList(request, area, base, device, controller.signal)
        .then((list) => {
          if (controller.signal.aborted) return;
          const needle = search.trim().toLowerCase();
          const filtered = needle ? list.filter((entry) => entry.name.toLowerCase().includes(needle)) : list;
          loadedView.current = view;
          setTotal(filtered.length);
          setEntries(filtered.slice(page * pageSize, (page + 1) * pageSize).map((entry) => {
            const app = area === "apps" && !base && entry.directory ? appById.get(entry.name) : undefined;
            return {
              key: [base, entry.name].filter(Boolean).join("/"),
              name: entry.name,
              ...(app ? { label: app.title, app: true, icon: <AppIcon app={app} /> } : {}),
              directory: entry.directory,
              size: entry.size,
              modified: entry.modified,
              host: true,
            };
          }));
        })
        .catch((cause) => { if (!controller.signal.aborted) setError(mapError(cause)); })
        .finally(() => { if (!controller.signal.aborted) { setLoading(false); setRefreshing(false); } });
    } else {
      // Virtual desktop: app shortcuts + app folders + the real desktop files.
      const sources = desktopRef.current;
      const drilled = nav.trail.at(-1)?.id;
      const virtual: Item[] = [];
      if (drilled) {
        const folderItem = sources?.folders.find((item) => item.id === drilled);
        for (const id of folderItem?.apps ?? []) {
          const app = sources?.apps.find((entry) => entry.id === id);
          if (app) virtual.push({ key: `app:${app.id}`, name: app.title, directory: false, size: 0, modified: 0, host: false, appPath: app.path, icon: <AppIcon app={app} /> });
        }
        const needle = search.trim().toLowerCase();
        const filtered = needle ? virtual.filter((entry) => entry.name.toLowerCase().includes(needle)) : virtual;
        loadedView.current = view;
        setTotal(filtered.length);
        setEntries(filtered.slice(page * pageSize, (page + 1) * pageSize));
        setLoading(false);
        setRefreshing(false);
      } else {
        const filed = new Set((sources?.folders ?? []).flatMap((folderItem) => folderItem.apps));
        for (const folderItem of sources?.folders ?? []) virtual.push({ key: `folder:${folderItem.id}`, name: folderItem.name, directory: true, size: 0, children: folderItem.apps.length, modified: 0, host: false, appFolder: true });
        for (const app of sources?.apps ?? []) if (!filed.has(app.id)) virtual.push({ key: `app:${app.id}`, name: app.title, directory: false, size: 0, modified: 0, host: false, appPath: app.path, icon: <AppIcon app={app} /> });
        void ensureDesktopFolder(request).then(async (id) => {
          if (id) setDesktopFolderId(id);
          let files: Item[] = [];
          if (id) {
            const response = await request(`/api/v1/files?parent=${id}&limit=1000&offset=0`, { signal: controller.signal, credentials: "same-origin", cache: "no-store" });
            if (response.ok) {
              const value = (await response.json()) as { id: string; name: string; directory: boolean; size: number; children?: number }[];
              if (Array.isArray(value)) files = value.map((file) => ({ key: file.id, name: file.name, directory: file.directory, size: file.size, children: file.children ?? 0, modified: 0, host: false }));
            }
          }
          if (!controller.signal.aborted) {
            const needle = search.trim().toLowerCase();
            const combined = [...virtual, ...files];
            const filtered = needle ? combined.filter((entry) => entry.name.toLowerCase().includes(needle)) : combined;
            loadedView.current = view;
            setTotal(filtered.length);
            setEntries(filtered.slice(page * pageSize, (page + 1) * pageSize));
            setLoading(false);
            setRefreshing(false);
          }
        }).catch(() => {
          if (!controller.signal.aborted) {
            const needle = search.trim().toLowerCase();
            const filtered = needle ? virtual.filter((entry) => entry.name.toLowerCase().includes(needle)) : virtual;
            loadedView.current = view;
            setTotal(filtered.length);
            setEntries(filtered.slice(page * pageSize, (page + 1) * pageSize));
            setLoading(false);
            setRefreshing(false);
          }
        });
      }
    }
    return () => controller.abort();
  }, [request, nav, advanced, device, revision, desktopKey, page, pageSize, search]);

  // Keep the real desktop files in sync while the desktop place is open.
  useEffect(() => {
    if (nav.place.kind !== "desktop") return;
    const timer = setInterval(() => setRevision((value) => value + 1), 5000);
    return () => clearInterval(timer);
  }, [nav.place.kind]);

  const writeParent = useCallback(async (): Promise<string | null> => {
    if (isStore) return storeParent;
    if (!isDesktopRoot || !request) return null;
    if (desktopFolderId) return desktopFolderId;
    const id = await ensureDesktopFolder(request);
    if (id) setDesktopFolderId(id);
    return id;
  }, [isStore, storeParent, isDesktopRoot, request, desktopFolderId]);

  /** The user's home, relative to it; `null` in every other place. */
  const homePath = nav.place.kind === "host" && nav.place.area === "home" ? nav.segments.join("/") : null;

  const mutateHome = useCallback(async (
    action: { kind: FileOperationKind; name: string; count: number },
    run: (signal: AbortSignal) => Promise<void>,
  ): Promise<boolean> => {
    if (!request || mutationLock.current || !isHome) return false;
    mutationLock.current = true;
    setBusy(true);
    setError(null);
    setOperation({ ...action, phase: "pending" });
    const controller = new AbortController();
    try {
      await run(controller.signal);
      setRevision((value) => value + 1);
      setOperation({ ...action, phase: "complete" });
      return true;
    } catch (cause) {
      setError(mapError(cause));
      setOperation(null);
      return false;
    } finally {
      setBusy(false);
      mutationLock.current = false;
    }
  }, [request, isHome]);

  const mutate = useCallback(async (method: string, query: Record<string, string>, body?: File, kind: FileOperationKind = "rename"): Promise<boolean> => {
    if (!request || mutationLock.current || !isWritable) return false;
    mutationLock.current = true;
    setBusy(true);
    setError(null);
    const action = { kind, name: query.name ?? "", count: 1 };
    setOperation({ ...action, phase: "pending" });
    try {
      const response = await request(`/api/v1/files?${new URLSearchParams(query)}`, { method, credentials: "same-origin", headers: { "Content-Type": "application/octet-stream" }, ...(body ? { body } : {}) });
      if (!response.ok) throw new Error(fail(response.status));
      setRevision((value) => value + 1);
      setOperation({ ...action, phase: "complete" });
      return true;
    } catch (cause) {
      setError(mapError(cause));
      setOperation(null);
      return false;
    } finally {
      setBusy(false);
      mutationLock.current = false;
    }
  }, [request, isWritable]);

  const open = useCallback((item: Item) => {
    setOpenError(null);
    if (nav.place.kind === "desktop") {
      if (item.appFolder) { go({ ...nav, trail: [...nav.trail, { id: item.key.slice("folder:".length), name: item.name }] }); return; }
      if (item.appPath) { openApp?.(item.appPath); return; }
      if (item.directory) { go({ place: { kind: "store" }, trail: [{ id: ROOT.id, name: "" }, { id: item.key, name: item.name }], segments: [] }); return; }
    } else if (item.directory) {
      if (nav.place.kind === "store") go({ ...nav, trail: [...nav.trail, { id: item.key, name: item.name }] });
      else go({ ...nav, segments: [...nav.segments, item.name] });
      return;
    }
    // Opening never downloads. The extension decides which installed program
    // handles the file; unknown or unavailable programs produce a shell message.
    const extension = extensionOf(item.name);
    const handler = fileAssociation(item.name);
    if (!handler) {
      setOpenError({ name: item.name, extension });
      return;
    }
    if (!handler.installed || !handler.appPath) {
      setOpenError({ name: item.name, extension, program: handler.label });
      return;
    }
    // Tell the handler which file to open. Personal files are addressed by id;
    // host files by their read-only root, area and path.
    const params = new URLSearchParams({ name: item.name });
    if (item.host && nav.place.kind === "host") {
      params.set("source", "host");
      params.set("area", nav.place.area);
      params.set("path", item.key);
    } else {
      params.set("id", item.key);
    }
    openApp?.(`${handler.appPath}?${params.toString()}`);
  }, [nav, go, openApp]);

  const download = useCallback((item: Item) => {
    const link = document.createElement("a");
    link.href = item.host && nav.place.kind === "host" ? hostContentUrl(nav.place.area, item.key, device) : `/api/v1/files/content?id=${encodeURIComponent(item.key)}`;
    link.download = item.name;
    link.click();
  }, [nav, device]);

  const removeMany = useCallback(async (ids: readonly string[]): Promise<boolean> => {
    if (!request || ids.length === 0 || mutationLock.current || !isWritable) return false;
    if (isHome) {
      const action = { kind: "remove" as const, name: entries.find((entry) => entry.key === ids[0])?.name ?? "", count: ids.length };
      return mutateHome(action, async (signal) => {
        for (const path of ids) await removeHostEntry(request, path, device, signal);
      });
    }
    mutationLock.current = true;
    setBusy(true);
    setError(null);
    const action = { kind: "remove" as const, name: entries.find(entry => entry.key === ids[0])?.name ?? "", count: ids.length };
    setOperation({ ...action, phase: "pending" });
    let removed = 0;
    try {
      // Serialized rather than fire-and-forget: every delete settles before the
      // next request, so a mid-batch failure leaves a predictable partial state.
      for (const id of ids) {
        const response = await request(`/api/v1/files?${new URLSearchParams({ id })}`, {
          method: "DELETE",
          credentials: "same-origin",
          headers: { "Content-Type": "application/octet-stream" }
        });
        if (!response.ok) {
          // Deleting by id has no name conflict: a 409 on a directory means the
          // folder still has children. Keep the generic conflict copy for files.
          const directory = entries.some((entry) => entry.key === id && entry.directory);
          throw new Error(response.status === 409 && directory ? "folderNotEmpty" : fail(response.status));
        }
        removed += 1;
      }
      setOperation({ ...action, phase: "complete" });
      return true;
    } catch (cause) {
      setError(mapError(cause));
      setOperation(null);
      return false;
    } finally {
      if (removed > 0) setRevision((value) => value + 1);
      setBusy(false);
      mutationLock.current = false;
    }
  }, [request, entries, isWritable]);

  return {
    nav,
    entries,
    total,
    loading,
    error,
    openError,
    clearOpenError: () => setOpenError(null),
    busy,
    operation,
    refreshing,
    isStore,
    storeParent,
    canBack: index > 0,
    canForward: index < history.length - 1,
    canUp: isStore ? nav.trail.length > 1 : nav.place.kind === "host" ? nav.segments.length > 0 : nav.trail.length > 0,
    navigate: go,
    open,
    download,
    back,
    forward,
    up,
    clearError: () => setError(null),
    refresh: () => { setError(null); setOperation(current => current?.phase === "pending" ? current : null); setRevision((value) => value + 1); },
    isWritable,
    canMove,
    createFolder: async (name: string) => {
      if (isHome) {
        return mutateHome({ kind: "createFolder", name, count: 1 }, (signal) =>
          createHostEntry(request!, homePath ?? "", name, true, device, signal));
      }
      const parent = await writeParent();
      if (!parent) { setError("unavailable"); return false; }
      return mutate("POST", { parent, name, directory: "true" }, undefined, "createFolder");
    },
    createFile: async (name: string) => {
      if (isHome) {
        return mutateHome({ kind: "createFile", name, count: 1 }, (signal) =>
          createHostEntry(request!, homePath ?? "", name, false, device, signal));
      }
      const parent = await writeParent();
      if (!parent) { setError("unavailable"); return false; }
      return mutate("POST", { parent, name, directory: "false" }, undefined, "createFile");
    },
    upload: async (file: File) => {
      if (file.size > 16 * 1024 * 1024) return setError("limit");
      if (isHome) {
        const target = [homePath, file.name].filter(Boolean).join("/");
        await mutateHome({ kind: "upload", name: file.name, count: 1 }, async (signal) => {
          await createHostEntry(request!, homePath ?? "", file.name, false, device, signal);
          await writeHostFile(request!, target, file, device, signal);
        });
        return;
      }
      const parent = await writeParent();
      if (!parent) return setError("unavailable");
      await mutate("POST", { parent, name: file.name, directory: "false" }, file, "upload");
    },
    rename: async (id: string, name: string) => {
      if (isHome) {
        await mutateHome({ kind: "rename", name, count: 1 }, (signal) =>
          renameHostEntry(request!, id, name, device, signal));
        return;
      }
      const parent = await writeParent();
      if (!parent) return setError("unavailable");
      await mutate("PUT", { id, parent, name }, undefined, "rename");
    },
    remove: (id: string) => removeMany([id]),
    removeMany,
    move: async (id: string, name: string) => {
      if (!canMove) { setError("unavailable"); return false; }
      const parent = await writeParent();
      if (!parent) { setError("unavailable"); return false; }
      return mutate("PUT", { id, parent, name }, undefined, "move");
    },
    relocate: (id: string, name: string, parent: string) => {
      if (!canMove) { setError("unavailable"); return false; }
      return mutate("PUT", { id, parent, name }, undefined, "move");
    },
  };
}
