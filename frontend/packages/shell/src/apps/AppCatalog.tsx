import { createContext, useContext, useEffect, useState, type PropsWithChildren } from "react";
import type { ShellLiveSource } from "../live-updates";
import { fetchCatalog, type InstalledApp } from "./client";
interface CatalogState { apps: readonly InstalledApp[]; status: "loading" | "ready" | "unavailable" }
const Context = createContext<CatalogState>({ apps: [], status: "loading" });
export function AppCatalog({ children, live, revision }: PropsWithChildren<{ live: ShellLiveSource | undefined; revision: string }>) {
  const [state, setState] = useState<CatalogState>({ apps: [], status: "loading" });
  useEffect(() => {
    if (!live) { setState({ apps: [], status: "unavailable" }); return; }
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const apps = await fetchCatalog(live!.request, controller.signal);
        if (!controller.signal.aborted) setState({ apps, status: "ready" });
      } catch {
        if (!controller.signal.aborted) setState({ apps: [], status: "unavailable" });
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
