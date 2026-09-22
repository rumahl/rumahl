import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { App } from "./App";
import "./styles.css";

export function renderShell(snapshot: ShellSnapshotV1): void {
  const root = document.getElementById("root");
  if (!root) {
    throw new Error("rumahl shell root element is missing");
  }

  createRoot(root).render(
    <StrictMode>
      <App snapshot={snapshot} />
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
