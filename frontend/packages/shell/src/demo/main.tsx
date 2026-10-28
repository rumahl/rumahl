import { startShell } from "../runtime";
import { DemoBackend } from "./backend";

declare const __RUMAHL_DEMO_WIDGET_ORIGIN__: string;

const widgetOrigin = __RUMAHL_DEMO_WIDGET_ORIGIN__ || window.location.origin;
const backend = new DemoBackend({ widgetOrigin });

// Demo app frames run in an opaque sandbox and cannot use localStorage. They
// ask the shell to cache small app values instead, so a demo note survives a
// reload without giving the app access to the shell origin.
const APP_STORE_PREFIX = "rumahl.demo.app-store.v1:";
type StoreRequest = { type: "rumahl-demo-store-request"; id: number; op: "get" | "set"; key: string; value?: string };
window.addEventListener("message", (event: MessageEvent<unknown>) => {
  const data = event.data as Partial<StoreRequest> | null;
  if (!data || data.type !== "rumahl-demo-store-request" || typeof data.id !== "number" ||
      (data.op !== "get" && data.op !== "set") || typeof data.key !== "string") return;
  const key = `${APP_STORE_PREFIX}${data.key.slice(0, 128)}`;
  let value: string | null = null;
  try {
    if (data.op === "set" && typeof data.value === "string" && data.value.length <= 200_000) {
      localStorage.setItem(key, data.value);
      value = data.value;
    } else {
      value = localStorage.getItem(key);
    }
  } catch { /* demo continues without durable app storage */ }
  (event.source as Window | null)?.postMessage({ type: "rumahl-demo-store-response", id: data.id, value }, "*");
});

await startShell({
  request: backend.request,
  openEvents: backend.openEvents,
  allowDevelopmentLoopback: true,
  demo: { frameOrigin: widgetOrigin },
  renderFailure: () => {
    const root = document.getElementById("root");
    if (!root) return;
    root.replaceChildren();
    const message = document.createElement("p");
    message.className = "startup-message startup-message--error";
    message.textContent = "The demo could not start. Reload the page to try again.";
    root.append(message);
  }
});
