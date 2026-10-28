import { useSyncExternalStore } from "react";

export type FrontendUpdatePhase = "idle" | "downloading" | "ready";
export interface FrontendUpdateState {
  phase: FrontendUpdatePhase;
}

const IDLE: FrontendUpdateState = { phase: "idle" };
let state: FrontendUpdateState = IDLE;
let registration: ServiceWorkerRegistration | null = null;
const listeners = new Set<() => void>();
const watchedRegistrations = new WeakSet<object>();
const watchedWorkers = new WeakSet<object>();
let pollStarted = false;
let reloading = false;
let dismissedWorker: ServiceWorker | null = null;

function emit(): void { for (const listener of listeners) listener(); }
function setPhase(phase: FrontendUpdatePhase): void {
  if (state.phase === phase) return;
  state = phase === "idle" ? IDLE : { phase };
  emit();
}

function inspect(target: ServiceWorkerRegistration): void {
  // A waiting worker only means an update when an active worker already
  // controls the page; the very first installation must stay silent.
  if (target.waiting && target.active) {
    if (target.waiting !== dismissedWorker) setPhase("ready");
    else if (state.phase !== "idle") setPhase("idle");
    return;
  }
  const installing = target.installing;
  if (installing) {
    // The install may have completed before the event listener was attached.
    if (installing.state === "installed") { setPhase(target.active ? "ready" : "idle"); return; }
    if (installing.state === "redundant") { setPhase("idle"); return; }
    // Only an update over an existing worker is announced; the first install
    // is part of normal startup.
    if (target.active) setPhase("downloading");
    if (!watchedWorkers.has(installing)) {
      watchedWorkers.add(installing);
      installing.addEventListener("statechange", () => {
        if (installing.state === "installed") setPhase(target.active ? "ready" : "idle");
        else if (installing.state === "redundant") setPhase("idle");
      });
    }
    return;
  }
  if (state.phase !== "ready") setPhase("idle");
}

/**
 * Watches the registration for a new frontend package. The update stays in the
 * worker's waiting slot until the user explicitly reloads.
 */
export function watchFrontendUpdates(target: ServiceWorkerRegistration): void {
  if (!target || typeof target.addEventListener !== "function") return;
  registration = target;
  if (!watchedRegistrations.has(target)) {
    watchedRegistrations.add(target);
    target.addEventListener("updatefound", () => inspect(target));
  }
  inspect(target);
  if (pollStarted || typeof window === "undefined") return;
  pollStarted = true;
  const check = () => {
    if (document.hidden) return;
    const current = registration;
    if (!current) return;
    const update = typeof current.update === "function" ? current.update() : Promise.resolve();
    void update.catch(() => undefined).finally(() => inspect(current));
  };
  window.setInterval(check, 15 * 60_000);
  document.addEventListener("visibilitychange", () => { if (!document.hidden) check(); });
  window.addEventListener("online", check);
}

/** Current update state (also used by tests and diagnostics). */
export function frontendUpdateState(): FrontendUpdateState { return state; }
function getState(): FrontendUpdateState { return state; }
function getServerState(): FrontendUpdateState { return IDLE; }
export function subscribeFrontendUpdate(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}
export function useFrontendUpdate(): FrontendUpdateState {
  return useSyncExternalStore(subscribeFrontendUpdate, getState, getServerState);
}

/** Hides the prompt for this waiting worker; a newer update surfaces again. */
export function dismissFrontendUpdate(): void {
  dismissedWorker = registration?.waiting ?? null;
  setPhase("idle");
}

/** Activates the waiting worker (if any) and reloads once it takes control. */
export function applyFrontendUpdate(): void {
  if (typeof window === "undefined" || reloading) return;
  reloading = true;
  let reloaded = false;
  const reload = () => {
    if (reloaded) return;
    reloaded = true;
    window.location.reload();
  };
  const waiting = registration?.waiting;
  if (!waiting || typeof navigator === "undefined" || !("serviceWorker" in navigator)) { reload(); return; }
  navigator.serviceWorker.addEventListener("controllerchange", reload, { once: true });
  try { waiting.postMessage({ type: "RUMAHL_SKIP_WAITING" }); }
  catch { reload(); return; }
  window.setTimeout(reload, 2_000);
}
