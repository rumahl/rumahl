import { SHELL_VERSION } from "./version";

/** Demo builds show real identifiers, prefixed so the session is unmistakable. */
export function osName(demo: boolean): string {
  return demo ? "rumahl OS Demo" : "rumahl OS";
}

export function buildLabel(buildId: string, demo: boolean): string {
  return demo ? `DEMO-${buildId}` : buildId;
}

export function versionLabel(demo: boolean): string {
  return demo ? `DEMO-${SHELL_VERSION}` : SHELL_VERSION;
}
