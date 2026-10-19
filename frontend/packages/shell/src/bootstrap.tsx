import { StrictMode } from "react";
import { createRoot, hydrateRoot } from "react-dom/client";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { App } from "./App";
import type { ShellLiveSource } from "./live-updates";
import { preloadAppearance } from "./theme";
import { loadInstalledThemes } from "./preferences/installed-themes";
import "@fontsource-variable/space-grotesk";
import "@fontsource-variable/bricolage-grotesque";
import "@fontsource/instrument-serif/400.css";
import "@fontsource-variable/outfit";
import "@fontsource-variable/sora";
import "@fontsource-variable/syne";
import "@fontsource-variable/unbounded";
import "@fontsource-variable/fraunces";
import "@fontsource-variable/playfair-display";
import "@fontsource/dm-serif-display";
import "@fontsource-variable/gabarito";
import "@fontsource-variable/manrope";
import "@fontsource-variable/plus-jakarta-sans";
import "./fonts.css";
import "./styles.css";

/**
 * Install the offline kit. The worker can distinguish "this device is offline"
 * from "rumahl OS is unreachable" and can serve the recovery screen for failed
 * navigations. Best-effort: the OS works without it.
 */
function registerServiceWorker(): void {
  if (typeof navigator === "undefined" || !("serviceWorker" in navigator)) return;
  const host = location.hostname;
  const secure = location.protocol === "https:" || host === "localhost" || host === "127.0.0.1";
  if (!secure) return;
  navigator.serviceWorker.register("/service-worker.js", { scope: "/" }).catch(() => { /* optional */ });
}

export function renderShell(snapshot: ShellSnapshotV1, live?: ShellLiveSource): void {
  const root = document.getElementById("root");
  if (!root) {
    throw new Error("rumahl shell root element is missing");
  }
  registerServiceWorker();
  // Register installed custom themes before the first paint resolves the theme.
  loadInstalledThemes();
  preloadAppearance(snapshot);

  createRoot(root).render(
    <StrictMode>
      <App live={live} snapshot={snapshot} router={live?.allowDevelopmentLoopback ? "memory" : "browser"} />
    </StrictMode>
  );
}

export function hydrateShell(snapshot: ShellSnapshotV1, live?: ShellLiveSource): ReturnType<typeof hydrateRoot> {
  const root = document.getElementById("root");
  if (!root || root.dataset.shellSsr !== "1" || !root.hasChildNodes()) {
    throw new Error("server-rendered shell root is missing");
  }
  registerServiceWorker();
  // Register installed custom themes before the first paint resolves the theme.
  loadInstalledThemes();
  preloadAppearance(snapshot);
  return hydrateRoot(
    root,
    <StrictMode>
      <App live={live} snapshot={snapshot} router={live?.allowDevelopmentLoopback ? "memory" : "browser"} />
    </StrictMode>
  );
}

export function renderStartupFailure(): void {
  // The OS could not start: hand over to the standalone recovery system, which
  // shows what happened and returns to the shell once it is healthy again.
  if (typeof window !== "undefined") {
    window.location.assign("/recovery?reason=boot");
    return;
  }
  const root = document.getElementById("root");
  if (!root) return;
  root.replaceChildren();
  const message = document.createElement("p");
  message.className = "startup-message startup-message--error";
  message.textContent = "rumahl OS could not start. Recovery is at /recovery.";
  root.append(message);
}
