import type { ShellRequest } from "../snapshot-client";

export interface HostEntry {
  name: string;
  directory: boolean;
  size: number;
  modified: number;
}
export type HostArea = "apps" | "system";

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || !value || Array.isArray(value)) throw new Error("invalid host response");
  return value as Record<string, unknown>;
}
export function parseHostList(value: unknown): readonly HostEntry[] {
  const object = record(value);
  if (object.fsVersion !== 1 || (object.area !== "apps" && object.area !== "system") ||
      typeof object.path !== "string" || object.path.length > 4096 ||
      !Array.isArray(object.entries) || object.entries.length > 2000)
    throw new Error("invalid host listing");
  return object.entries.map((value: unknown) => {
    const entry = record(value);
    if (typeof entry.name !== "string" || !entry.name || entry.name.length > 255 ||
        entry.name === "." || entry.name === ".." || entry.name.includes("/") || entry.name.includes("\0") ||
        typeof entry.directory !== "boolean" ||
        typeof entry.size !== "number" || !Number.isInteger(entry.size) || entry.size < 0 ||
        typeof entry.modified !== "number" || !Number.isInteger(entry.modified) || entry.modified < 0)
      throw new Error("invalid host entry");
    return { name: entry.name, directory: entry.directory, size: entry.size, modified: entry.modified };
  });
}
export async function fetchHostList(
  request: ShellRequest,
  area: HostArea,
  path: string,
  device: string,
  signal: AbortSignal,
): Promise<readonly HostEntry[]> {
  const query = new URLSearchParams({ area, path, device });
  const response = await request(`/api/v1/shell/fs?${query.toString()}`, {
    cache: "no-store",
    credentials: "same-origin",
    headers: { Accept: "application/json" },
    signal,
  });
  if (!response.ok) throw new Error("host listing unavailable");
  const text = await response.text();
  if (text.length > 512 * 1024) throw new Error("host response too large");
  return parseHostList(JSON.parse(text));
}
export function hostContentUrl(area: HostArea, path: string, device: string): string {
  const query = new URLSearchParams({ area, path, device });
  return `/api/v1/shell/fs/content?${query.toString()}`;
}
