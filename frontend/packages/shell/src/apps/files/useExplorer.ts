import { useCallback, useEffect, useState } from "react";
import type { ShellRequest } from "../../snapshot-client";
import { fetchHostList, hostContentUrl } from "../fsClient";
import { fileAssociation } from "./fileTypes";
import { freshStore, type Item, type Nav } from "./types";

export type ExplorerError = "conflict" | "limit" | "missing" | "unavailable";

function fail(status: number): ExplorerError {
  return status === 409 ? "conflict" : status === 413 ? "limit" : status === 404 ? "missing" : "unavailable";
}
function mapError(cause: unknown): ExplorerError {
  const message = cause instanceof Error ? cause.message : "";
  return message === "conflict" || message === "limit" || message === "missing" ? message : "unavailable";
}

/**
 * Navigation, listing and personal-store mutations for the file explorer.
 * Host roots are read-only; all writes go to the personal store.
 */
export function useExplorer(request: ShellRequest | null, device: string, advanced: boolean) {
  const [history, setHistory] = useState<Nav[]>(() => [freshStore()]);
  const [index, setIndex] = useState(0);
  const nav = history[Math.min(index, history.length - 1)]!;
  const [entries, setEntries] = useState<Item[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ExplorerError | null>(null);
  const [busy, setBusy] = useState(false);
  const [revision, setRevision] = useState(0);

  const isStore = nav.place.kind === "store";
  const storeParent = nav.trail.at(-1)?.id ?? "root";

  const go = useCallback((next: Nav) => {
    setHistory((current) => [...current.slice(0, index + 1), next]);
    setIndex((current) => current + 1);
    setError(null);
  }, [index]);
  const back = useCallback(() => setIndex((current) => Math.max(0, current - 1)), []);
  const forward = useCallback(() => setIndex((current) => Math.min(history.length - 1, current + 1)), [history.length]);
  const up = useCallback(() => {
    if (isStore) {
      if (nav.trail.length > 1) go({ ...nav, trail: nav.trail.slice(0, -1) });
    } else if (nav.segments.length) {
      go({ ...nav, segments: nav.segments.slice(0, -1) });
    }
  }, [isStore, nav, go]);

  useEffect(() => {
    if (!request || (nav.place.kind === "host" && !advanced)) {
      setLoading(false);
      return;
    }
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    setEntries([]);
    if (nav.place.kind === "store") {
      const parent = nav.trail.at(-1)?.id ?? "root";
      void request(`/api/v1/files?parent=${parent}`, { signal: controller.signal, credentials: "same-origin", cache: "no-store" })
        .then(async (response) => {
          if (!response.ok) throw new Error(fail(response.status));
          const value = (await response.json()) as { id: string; name: string; directory: boolean; size: number }[];
          if (!Array.isArray(value) || value.length > 1000) throw new Error("unavailable");
          if (!controller.signal.aborted) setEntries(value.map((file) => ({ key: file.id, name: file.name, directory: file.directory, size: file.size, modified: 0, host: false })));
        })
        .catch((cause) => { if (!controller.signal.aborted) setError(mapError(cause)); })
        .finally(() => { if (!controller.signal.aborted) setLoading(false); });
    } else {
      const area = nav.place.area;
      const base = nav.segments.join("/");
      void fetchHostList(request, area, base, device, controller.signal)
        .then((list) => { if (!controller.signal.aborted) setEntries(list.map((entry) => ({ key: [base, entry.name].filter(Boolean).join("/"), name: entry.name, directory: entry.directory, size: entry.size, modified: entry.modified, host: true }))); })
        .catch((cause) => { if (!controller.signal.aborted) setError(mapError(cause)); })
        .finally(() => { if (!controller.signal.aborted) setLoading(false); });
    }
    return () => controller.abort();
  }, [request, nav, advanced, device, revision]);

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
    if (item.directory) {
      if (nav.place.kind === "store") go({ ...nav, trail: [...nav.trail, { id: item.key, name: item.name }] });
      else go({ ...nav, segments: [...nav.segments, item.name] });
      return;
    }
    // Which program opens a file is decided by its extension (fileTypes.ts).
    // Viewer/editor programs are the next step: when one is associated, it is
    // launched here with the file. Until then every file is downloaded, so
    // nothing is lost.
    const handler = fileAssociation(item.name);
    if (handler.kind !== "download") {
      // TODO: launch `handler.id` with this file once such programs are installed.
    }
    const link = document.createElement("a");
    link.href = item.host && nav.place.kind === "host" ? hostContentUrl(nav.place.area, item.key, device) : `/api/v1/files/content?id=${encodeURIComponent(item.key)}`;
    link.download = item.name;
    link.click();
  }, [nav, go, device]);

  return {
    nav,
    entries,
    loading,
    error,
    busy,
    isStore,
    storeParent,
    canBack: index > 0,
    canForward: index < history.length - 1,
    canUp: isStore ? nav.trail.length > 1 : nav.segments.length > 0,
    navigate: go,
    open,
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
