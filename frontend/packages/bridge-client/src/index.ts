/**
 * App-side client for the rumahl OS bridge (v1).
 *
 * Third-party apps run in a sandboxed, opaque-origin iframe. This client speaks
 * the `rumahl.bridge.v1` postMessage protocol to the shell that hosts the app:
 * it performs the `hello`/`welcome` handshake and exposes typed helpers.
 *
 * Usage (inside an installed app):
 * ```ts
 * import { connectRumahlBridge } from "@rumahl/bridge-client";
 * const rumahl = connectRumahlBridge();
 * await rumahl.ready();
 * await rumahl.notify({ message: "Synced", variant: "success" });
 * rumahl.window.minimize();
 * rumahl.on("os.theme.changed", (theme) => applyTheme(theme));
 * ```
 *
 * The client is framework-agnostic and safe when the page is opened outside
 * rumahl OS: `available` is `false` and every call rejects.
 */
import {
  BRIDGE_PROVIDER_METHOD,
  RUMAHL_BRIDGE,
  parseBridgeMessage,
  type BridgeAppInfo,
  type BridgeCapabilityResource,
  type BridgeCapabilityResult,
  type BridgeNotification,
  type BridgeProviderInvocation,
  type BridgeRequest,
  type BridgeResponse,
  type BridgeTheme
} from "@rumahl/contracts/bridge";

export type {
  BridgeAppInfo,
  BridgeCapabilityResource,
  BridgeCapabilityResult,
  BridgeNotification,
  BridgeProviderInvocation,
  BridgeTheme
} from "@rumahl/contracts/bridge";

export class RumahlBridgeError extends Error {
  constructor(readonly code: string, message: string) {
    super(message);
    this.name = "RumahlBridgeError";
  }
}

export type BridgeEventHandler = (payload: unknown) => void;

/** Handles a capability invocation delivered to this app as a provider. */
export type BridgeProviderHandler = (invocation: BridgeProviderInvocation) => unknown | Promise<unknown>;

export interface RumahlBridge {
  /** True when the app is embedded in the shell (a parent frame exists). */
  readonly available: boolean;
  /** The OS app id reported by the shell, once the handshake completed. */
  readonly appId: string | null;
  /** Resolves after the shell acknowledged the handshake. */
  ready(): Promise<{ appId: string; methods: readonly string[] }>;
  /** Sends an arbitrary bridge method. Prefer the typed helpers below. */
  call<T = unknown>(method: string, params?: unknown): Promise<T>;
  info(): Promise<BridgeAppInfo>;
  theme(): Promise<BridgeTheme>;
  notify(input: BridgeNotification): Promise<void>;
  /** Invokes a capability provided by another installed app, on the user's behalf. */
  invokeCapability(capability: string, resource?: BridgeCapabilityResource): Promise<BridgeCapabilityResult>;
  /** Registers a handler for a capability this app provides. Returns an unregister. */
  provide(capability: string, handler: BridgeProviderHandler): () => void;
  readonly window: {
    close(): Promise<void>;
    minimize(): Promise<void>;
    focus(): Promise<void>;
  };
  /** Subscribes to an OS event topic (for example `os.theme.changed`). */
  on(topic: string, listener: BridgeEventHandler): () => void;
  /** Removes listeners and rejects pending calls. */
  dispose(): void;
}

export interface ConnectOptions {
  /** Message target. Defaults to `window.parent`. Overridable for tests. */
  target?: Window;
  /** Per-request timeout in ms. Defaults to 5000. */
  timeoutMs?: number;
  /** Handshake retry interval in ms. Defaults to 600. */
  handshakeRetryMs?: number;
}

const DEFAULT_TIMEOUT_MS = 5000;
const DEFAULT_RETRY_MS = 600;

function normalizeProviderInvocation(value: unknown): BridgeProviderInvocation | null {
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  if (typeof record.capability !== "string" || record.capability.length === 0 || record.capability.length > 200) return null;
  const invocation: BridgeProviderInvocation = { capability: record.capability };
  if (record.resource !== undefined && record.resource !== null) {
    if (typeof record.resource !== "object") return null;
    const resource = record.resource as Record<string, unknown>;
    if (typeof resource.namespace !== "string" || typeof resource.kind !== "string" || typeof resource.key !== "string") return null;
    invocation.resource = { namespace: resource.namespace, kind: resource.kind, key: resource.key };
  }
  return invocation;
}

export function connectRumahlBridge(options: ConnectOptions = {}): RumahlBridge {
  const self = typeof window !== "undefined" ? window : undefined;
  const target = options.target ?? self?.parent;
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const retryMs = options.handshakeRetryMs ?? DEFAULT_RETRY_MS;
  const embedded = target !== undefined && target !== self;

  let sequence = 0;
  let welcome: { appId: string; methods: readonly string[] } | null = null;
  const pending = new Map<string, { resolve: (value: unknown) => void; reject: (error: Error) => void; timer: ReturnType<typeof setTimeout> }>();
  const listeners = new Map<string, Set<BridgeEventHandler>>();
  const providers = new Map<string, BridgeProviderHandler>();

  let settleReady: (value: { appId: string; methods: readonly string[] }) => void = () => undefined;
  let failReady: (error: Error) => void = () => undefined;
  const readyPromise = new Promise<{ appId: string; methods: readonly string[] }>((resolve, reject) => {
    settleReady = resolve;
    failReady = reject;
  });
  // A disposer may reject `ready()`; keep that from becoming an unhandled
  // rejection for apps that never awaited it.
  void readyPromise.catch(() => undefined);

  const post = (message: unknown): void => {
    if (!target) return;
    try { target.postMessage(message, "*"); } catch { /* the parent navigated away */ }
  };
  const hello = (): void => post({ bridge: RUMAHL_BRIDGE, kind: "hello" });

  const respond = (id: string, ok: boolean, extra: Partial<BridgeResponse>): void => {
    const message: BridgeResponse = { bridge: RUMAHL_BRIDGE, kind: "response", id, ok, ...extra };
    post(message);
  };
  async function handleProviderRequest(message: BridgeRequest): Promise<void> {
    if (message.method !== BRIDGE_PROVIDER_METHOD) {
      respond(message.id, false, { error: { code: "unknown-method", message: `unknown method: ${message.method}` } });
      return;
    }
    const invocation = normalizeProviderInvocation(message.params);
    if (!invocation) {
      respond(message.id, false, { error: { code: "invalid-params", message: "a capability id is required" } });
      return;
    }
    const handler = providers.get(invocation.capability);
    if (!handler) {
      respond(message.id, false, { error: { code: "no-provider", message: `not provided: ${invocation.capability}` } });
      return;
    }
    try {
      respond(message.id, true, { result: await handler(invocation) });
    } catch (error) {
      respond(message.id, false, { error: { code: "failed", message: error instanceof Error ? error.message : String(error) } });
    }
  }

  const onMessage = (event: MessageEvent): void => {
    // The shell's origin is opaque, so bind on the exact parent window instead.
    if (event.source !== target) return;
    const message = parseBridgeMessage(event.data);
    if (!message) return;
    if (message.kind === "welcome") {
      welcome = { appId: message.appId, methods: message.methods };
      stopHandshake();
      settleReady(welcome);
      return;
    }
    if (message.kind === "request") {
      void handleProviderRequest(message);
      return;
    }
    if (message.kind === "response") {
      const entry = pending.get(message.id);
      if (!entry) return;
      pending.delete(message.id);
      clearTimeout(entry.timer);
      if (message.ok) entry.resolve(message.result);
      else entry.reject(new RumahlBridgeError(message.error?.code ?? "failed", message.error?.message ?? "the shell rejected the request"));
      return;
    }
    if (message.kind === "event") {
      for (const listener of listeners.get(message.topic) ?? []) {
        try { listener(message.payload); } catch { /* a listener must not break the channel */ }
      }
    }
  };

  let handshake: ReturnType<typeof setInterval> | undefined;
  function startHandshake(): void {
    if (!embedded) return;
    hello();
    handshake = setInterval(() => { if (!welcome) hello(); }, retryMs);
  }
  function stopHandshake(): void {
    if (handshake !== undefined) { clearInterval(handshake); handshake = undefined; }
  }

  function request<T>(method: string, params?: unknown): Promise<T> {
    if (!embedded) return Promise.reject(new RumahlBridgeError("not-embedded", "This app is not running inside rumahl OS."));
    const id = `${Date.now().toString(36)}-${(++sequence).toString(36)}`;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new RumahlBridgeError("timeout", `The shell did not answer ${method}.`));
      }, timeoutMs);
      pending.set(id, { resolve: resolve as (value: unknown) => void, reject, timer });
      const message: BridgeRequest = { bridge: RUMAHL_BRIDGE, kind: "request", id, method };
      if (params !== undefined) message.params = params;
      post(message);
    });
  }

  if (typeof window !== "undefined") window.addEventListener("message", onMessage);
  if (embedded) startHandshake();

  return {
    available: embedded,
    get appId() { return welcome?.appId ?? null; },
    ready: () => readyPromise,
    call: request,
    info: () => request<BridgeAppInfo>("os.info"),
    theme: () => request<BridgeTheme>("os.theme.get"),
    notify: async (input) => { await request("os.notification", input); },
    invokeCapability: (capability, resource) =>
      request<BridgeCapabilityResult>("os.capabilities.invoke", resource ? { capability, resource } : { capability }),
    provide: (capability, handler) => {
      providers.set(capability, handler);
      return () => { if (providers.get(capability) === handler) providers.delete(capability); };
    },
    window: {
      close: async () => { await request("os.window.close"); },
      minimize: async () => { await request("os.window.minimize"); },
      focus: async () => { await request("os.window.focus"); }
    },
    on: (topic, listener) => {
      const set = listeners.get(topic) ?? new Set<BridgeEventHandler>();
      set.add(listener);
      listeners.set(topic, set);
      return () => { set.delete(listener); if (set.size === 0) listeners.delete(topic); };
    },
    dispose: () => {
      stopHandshake();
      if (typeof window !== "undefined") window.removeEventListener("message", onMessage);
      for (const entry of pending.values()) { clearTimeout(entry.timer); entry.reject(new RumahlBridgeError("disposed", "The bridge was disposed.")); }
      pending.clear();
      listeners.clear();
      if (!welcome) failReady(new RumahlBridgeError("disposed", "The bridge was disposed before it was ready."));
    }
  };
}
