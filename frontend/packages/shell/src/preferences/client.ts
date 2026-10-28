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

export type OsMode = "guided" | "advanced" | "developer";
export function isOsMode(value: unknown): value is OsMode {
  return value === "guided" || value === "advanced" || value === "developer";
}
export interface OsModeSettings {
  settingsVersion: 1;
  ownerId: string;
  revision: number;
  user: { osMode: OsMode };
  device: { osMode: OsMode | null };
  effective: { osMode: OsMode };
}
export interface OsModeUpdate {
  revision: number;
  scope: PreferenceScope;
  mode: OsMode | null;
  password?: string;
}
export function parseOsModeSettings(value: unknown): OsModeSettings {
  if (!value || typeof value !== "object") throw new Error("invalid os mode");
  const s = value as Partial<OsModeSettings>;
  const device = s.device?.osMode ?? null;
  if (s.settingsVersion !== 1 || typeof s.ownerId !== "string" || !/^[0-9a-f-]{36}$/.test(s.ownerId) ||
      !Number.isSafeInteger(s.revision) || s.revision! < 0 || !isOsMode(s.user?.osMode) ||
      (device !== null && !isOsMode(device)) || s.effective?.osMode !== (device ?? s.user.osMode)) {
    throw new Error("invalid os mode");
  }
  return { settingsVersion: 1, ownerId: s.ownerId, revision: s.revision!, user: { osMode: s.user.osMode }, device: { osMode: device }, effective: { osMode: s.effective.osMode } };
}
export async function requestOsMode(request: ShellRequest, device: string, signal: AbortSignal, update?: OsModeUpdate): Promise<OsModeSettings> {
  const response = await request(`/api/v1/shell/os-mode?device=${encodeURIComponent(device)}`, {
    method: update ? "PUT" : "GET", credentials: "same-origin", cache: "no-store", signal,
    headers: update ? { "Content-Type": "application/json", Accept: "application/json" } : { Accept: "application/json" },
    ...(update ? { body: JSON.stringify({ settingsVersion: 1, ...update }) } : {})
  });
  if (!response.ok) throw new PreferencesHttpError(response.status);
  const body = await response.text();
  if (body.length > 4096) throw new Error("os mode too large");
  return parseOsModeSettings(JSON.parse(body));
}
