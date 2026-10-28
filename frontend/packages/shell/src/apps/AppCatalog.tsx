import { createContext, useContext, useEffect, useRef, useState, type PropsWithChildren } from "react";
import type { ShellLiveSource } from "../live-updates";
import { fetchCatalog, type InstalledApp } from "./client";

/** Display-only app from the server snapshot (no launch credentials). */
export interface CatalogPreviewApp {
  id: string;
  title: string;
  launchable: boolean;
}

interface CatalogState {
  apps: readonly InstalledApp[];
  preview: readonly CatalogPreviewApp[];
  status: "loading" | "ready" | "unavailable";
}

const Context = createContext<CatalogState>({ apps: [], preview: [], status: "loading" });

export function AppCatalog({ children, live, revision, initial }: PropsWithChildren<{
  live: ShellLiveSource | undefined;
  revision: string;
  initial?: readonly CatalogPreviewApp[] | undefined;
}>) {
  const preview = useRef(initial ?? []);
  const [state, setState] = useState<CatalogState>({ apps: [], preview: preview.current, status: "loading" });
  useEffect(() => {
    if (!live) { setState({ apps: [], preview: preview.current, status: "unavailable" }); return; }
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const apps = await fetchCatalog(live!.request, controller.signal);
        if (!controller.signal.aborted) setState({ apps, preview: preview.current, status: "ready" });
      } catch {
        if (!controller.signal.aborted) setState({ apps: [], preview: preview.current, status: "unavailable" });
      } finally {
        if (!controller.signal.aborted) timer = setTimeout(() => { void refresh(); }, 30_000);
      }
    }
    void refresh();
    return () => { controller.abort(); clearTimeout(timer); };
  }, [live, revision]);
  return <Context value={state}>{children}</Context>;
}
export function useAppCatalog() { return useContext(Context); }
