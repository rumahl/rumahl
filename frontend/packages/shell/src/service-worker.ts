import { watchFrontendUpdates } from "./frontend-updates";

/** Registration is shared by shell startup and the device settings. */
let pending: Promise<ServiceWorkerRegistration> | undefined;
let activeRegistration: ServiceWorkerRegistration | undefined;
export let serviceWorkerError = "";
export function registerServiceWorker(): Promise<ServiceWorkerRegistration> {
  if (activeRegistration) return Promise.resolve(activeRegistration);
  if (!pending) {
    pending = (async () => {
      if (!window.isSecureContext) throw new Error("HTTPS / localhost required");
      if (!("serviceWorker" in navigator)) throw new Error("Service Worker API unavailable");
      const registration = await navigator.serviceWorker.register("/service-worker.js", { scope: "/", updateViaCache: "none" });
      activeRegistration = registration;
      watchFrontendUpdates(registration);
      serviceWorkerError = "";
      window.dispatchEvent(new Event("rumahl:service-worker"));
      return registration;
    })().then(registration => {
      pending = undefined;
      return registration;
    }, error => {
      pending = undefined;
      serviceWorkerError = error instanceof Error ? error.message : String(error);
      window.dispatchEvent(new Event("rumahl:service-worker"));
      throw error;
    });
  }
  return pending;
}
export function workerDetails(worker: ServiceWorker): Promise<{ version: string; cached: number; total: number }> {
  return new Promise((resolve, reject) => {
    const channel = new MessageChannel();
    const close = () => { clearTimeout(timer); channel.port1.close(); channel.port2.close(); };
    const timer = setTimeout(() => { close(); reject(new Error("Service Worker status timeout")); }, 3000);
    channel.port1.onmessage = event => {
      close();
      const data = event.data;
      if (typeof data?.version !== "string" || !Number.isInteger(data.cached) || !Number.isInteger(data.total)) {
        reject(new Error("Invalid Service Worker status"));
      } else resolve(data);
    };
    try { worker.postMessage({ type: "RUMAHL_STATUS" }, [channel.port2]); }
    catch (error) { close(); reject(error); }
  });
}
