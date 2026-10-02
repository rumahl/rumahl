import { StrictMode } from "react";
import { createRoot, hydrateRoot } from "react-dom/client";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { App } from "./App";
import type { ShellLiveSource } from "./live-updates";
import { preloadAppearance } from "./theme";
import "./styles.css";

export function renderShell(snapshot: ShellSnapshotV1, live?: ShellLiveSource): void {
  const root = document.getElementById("root");
  if (!root) {
    throw new Error("rumahl shell root element is missing");
  }
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
  preloadAppearance(snapshot);
  return hydrateRoot(
    root,
    <StrictMode>
      <App live={live} snapshot={snapshot} router={live?.allowDevelopmentLoopback ? "memory" : "browser"} />
    </StrictMode>
  );
}

export function renderStartupFailure(): void {
  const root = document.getElementById("root");
  if (!root) return;
  root.replaceChildren();
  const message = document.createElement("p");
  message.className = "startup-message startup-message--error";
  message.textContent = "rumahl OS could not start. Please try again.";
  root.append(message);
}
