import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { hydrateShell, renderShell, renderStartupFailure } from "./bootstrap";
import { readEmbeddedShellSnapshot } from "./embedded-snapshot";
import { SHELL_BUILD_ID } from "./build-id";
import { browserEventSocket, type OpenEvents, type ShellLiveSource } from "./live-updates";
import { fetchShellSnapshot, type ShellRequest } from "./snapshot-client";
import { finishSplashAfterPaint, splashCopy, splashFail, splashProgress, splashSetLocale } from "./splash";
import { waitForUiStyles } from "./ui-ready";

interface ShellRuntimeDependencies {
  render?: (snapshot: ShellSnapshotV1, live: ShellLiveSource) => void;
  hydrate?: (snapshot: ShellSnapshotV1, live: ShellLiveSource) => void;
  readEmbedded?: () => ShellSnapshotV1 | null;
  renderFailure?: () => void;
  request?: ShellRequest;
  openEvents?: OpenEvents;
  allowDevelopmentLoopback?: boolean;
  demo?: { frameOrigin: string } | undefined;
}

function rememberLocale(locale: string): void {
  try {
    localStorage.setItem("rumahl.locale", locale);
  } catch {
    // Storage can be disabled; the splash still uses the document language.
  }
}

export async function startShell({
  render = renderShell,
  hydrate = hydrateShell,
  readEmbedded = readEmbeddedShellSnapshot,
  renderFailure = renderStartupFailure,
  request = globalThis.fetch,
  openEvents = browserEventSocket,
  allowDevelopmentLoopback = false,
  demo
}: ShellRuntimeDependencies = {}): Promise<boolean> {
  if (typeof window !== "undefined") {
    try {
      const url = new URL(window.location.href);
      if (url.searchParams.has("rumahl-shell")) {
        url.searchParams.delete("rumahl-shell");
        window.history.replaceState(null, "", url);
      }
    } catch {
      // A sandboxed frame may forbid history changes; the shell still boots.
    }
  }
  try {
    splashProgress(8, splashCopy("boot"));
    const uiReady = waitForUiStyles();
    const live = { request, openEvents, allowDevelopmentLoopback, ...(demo ? { demo } : {}) };
    const embedded = readEmbedded();
    if (embedded) {
      rememberLocale(embedded.user.locale);
      splashSetLocale(embedded.user.locale);
      splashProgress(45, splashCopy("preparing"));
      hydrate(embedded, live);
      await uiReady;
      finishSplashAfterPaint();
      return true;
    }
    splashProgress(18, splashCopy("loadingData"));
    const snapshot = await fetchShellSnapshot(request);
    if (snapshot.shellBuildId !== SHELL_BUILD_ID) {
      throw new Error("incompatible shell build");
    }
    rememberLocale(snapshot.user.locale);
    splashSetLocale(snapshot.user.locale);
    splashProgress(58, splashCopy("dataLoaded"));
    splashProgress(72, splashCopy("buildingDesktop"));
    render(snapshot, live);
    await uiReady;
    finishSplashAfterPaint();
    return true;
  } catch {
    splashFail(splashCopy("failed"));
    renderFailure();
    return false;
  }
}
