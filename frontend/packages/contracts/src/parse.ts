import type { ExtensionContribution, ShellSnapshotV1 } from "./v1";

const MAX_SNAPSHOT_BYTES = 256 * 1024;
const OPAQUE_ID = /^[A-Za-z0-9._-]+$/;
const LOCALE = /^[a-z]{2,3}(?:-(?:[A-Z]{2}|[A-Za-z0-9]{4,8}))*$/;
const STYLESHEET = /^\/shell\/themes\/sha256-[0-9a-f]{64}\.css$/;
const SIMPLE_ID = /^[a-z][a-z0-9-]*$/;

export function parseShellSnapshot(value: unknown): ShellSnapshotV1 {
  let serialized: string;
  try {
    serialized = JSON.stringify(value);
  } catch {
    throw new Error("invalid shell snapshot");
  }
  if (!serialized || new TextEncoder().encode(serialized).byteLength > MAX_SNAPSHOT_BYTES) {
    throw new Error("invalid shell snapshot");
  }

  if (
    !hasKeys(value, [
      "snapshotVersion", "uiContractVersion", "extensionApiVersion", "shellBuildId",
      "revision", "user", "theme", "systemStatus", "contributions"
    ]) ||
    value.snapshotVersion !== 1 ||
    value.uiContractVersion !== 1 ||
    value.extensionApiVersion !== 1 ||
    !validOpaqueId(value.shellBuildId) ||
    !validOpaqueId(value.revision) ||
    !validUser(value.user) ||
    !validTheme(value.theme) ||
    !validSystemStatus(value.systemStatus) ||
    !Array.isArray(value.contributions) ||
    value.contributions.length > 512 ||
    !value.contributions.every(validContribution)
  ) {
    throw new Error("invalid shell snapshot");
  }
  const ids = value.contributions.map((item: ExtensionContribution) => item.id);
  if (new Set(ids).size !== ids.length) {
    throw new Error("invalid shell snapshot");
  }
  return value as unknown as ShellSnapshotV1;
}

function validUser(value: unknown): boolean {
  return hasKeys(value, ["displayName", "locale"]) &&
    validTitle(value.displayName, 128) &&
    typeof value.locale === "string" && LOCALE.test(value.locale);
}

function validTheme(value: unknown): boolean {
  return hasKeys(value, ["stylesheetUrl", "windowChrome"]) &&
    typeof value.stylesheetUrl === "string" && STYLESHEET.test(value.stylesheetUrl) &&
    (value.windowChrome === "standard" || value.windowChrome === "compact");
}

function validSystemStatus(value: unknown): boolean {
  return hasKeys(value, ["protection", "installedAppCount", "observedAtUnixMs", "lastActivityAtUnixMs"]) &&
    (value.protection === "active" || value.protection === "attention") &&
    nonNegativeInteger(value.installedAppCount) && value.installedAppCount <= 0xffffffff &&
    nonNegativeInteger(value.observedAtUnixMs) && value.observedAtUnixMs > 0 &&
    (value.lastActivityAtUnixMs === null ||
      (nonNegativeInteger(value.lastActivityAtUnixMs) && value.lastActivityAtUnixMs <= value.observedAtUnixMs));
}

function validContribution(value: unknown): boolean {
  if (!isRecord(value) || !validNamespacedId(value.id) || !validTitle(value.title, 128)) return false;
  if (value.kind === "command" || value.kind === "search_provider") {
    return hasKeys(value, ["kind", "id", "title", "capability"]) && validNamespacedId(value.capability);
  }
  return value.kind === "widget" &&
    hasKeys(value, ["kind", "id", "title", "slot", "entrypoint"]) &&
    value.slot === "dashboard_widgets" && validSimpleId(value.entrypoint);
}

function validOpaqueId(value: unknown): boolean {
  return typeof value === "string" && value.length > 0 && value.length <= 128 && OPAQUE_ID.test(value);
}

function validNamespacedId(value: unknown): boolean {
  return typeof value === "string" && value.length <= 192 && value.split(".").length >= 3 &&
    value.split(".").every(validSimpleId);
}

function validSimpleId(value: unknown): boolean {
  return typeof value === "string" && value.length > 0 && value.length <= 64 &&
    SIMPLE_ID.test(value) && !value.endsWith("-");
}

function validTitle(value: unknown, maxLength: number): boolean {
  return typeof value === "string" && value.trim().length > 0 &&
    Array.from(value).length <= maxLength && !Array.from(value).some((character) => {
      const codePoint = character.codePointAt(0)!;
      return codePoint <= 31 || (codePoint >= 127 && codePoint <= 159);
    });
}

function nonNegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function hasKeys(value: unknown, expected: readonly string[]): value is Record<string, unknown> {
  return isRecord(value) && Object.keys(value).length === expected.length &&
    expected.every((key) => Object.hasOwn(value, key));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
