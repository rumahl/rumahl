import { useCallback, useEffect, useRef, useState } from "react";
import type { ShellRequest } from "../../snapshot-client";
import type { ShellApp } from "../../apps/useShellApps";
import { AppIcon } from "../../apps/AppTile";
import type { AppFolder } from "../../preferences/Workspace";
import { ensureDesktopFolder } from "../../shell/desktop-files";
import { fetchHostList, hostContentUrl } from "../fsClient";
import { extensionOf, fileAssociation } from "./fileTypes";
import { freshStore, ROOT, type Item, type Nav } from "./types";

export type ExplorerError = "conflict" | "limit" | "missing" | "unavailable";
export interface OpenError { name: string; extension: string; program?: string }

export interface DesktopSources { apps: readonly ShellApp[]; folders: readonly AppFolder[] }
export interface ExplorerOptions { desktop?: DesktopSources; openApp?: (path: string) => void; desktopKey?: string }

function fail(status: number): ExplorerError {
  return status === 409 ? "conflict" : status === 413 ? "limit" : status === 404 ? "missing" : "unavailable";
}
function mapError(cause: unknown): ExplorerError {
  const message = cause instanceof Error ? cause.message : "";
  return message === "conflict" || message === "limit" || message === "missing" ? message : "unavailable";
}

/**
 * Navigation, listing and personal-store mutations for the file explorer.
 * Host roots are read-only; all writes go to the personal store. The virtual
 * "desktop" place mirrors the OS desktop: app shortcuts, app-collection folders
 * and the real files in the desktop store folder.
 */
export function useExplorer(request: ShellRequest | null, device: string, advanced: boolean, options: ExplorerOptions = {}) {
  const { desktop, openApp, desktopKey } = options;
  // The desktop sources are read through a ref so the listing effect can key on
  // the stable `desktopKey` string instead of the object identity (avoids loops).
  const desktopRef = useRef(desktop);
  desktopRef.current = desktop;
  const [history, setHistory] = useState<Nav[]>(() => [freshStore()]);
  const [index, setIndex] = useState(0);
  const nav = history[Math.min(index, history.length - 1)]!;
  const [entries, setEntries] = useState<Item[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ExplorerError | null>(null);
  const [openError, setOpenError] = useState<OpenError | null>(null);
  const [busy, setBusy] = useState(false);
  const [revision, setRevision] = useState(0);
  const loadedView = useRef("");

  const isStore = nav.place.kind === "store";
  const storeParent = nav.trail.at(-1)?.id ?? "root";

  const go = useCallback((next: Nav) => {
    setHistory((current) => [...current.slice(0, index + 1), next]);
    setIndex((current) => current + 1);
    setError(null);
    setOpenError(null);
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
    if (!request || (nav.place.kind === "host" && !advanced)) {
      setLoading(false);
      return;
    }
    const controller = new AbortController();
    // A revision bump is an automatic background refresh (desktop polling or
    // a completed mutation). Keep the current listing visible and only swap it
    // once the fresh data arrives; only real view changes show the loader.
    const view = `${JSON.stringify(nav)}|${advanced}|${device}|${desktopKey ?? ""}`;
    const background = loadedView.current === view;
    if (!background) {
      setLoading(true);
      setError(null);
      setEntries([]);
    }
    if (nav.place.kind === "store") {
      const parent = nav.trail.at(-1)?.id ?? "root";
      void request(`/api/v1/files?parent=${parent}`, { signal: controller.signal, credentials: "same-origin", cache: "no-store" })
        .then(async (response) => {
          if (!response.ok) throw new Error(fail(response.status));
          const value = (await response.json()) as { id: string; name: string; directory: boolean; size: number }[];
          if (!Array.isArray(value) || value.length > 1000) throw new Error("unavailable");
          if (!controller.signal.aborted) {
            loadedView.current = view;
            setEntries(value.map((file) => ({ key: file.id, name: file.name, directory: file.directory, size: file.size, modified: 0, host: false })));
          }
        })
        .catch((cause) => { if (!controller.signal.aborted && !background) setError(mapError(cause)); })
        .finally(() => { if (!controller.signal.aborted) setLoading(false); });
    } else if (nav.place.kind === "host") {
      const area = nav.place.area;
      const base = nav.segments.join("/");
      const appById = new Map((desktopRef.current?.apps ?? []).map((app) => [app.id, app]));
      void fetchHostList(request, area, base, device, controller.signal)
        .then((list) => {
          if (controller.signal.aborted) return;
          loadedView.current = view;
          setEntries(list.map((entry) => {
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
        .catch((cause) => { if (!controller.signal.aborted && !background) setError(mapError(cause)); })
        .finally(() => { if (!controller.signal.aborted) setLoading(false); });
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
        loadedView.current = view;
        setEntries(virtual);
        setLoading(false);
      } else {
        const filed = new Set((sources?.folders ?? []).flatMap((folderItem) => folderItem.apps));
        for (const folderItem of sources?.folders ?? []) virtual.push({ key: `folder:${folderItem.id}`, name: folderItem.name, directory: true, size: 0, modified: 0, host: false, appFolder: true });
        for (const app of sources?.apps ?? []) if (!filed.has(app.id)) virtual.push({ key: `app:${app.id}`, name: app.title, directory: false, size: 0, modified: 0, host: false, appPath: app.path, icon: <AppIcon app={app} /> });
        void ensureDesktopFolder(request).then(async (id) => {
          let files: Item[] = [];
          if (id) {
            const response = await request(`/api/v1/files?parent=${id}`, { signal: controller.signal, credentials: "same-origin", cache: "no-store" });
            if (response.ok) {
              const value = (await response.json()) as { id: string; name: string; directory: boolean; size: number }[];
              if (Array.isArray(value)) files = value.map((file) => ({ key: file.id, name: file.name, directory: file.directory, size: file.size, modified: 0, host: false }));
            }
          }
          if (!controller.signal.aborted) { loadedView.current = view; setEntries([...virtual, ...files]); setLoading(false); }
        }).catch(() => { if (!controller.signal.aborted) { loadedView.current = view; setEntries(virtual); setLoading(false); } });
      }
    }
    return () => controller.abort();
  }, [request, nav, advanced, device, revision, desktopKey]);

  // Keep the real desktop files in sync while the desktop place is open.
  useEffect(() => {
    if (nav.place.kind !== "desktop") return;
    const timer = setInterval(() => setRevision((value) => value + 1), 5000);
    return () => clearInterval(timer);
  }, [nav.place.kind]);

  const mutate = useCallback(async (method: string, query: Record<string, string>, body?: File) => {
    if (!request || busy) return;
    setBusy(true);
    setError(null);
    try {
      const response = await request(`/api/v1/files?${new URLSearchParams(query)}`, { method, credentials: "same-origin", headers: { "Content-Type": "application/octet-stream" }, ...(body ? { body } : {}) });
      if (!response.ok) throw new Error(fail(response.status));
      setRevision((value) => value + 1);
    } catch (cause) {
      setError(mapError(cause));
    } finally {
      setBusy(false);
    }
  }, [request, busy]);

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
    openApp?.(handler.appPath);
  }, [nav, go, openApp]);

  const download = useCallback((item: Item) => {
    const link = document.createElement("a");
    link.href = item.host && nav.place.kind === "host" ? hostContentUrl(nav.place.area, item.key, device) : `/api/v1/files/content?id=${encodeURIComponent(item.key)}`;
    link.download = item.name;
    link.click();
  }, [nav, device]);

  return {
    nav,
    entries,
    loading,
    error,
    openError,
    clearOpenError: () => setOpenError(null),
    busy,
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
    refresh: () => setRevision((value) => value + 1),
    createFolder: (name: string) => mutate("POST", { parent: storeParent, name, directory: "true" }),
    createFile: (name: string) => mutate("POST", { parent: storeParent, name, directory: "false" }),
    upload: (file: File) => file.size > 16 * 1024 * 1024 ? setError("limit") : mutate("POST", { parent: storeParent, name: file.name, directory: "false" }, file),
    rename: (id: string, name: string) => mutate("PUT", { id, parent: storeParent, name }),
    remove: (id: string) => mutate("DELETE", { id }),
    move: (id: string, name: string) => mutate("PUT", { id, parent: storeParent, name }),
    relocate: (id: string, name: string, parent: string) => mutate("PUT", { id, parent, name }),
  };
}
