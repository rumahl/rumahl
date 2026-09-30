import { isInternalTarget } from "./internal";
/** Route IDs are navigation inputs, never proof of installation or permission. */
export const APP_ID = /^[A-Za-z0-9][A-Za-z0-9._-]{0,254}$/;
export function appPath(id: string, segments: readonly string[] = []): string {
  const invalidSegment = segments.some((part) =>
    !part || part === "." || part === ".." || part.includes("/") || part.includes("\\") ||
    Array.from(part).some((char) => char.charCodeAt(0) < 32)
  );
  if (!APP_ID.test(id) || invalidSegment) {
    throw new Error("invalid app route");
  }
  return `/app/${encodeURIComponent(id)}${segments.map((part) => `/${encodeURIComponent(part)}`).join("")}`;
}
export type { ShellMode } from "../preferences/client";
export function localPath(target: string): string {
  if (isInternalTarget(target)) return `/#${target}`;
  if (!target.startsWith("/") || target.startsWith("//") || target.includes("\\")) {
    throw new Error("shell navigation must be local");
  }
  const url = new URL(target, "https://shell.invalid");
  if (url.origin !== "https://shell.invalid") throw new Error("shell navigation must be local");
  // Retire old links without letting URL parameters override persisted preferences.
  url.searchParams.delete("mode");
  return url.pathname + url.search + url.hash;
}
