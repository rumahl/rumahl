/**
 * The app <-> OS bridge (v1).
 *
 * Installed apps run in a cross-origin, sandboxed iframe with an opaque origin
 * (`sandbox="allow-scripts"`, no `allow-same-origin`). They therefore cannot
 * read the shell's cookies or call its APIs. `window.postMessage` is the only
 * channel between the app and the shell.
 *
 * Because the app's origin is opaque, `event.origin` is `"null"` in both
 * directions and `targetOrigin` must be `"*"`. Neither side may trust
 * `event.origin`; instead the shell binds the channel to the concrete
 * `event.source === iframe.contentWindow` of the frame it created, and the app
 * binds to `event.source === window.parent`.
 *
 * Every request carries a method name. The shell authorizes each method against
 * the app's declared capabilities (a later slice) and re-checks it per call.
 */
export const RUMAHL_BRIDGE = "rumahl.bridge.v1" as const;
export const RUMAHL_BRIDGE_VERSION = 1 as const;

/** Window controls an app may request for its own window. */
export type BridgeWindowAction = "close" | "minimize" | "focus";

/**
 * Methods the bridge understands. This is the complete v1 surface; new methods
 * are additive and gated by capabilities.
 */
export type BridgeMethod =
  | "os.info"
  | "os.theme.get"
  | "os.notification"
  | "os.window.close"
  | "os.window.minimize"
  | "os.window.focus"
  | "os.capabilities.invoke";

export const BRIDGE_METHODS: readonly BridgeMethod[] = [
  "os.info",
  "os.theme.get",
  "os.notification",
  "os.window.close",
  "os.window.minimize",
  "os.window.focus",
  "os.capabilities.invoke"
];

/**
 * Permission ids an app declares in its manifest to use bridge methods. The
 * shell exposes the app's declared permissions through the catalog and refuses
 * any method whose capability the app did not declare.
 */
export const BRIDGE_CAPABILITIES = {
  info: "com.rumahl.os.info",
  theme: "com.rumahl.os.theme",
  notification: "com.rumahl.os.notification",
  window: "com.rumahl.os.window",
  capabilities: "com.rumahl.os.capabilities"
} as const;

/** The full baseline capability set (used when a catalog omits capabilities). */
export const BRIDGE_CAPABILITY_LIST: readonly string[] = Object.values(BRIDGE_CAPABILITIES);

/** Maps a bridge method to the capability it requires, or `null` when unknown. */
export function capabilityForMethod(method: string): string | null {
  if (method === "os.info") return BRIDGE_CAPABILITIES.info;
  if (method === "os.theme.get") return BRIDGE_CAPABILITIES.theme;
  if (method === "os.notification") return BRIDGE_CAPABILITIES.notification;
  if (method.startsWith("os.window.")) return BRIDGE_CAPABILITIES.window;
  if (method === "os.capabilities.invoke") return BRIDGE_CAPABILITIES.capabilities;
  return null;
}

export type BridgeScheme = "light" | "dark";
export type BridgeToastVariant = "success" | "warning" | "info" | "error";

export interface BridgeAppInfo {
  app: { id: string; title: string; version: string };
  account: { displayName: string };
  mode: "desktop" | "launcher";
}

export interface BridgeTheme {
  scheme: BridgeScheme;
  accent: string;
  reducedMotion: boolean;
}

export interface BridgeNotification {
  title?: string;
  message: string;
  variant?: BridgeToastVariant;
}

/** A resource a capability is invoked against. */
export interface BridgeCapabilityResource {
  namespace: string;
  kind: string;
  key: string;
}

/** Params for `os.capabilities.invoke`. */
export interface BridgeCapabilityInvocation {
  capability: string;
  resource?: BridgeCapabilityResource;
}

/**
 * Method the shell sends to an app that *provides* a capability. The reverse
 * of `os.capabilities.invoke`: the shell delivers the invocation to the
 * provider frame and the app answers with a `response`.
 */
export const BRIDGE_PROVIDER_METHOD = "provider.invoke" as const;

/** Params carried with a `provider.invoke` request. */
export interface BridgeProviderInvocation {
  capability: string;
  resource?: BridgeCapabilityResource;
}

/** The outcome of a capability invocation. */
export type BridgeCapabilityOutcome = "invoked" | "denied";

export interface BridgeCapabilityResult {
  capability: string;
  outcome: BridgeCapabilityOutcome;
  /** The provider's result value, when it returned one. */
  result?: unknown;
}

export interface BridgeHello {
  bridge: typeof RUMAHL_BRIDGE;
  kind: "hello";
}

export interface BridgeWelcome {
  bridge: typeof RUMAHL_BRIDGE;
  kind: "welcome";
  version: number;
  appId: string;
  methods: readonly string[];
}

export interface BridgeRequest {
  bridge: typeof RUMAHL_BRIDGE;
  kind: "request";
  id: string;
  method: string;
  params?: unknown;
}

export interface BridgeResponse {
  bridge: typeof RUMAHL_BRIDGE;
  kind: "response";
  id: string;
  ok: boolean;
  result?: unknown;
  error?: { code: string; message: string };
}

export interface BridgeEvent {
  bridge: typeof RUMAHL_BRIDGE;
  kind: "event";
  topic: string;
  payload?: unknown;
}

export type BridgeMessage = BridgeHello | BridgeWelcome | BridgeRequest | BridgeResponse | BridgeEvent;

const MAX_ID = 128;
const MAX_METHOD = 64;
const MAX_TOPIC = 64;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Validates an untrusted `message` event payload. Returns `null` when it is not a bridge message. */
export function parseBridgeMessage(value: unknown): BridgeMessage | null {
  if (!isRecord(value) || value.bridge !== RUMAHL_BRIDGE) return null;
  switch (value.kind) {
    case "hello":
      return { bridge: RUMAHL_BRIDGE, kind: "hello" };
    case "welcome":
      if (typeof value.version !== "number" || typeof value.appId !== "string" || !Array.isArray(value.methods)) return null;
      return value as unknown as BridgeWelcome;
    case "request": {
      if (typeof value.id !== "string" || value.id.length === 0 || value.id.length > MAX_ID) return null;
      if (typeof value.method !== "string" || value.method.length === 0 || value.method.length > MAX_METHOD) return null;
      const message: BridgeRequest = { bridge: RUMAHL_BRIDGE, kind: "request", id: value.id, method: value.method };
      if ("params" in value) message.params = value.params;
      return message;
    }
    case "response":
      if (typeof value.id !== "string" || typeof value.ok !== "boolean") return null;
      return value as unknown as BridgeResponse;
    case "event":
      if (typeof value.topic !== "string" || value.topic.length === 0 || value.topic.length > MAX_TOPIC) return null;
      return value as unknown as BridgeEvent;
    default:
      return null;
  }
}
