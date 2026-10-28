import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { hydrateShell, renderShell, renderStartupFailure } from "./bootstrap";
import { readEmbeddedShellSnapshot } from "./embedded-snapshot";
import { SHELL_BUILD_ID } from "./build-id";
import { browserEventSocket, type OpenEvents, type ShellLiveSource } from "./live-updates";
import { fetchShellSnapshot, type ShellRequest } from "./snapshot-client";

interface ShellRuntimeDependencies {
  render?: (snapshot: ShellSnapshotV1, live: ShellLiveSource) => void;
  hydrate?: (snapshot: ShellSnapshotV1, live: ShellLiveSource) => void;
  readEmbedded?: () => ShellSnapshotV1 | null;
  renderFailure?: () => void;
  request?: ShellRequest;
  openEvents?: OpenEvents;
  allowDevelopmentLoopback?: boolean;
}

export async function startShell({
  render = renderShell,
  hydrate = hydrateShell,
  readEmbedded = readEmbeddedShellSnapshot,
  renderFailure = renderStartupFailure,
  request = globalThis.fetch,
  openEvents = browserEventSocket,
  allowDevelopmentLoopback = false
}: ShellRuntimeDependencies = {}): Promise<boolean> {
  try {
    const live = { request, openEvents, allowDevelopmentLoopback };
    const embedded = readEmbedded();
    if (embedded) {
      hydrate(embedded, live);
      return true;
    }
    const snapshot = await fetchShellSnapshot(request);
    if (snapshot.shellBuildId !== SHELL_BUILD_ID) {
      throw new Error("incompatible shell build");
    }
    render(snapshot, live);
    return true;
  } catch {
    renderFailure();
    return false;
  }
}
