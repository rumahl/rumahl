import { useCallback, useEffect, useState } from "react";
import type { ShellRequest } from "../snapshot-client";

/**
 * The OS desktop is backed by a real personal-store folder (`~/Desktop`). Files
 * placed on the desktop live in `/api/v1/files`, so the Files app and the shell
 * desktop always show the same contents.
 */
export const DESKTOP_FOLDER_NAME = "Desktop";

export interface DesktopFile { id: string; name: string; directory: boolean; size: number }

interface StoreEntry { id: string; name: string; directory: boolean; size: number }

function call(request: ShellRequest, path: string, init?: RequestInit): Promise<Response> {
  return request(path, { credentials: "same-origin", cache: "no-store", ...init });
}

async function list(request: ShellRequest, parent: string): Promise<StoreEntry[]> {
  const response = await call(request, `/api/v1/files?parent=${encodeURIComponent(parent)}`);
  if (!response.ok) throw new Error("unavailable");
  const value: unknown = await response.json();
  return Array.isArray(value) ? value as StoreEntry[] : [];
}

/** Finds the desktop folder, creating it on first use. Returns its id (or null). */
export async function ensureDesktopFolder(request: ShellRequest | null | undefined): Promise<string | null> {
  if (!request) return null;
  try {
    const existing = (await list(request, "root")).find((entry) => entry.directory && entry.name === DESKTOP_FOLDER_NAME);
    if (existing) return existing.id;
    const created = await call(request, `/api/v1/files?parent=root&name=${encodeURIComponent(DESKTOP_FOLDER_NAME)}&directory=true`, { method: "POST" });
    if (!created.ok) return null;
    return (await list(request, "root")).find((entry) => entry.directory && entry.name === DESKTOP_FOLDER_NAME)?.id ?? null;
  } catch { return null; }
}

/** Lists (and stays in sync with) the files placed on the OS desktop. */
export function useDesktopFiles(request: ShellRequest | null | undefined) {
  const [folderId, setFolderId] = useState<string | null>(null);
  const [files, setFiles] = useState<DesktopFile[]>([]);
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    let cancelled = false;
    if (!request) { setFiles([]); return; }
    (async () => {
      const id = await ensureDesktopFolder(request);
      if (cancelled || !id) return;
      setFolderId(id);
      try {
        const entries = await list(request, id);
        if (!cancelled) setFiles(entries.map((entry) => ({ id: entry.id, name: entry.name, directory: entry.directory, size: entry.size })));
      } catch { if (!cancelled) setFiles([]); }
    })();
    return () => { cancelled = true; };
  }, [request, revision]);

  const refresh = useCallback(() => setRevision((value) => value + 1), []);
  const upload = useCallback(async (file: File): Promise<boolean> => {
    if (!request || !folderId) return false;
    const response = await call(request, `/api/v1/files?parent=${folderId}&name=${encodeURIComponent(file.name)}&directory=false`, { method: "POST", headers: { "Content-Type": "application/octet-stream" }, body: file });
    if (response.ok) refresh();
    return response.ok;
  }, [request, folderId, refresh]);
  const remove = useCallback(async (id: string): Promise<void> => {
    if (!request) return;
    await call(request, `/api/v1/files?id=${encodeURIComponent(id)}`, { method: "DELETE" });
    refresh();
  }, [request, refresh]);
  const contentUrl = useCallback((id: string) => `/api/v1/files/content?id=${encodeURIComponent(id)}`, []);

  return { folderId, files, upload, remove, refresh, contentUrl };
}
