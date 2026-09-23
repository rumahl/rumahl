export const SHELL_EVENT_VERSION = 1 as const;

export type ShellEventV1 =
  | { eventVersion: 1; kind: "snapshot_changed"; revision: string }
  | { eventVersion: 1; kind: "session_revoked" };

const REVISION = /^[A-Za-z0-9._-]{1,128}$/;

export function parseShellEvent(data: string): ShellEventV1 {
  if (data.length > 1024) throw new Error("shell event too large");
  const value: unknown = JSON.parse(data);
  if (!isRecord(value) || value.eventVersion !== SHELL_EVENT_VERSION) {
    throw new Error("invalid shell event");
  }
  if (value.kind === "session_revoked" && Object.keys(value).length === 2) {
    return value as ShellEventV1;
  }
  if (value.kind === "snapshot_changed" && Object.keys(value).length === 3 &&
      typeof value.revision === "string" && REVISION.test(value.revision)) {
    return value as ShellEventV1;
  }
  throw new Error("invalid shell event");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
