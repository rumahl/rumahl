import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { parseShellSnapshot } from "@rumahl/contracts/parse";
import { SHELL_BUILD_ID } from "./build-id";

export const EMBEDDED_SNAPSHOT_ID = "rumahl-shell-snapshot";

export function readEmbeddedShellSnapshot(): ShellSnapshotV1 | null {
  const element = document.getElementById(EMBEDDED_SNAPSHOT_ID);
  if (!element) return null;

  try {
    if (element.tagName !== "SCRIPT" || element.getAttribute("type") !== "application/json") {
      throw new Error("invalid embedded shell snapshot element");
    }
    const snapshot = parseShellSnapshot(JSON.parse(element.textContent ?? ""));
    if (snapshot.shellBuildId !== SHELL_BUILD_ID) {
      throw new Error("incompatible shell build");
    }
    return snapshot;
  } finally {
    element.remove();
  }
}
