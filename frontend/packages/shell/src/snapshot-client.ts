import type { ShellSnapshotV1 } from "@rumahl/contracts";

export const SHELL_SNAPSHOT_ENDPOINT = "/api/v1/shell/snapshot";

export async function fetchShellSnapshot(
  request: typeof fetch = globalThis.fetch
): Promise<ShellSnapshotV1> {
  const response = await request(SHELL_SNAPSHOT_ENDPOINT, {
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json" }
  });
  if (!response.ok) {
    throw new Error(`shell snapshot request failed with status ${response.status}`);
  }

  const snapshot: unknown = await response.json();
  if (!isShellSnapshot(snapshot)) {
    throw new Error("shell snapshot response does not match contract v1");
  }
  return snapshot;
}

function isShellSnapshot(value: unknown): value is ShellSnapshotV1 {
  if (!isRecord(value)) return false;
  if (
    value.snapshotVersion !== 1 ||
    value.uiContractVersion !== 1 ||
    value.extensionApiVersion !== 1 ||
    typeof value.shellBuildId !== "string" ||
    typeof value.revision !== "string" ||
    !isRecord(value.user) ||
    typeof value.user.displayName !== "string" ||
    typeof value.user.locale !== "string" ||
    !isRecord(value.theme) ||
    typeof value.theme.stylesheetUrl !== "string" ||
    (value.theme.windowChrome !== "standard" && value.theme.windowChrome !== "compact") ||
    !isSystemStatus(value.systemStatus) ||
    !Array.isArray(value.contributions)
  ) {
    return false;
  }
  return value.contributions.every(isContribution);
}

function isSystemStatus(value: unknown): boolean {
  return (
    isRecord(value) &&
    (value.protection === "active" || value.protection === "attention") &&
    isNonNegativeSafeInteger(value.installedAppCount) &&
    isPositiveSafeInteger(value.observedAtUnixMs) &&
    (value.lastActivityAtUnixMs === null ||
      (isNonNegativeSafeInteger(value.lastActivityAtUnixMs) &&
        value.lastActivityAtUnixMs <= value.observedAtUnixMs))
  );
}

function isContribution(value: unknown): boolean {
  if (!isRecord(value) || typeof value.id !== "string" || typeof value.title !== "string") {
    return false;
  }
  if (value.kind === "command" || value.kind === "search_provider") {
    return typeof value.capability === "string";
  }
  return (
    value.kind === "widget" &&
    value.slot === "dashboard_widgets" &&
    typeof value.entrypoint === "string"
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function isPositiveSafeInteger(value: unknown): value is number {
  return isNonNegativeSafeInteger(value) && value > 0;
}
