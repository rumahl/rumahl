import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { renderShell, renderStartupFailure } from "./bootstrap";
import { fetchShellSnapshot, type ShellRequest } from "./snapshot-client";

interface ShellRuntimeDependencies {
  render?: (snapshot: ShellSnapshotV1) => void;
  renderFailure?: () => void;
  request?: ShellRequest;
}

export async function startShell({
  render = renderShell,
  renderFailure = renderStartupFailure,
  request = globalThis.fetch
}: ShellRuntimeDependencies = {}): Promise<boolean> {
  try {
    render(await fetchShellSnapshot(request));
    return true;
  } catch {
    renderFailure();
    return false;
  }
}
