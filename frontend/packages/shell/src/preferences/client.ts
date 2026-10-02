import type { ShellRequest } from "../snapshot-client";
export type ShellMode = "desktop" | "launcher";
export type PreferenceScope = "user" | "device";
export type PreferenceKey = "shell.mode" | "shell.theme";
const THEME_ID = /^[a-z][a-z0-9-]*(?:\.[a-z][a-z0-9-]*){2,}$/;
export function isThemeId(value: unknown): value is string {
  return typeof value === "string" && value.length <= 128 && THEME_ID.test(value);
}
export interface Preferences {
  settingsVersion: 1;
  ownerId: string;
  revision: number;
  user: { shellMode: ShellMode; shellTheme: string };
  device: { shellMode: ShellMode | null; shellTheme: string | null };
  effective: { shellMode: ShellMode; shellTheme: string };
}
export interface PreferenceUpdate {
  revision: number;
  scope: PreferenceScope;
  key: PreferenceKey;
  value: ShellMode | string | null;
}
export function isMode(value: unknown): value is ShellMode { return value === "desktop" || value === "launcher"; }
export function parsePreferences(value: unknown): Preferences {
  if (!value || typeof value !== "object") throw new Error("invalid preferences");
  const p = value as Partial<Preferences>;
  if (p.settingsVersion !== 1 || typeof p.ownerId !== "string" || !/^[0-9a-f-]{36}$/.test(p.ownerId) ||
      !Number.isSafeInteger(p.revision) || p.revision! < 0 || !isMode(p.user?.shellMode) || !isThemeId(p.user?.shellTheme) ||
      (p.device?.shellMode !== null && !isMode(p.device?.shellMode)) ||
      (p.device?.shellTheme !== null && !isThemeId(p.device?.shellTheme)) ||
      p.effective?.shellMode !== (p.device?.shellMode ?? p.user.shellMode) ||
      p.effective?.shellTheme !== (p.device?.shellTheme ?? p.user.shellTheme)) throw new Error("invalid preferences");
  return { settingsVersion: 1, ownerId: p.ownerId, revision: p.revision!, user: { shellMode: p.user.shellMode, shellTheme: p.user.shellTheme }, device: { shellMode: p.device!.shellMode, shellTheme: p.device!.shellTheme }, effective: { shellMode: p.effective.shellMode, shellTheme: p.effective.shellTheme } };
}
export class PreferencesHttpError extends Error {
  constructor(readonly status: number) { super("preferences request failed"); }
}
export async function requestPreferences(request: ShellRequest, device: string, signal: AbortSignal, update?: PreferenceUpdate): Promise<Preferences> {
  const response = await request(`/api/v1/shell/preferences?device=${encodeURIComponent(device)}`, {
    method: update ? "PUT" : "GET", credentials: "same-origin", cache: "no-store", signal,
    headers: update ? { "Content-Type": "application/json", Accept: "application/json" } : { Accept: "application/json" },
    ...(update ? { body: JSON.stringify({ settingsVersion: 1, ...update }) } : {})
  });
  if (!response.ok) throw new PreferencesHttpError(response.status);
  const body = await response.text();
  if (body.length > 4096) throw new Error("preferences too large");
  return parsePreferences(JSON.parse(body));
}
