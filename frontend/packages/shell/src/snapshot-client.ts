import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { parseShellSnapshot } from "@rumahl/contracts/parse";

export const SHELL_SNAPSHOT_ENDPOINT = "/api/v1/shell/snapshot";
export type ShellRequest = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

export class ShellSnapshotHttpError extends Error {
  constructor(readonly status: number) {
    super(`shell snapshot request failed with status ${status}`);
  }
}

export async function fetchShellSnapshot(
  request: ShellRequest = globalThis.fetch
): Promise<ShellSnapshotV1> {
  const response = await request(SHELL_SNAPSHOT_ENDPOINT, {
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json" }
  });
  if (!response.ok) {
    throw new ShellSnapshotHttpError(response.status);
  }

  try {
    return parseShellSnapshot(await response.json());
  } catch {
    throw new Error("shell snapshot response does not match contract v1");
  }
}
