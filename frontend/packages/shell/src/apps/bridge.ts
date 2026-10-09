import {
  BRIDGE_METHODS,
  RUMAHL_BRIDGE,
  RUMAHL_BRIDGE_VERSION,
  capabilityForMethod,
  parseBridgeMessage,
  type BridgeCapabilityResource,
  type BridgeCapabilityResult,
  type BridgeEvent,
  type BridgeMessage,
  type BridgeNotification,
  type BridgeRequest,
  type BridgeResponse,
  type BridgeTheme,
  type BridgeWindowAction
} from "@rumahl/contracts/bridge";

/** Everything the bridge needs from the shell realm to serve one app window. */
export interface AppBridgeHandlers {
  appId: string;
  appTitle: string;
  version: string;
  accountName: string;
  mode: "desktop" | "launcher";
  /** Permission ids the app declared. A method is refused unless its capability is listed. */
  capabilities: readonly string[];
  theme(): BridgeTheme;
  notify(notification: BridgeNotification): void;
  windowControl(action: BridgeWindowAction): void;
  /** Invokes a capability provided by another installed app, on behalf of the user. */
  invokeCapability(capability: string, resource?: BridgeCapabilityResource): Promise<BridgeCapabilityResult>;
}

/**
 * Builds the response for one app request. Pure, so it can be unit-tested
 * without a DOM. A method is refused unless the app declared its capability;
 * unknown methods fail closed.
 */
export function handleBridgeRequest(message: BridgeRequest, handlers: AppBridgeHandlers): BridgeResponse {
  const respond = (ok: boolean, extra: Partial<BridgeResponse>): BridgeResponse => ({
    bridge: RUMAHL_BRIDGE,
    kind: "response",
    id: message.id,
    ok,
    ...extra
  });
  const capability = capabilityForMethod(message.method);
  if (capability === null) return respond(false, { error: { code: "unknown-method", message: `unknown method: ${message.method}` } });
  if (!handlers.capabilities.includes(capability)) return respond(false, { error: { code: "denied", message: `missing capability: ${capability}` } });
  try {
    switch (message.method) {
      case "os.info":
        return respond(true, {
          result: {
            app: { id: handlers.appId, title: handlers.appTitle, version: handlers.version },
            account: { displayName: handlers.accountName },
            mode: handlers.mode
          }
        });
      case "os.theme.get":
        return respond(true, { result: handlers.theme() });
      case "os.notification": {
        const notification = normalizeNotification(message.params);
        if (!notification) return respond(false, { error: { code: "invalid-params", message: "a non-empty message is required" } });
        handlers.notify(notification);
        return respond(true, { result: { shown: true } });
      }
      case "os.window.close":
        handlers.windowControl("close");
        return respond(true, { result: null });
      case "os.window.minimize":
        handlers.windowControl("minimize");
        return respond(true, { result: null });
      case "os.window.focus":
        handlers.windowControl("focus");
        return respond(true, { result: null });
      default:
        return respond(false, { error: { code: "unknown-method", message: `unknown method: ${message.method}` } });
    }
  } catch (error) {
    return respond(false, { error: { code: "failed", message: error instanceof Error ? error.message : String(error) } });
  }
}

function normalizeNotification(value: unknown): BridgeNotification | null {
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  if (typeof record.message !== "string" || record.message.length === 0 || record.message.length > 512) return null;
  const notification: BridgeNotification = { message: record.message };
  if (typeof record.title === "string" && record.title.length > 0 && record.title.length <= 128) notification.title = record.title;
  if (record.variant === "success" || record.variant === "warning" || record.variant === "info" || record.variant === "error") notification.variant = record.variant;
  return notification;
}

/**
 * Handles a request, awaiting capability invocations. Synchronous methods
 * resolve immediately; `os.capabilities.invoke` calls the OS on the user's
 * behalf and may take a round-trip.
 */
export async function handleBridgeRequestAsync(message: BridgeRequest, handlers: AppBridgeHandlers): Promise<BridgeResponse> {
  if (message.method !== "os.capabilities.invoke") return handleBridgeRequest(message, handlers);
  const respond = (ok: boolean, extra: Partial<BridgeResponse>): BridgeResponse => ({
    bridge: RUMAHL_BRIDGE,
    kind: "response",
    id: message.id,
    ok,
    ...extra
  });
  const capability = capabilityForMethod(message.method);
  if (capability === null) return respond(false, { error: { code: "unknown-method", message: `unknown method: ${message.method}` } });
  if (!handlers.capabilities.includes(capability)) return respond(false, { error: { code: "denied", message: `missing capability: ${capability}` } });
  const invocation = normalizeCapabilityInvocation(message.params);
  if (!invocation) return respond(false, { error: { code: "invalid-params", message: "a capability id is required" } });
  try {
    return respond(true, { result: await handlers.invokeCapability(invocation.capability, invocation.resource) });
  } catch (error) {
    return respond(false, { error: { code: "failed", message: error instanceof Error ? error.message : String(error) } });
  }
}

function normalizeCapabilityInvocation(value: unknown): { capability: string; resource?: BridgeCapabilityResource } | null {
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  if (typeof record.capability !== "string" || record.capability.length === 0 || record.capability.length > 200) return null;
  const invocation: { capability: string; resource?: BridgeCapabilityResource } = { capability: record.capability };
  if (record.resource !== undefined && record.resource !== null) {
    const resource = normalizeCapabilityResource(record.resource);
    if (!resource) return null;
    invocation.resource = resource;
  }
  return invocation;
}

function normalizeCapabilityResource(value: unknown): BridgeCapabilityResource | null {
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  if (typeof record.namespace !== "string" || record.namespace.length === 0 || record.namespace.length > 128) return null;
  if (typeof record.kind !== "string" || record.kind.length === 0 || record.kind.length > 128) return null;
  if (typeof record.key !== "string" || record.key.length === 0 || record.key.length > 256) return null;
  return { namespace: record.namespace, kind: record.kind, key: record.key };
}

/**
 * Binds the bridge to one app iframe. The channel is bound to the concrete
 * `iframe.contentWindow` because the sandboxed app has an opaque origin and
 * `event.origin` is always `"null"`. Returns a disposer.
 */
export function attachAppBridge(iframe: HTMLIFrameElement, handlers: AppBridgeHandlers): () => void {
  const source = iframe.contentWindow;
  if (!source) return () => undefined;
  const post = (message: BridgeMessage): void => {
    try { source.postMessage(message, "*"); } catch { /* the frame navigated or was removed */ }
  };
  const onMessage = (event: MessageEvent): void => {
    if (event.source !== source) return;
    const message = parseBridgeMessage(event.data);
    if (!message) return;
    if (message.kind === "hello") {
      post({ bridge: RUMAHL_BRIDGE, kind: "welcome", version: RUMAHL_BRIDGE_VERSION, appId: handlers.appId, methods: BRIDGE_METHODS });
      // Push the current theme so the app does not need a round-trip.
      post({ bridge: RUMAHL_BRIDGE, kind: "event", topic: "os.theme.changed", payload: handlers.theme() });
      return;
    }
    if (message.kind !== "request") return;
    void Promise.resolve(handleBridgeRequestAsync(message, handlers)).then((response) => post(response)).catch(() => undefined);
  };
  window.addEventListener("message", onMessage);
  return () => window.removeEventListener("message", onMessage);
}

/** Pushes an OS event to an app frame (for example `os.theme.changed`). */
export function postAppBridgeEvent(iframe: HTMLIFrameElement | null, topic: string, payload?: unknown): void {
  const source = iframe?.contentWindow;
  if (!source) return;
  const event: BridgeEvent = { bridge: RUMAHL_BRIDGE, kind: "event", topic, payload };
  try { source.postMessage(event, "*"); } catch { /* ignore */ }
}
