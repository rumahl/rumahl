import type { ShellRequest } from "../snapshot-client";
export type ShellMode = "desktop" | "launcher";
export type PreferenceScope = "user" | "device";
export interface Preferences {
  settingsVersion: 1;
  ownerId: string;
  revision: number;
  user: { shellMode: ShellMode };
  device: { shellMode: ShellMode | null };
  effective: { shellMode: ShellMode };
}
export function isMode(value: unknown): value is ShellMode { return value === "desktop" || value === "launcher"; }
export function parsePreferences(value: unknown): Preferences {
  if (!value || typeof value !== "object") throw new Error("invalid preferences");
  const p = value as Partial<Preferences>;
  if (p.settingsVersion !== 1 || typeof p.ownerId !== "string" || !/^[0-9a-f-]{36}$/.test(p.ownerId) ||
      !Number.isSafeInteger(p.revision) || p.revision! < 0 || !isMode(p.user?.shellMode) ||
      (p.device?.shellMode !== null && !isMode(p.device?.shellMode)) ||
      p.effective?.shellMode !== (p.device?.shellMode ?? p.user.shellMode)) throw new Error("invalid preferences");
  return { settingsVersion: 1, ownerId: p.ownerId, revision: p.revision!, user: { shellMode: p.user.shellMode }, device: { shellMode: p.device!.shellMode }, effective: { shellMode: p.effective.shellMode } };
}
export class PreferencesHttpError extends Error {
  constructor(readonly status: number) { super("preferences request failed"); }
}
export async function requestPreferences(request: ShellRequest, device: string, signal: AbortSignal, update?: { revision: number; scope: PreferenceScope; value: ShellMode | null }): Promise<Preferences> {
  const response = await request(`/api/v1/shell/preferences?device=${encodeURIComponent(device)}`, {
    method: update ? "PUT" : "GET", credentials: "same-origin", cache: "no-store", signal,
    headers: update ? { "Content-Type": "application/json", Accept: "application/json" } : { Accept: "application/json" },
    ...(update ? { body: JSON.stringify({ settingsVersion: 1, key: "shell.mode", ...update }) } : {})
  });
  if (!response.ok) throw new PreferencesHttpError(response.status);
  const body = await response.text();
  if (body.length > 4096) throw new Error("preferences too large");
  return parsePreferences(JSON.parse(body));
}
