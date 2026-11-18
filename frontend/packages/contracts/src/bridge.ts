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
  | "os.capabilities.invoke"
  | "os.warnings.register"
  | "os.warnings.publish"
  | "os.warnings.test"
  | "os.warnings.clear"
  | "os.files.request"
  | "os.files.read"
  | "os.files.write"
  | "pets.state"
  | "pets.interact"
  | "pets.action.register"
  | "pets.action.unregister";

export const BRIDGE_METHODS: readonly BridgeMethod[] = [
  "os.info",
  "os.theme.get",
  "os.notification",
  "os.window.close",
  "os.window.minimize",
  "os.window.focus",
  "os.capabilities.invoke",
  "os.warnings.register",
  "os.warnings.publish",
  "os.warnings.test",
  "os.warnings.clear",
  "os.files.request",
  "os.files.read",
  "os.files.write",
  "pets.state",
  "pets.interact",
  "pets.action.register",
  "pets.action.unregister"
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
  capabilities: "com.rumahl.os.capabilities",
  /** Lets an app read the active pet, trigger interactions and add menu actions. */
  pets: "com.rumahl.pets.interact",
  /** Explicit opt-in for urgent, non-auto-dismissing emergency notifications. */
  emergency: "com.rumahl.os.emergency",
  /** Register as an OS weather-warning source and publish/clear warnings. */
  warnings: "com.rumahl.os.warnings",
  /**
   * Filesystem permissions are OS-owned manifest permissions, not free-form
   * capability ids: the picker, read and write methods map onto the catalog
   * entries the platform defines, so an app must declare exactly the one it
   * uses. `os.files.request` is satisfied by either.
   */
  filesRead: "rumahl.files.read",
  filesWrite: "rumahl.files.write"
} as const;

/**
 * Baseline capabilities for catalogs that predate explicit capability lists.
 * The pet interface is deliberately absent: it must be declared by the app and
 * is never granted by a missing/legacy capability list.
 */
export const BRIDGE_CAPABILITY_LIST: readonly string[] = [
  BRIDGE_CAPABILITIES.info,
  BRIDGE_CAPABILITIES.theme,
  BRIDGE_CAPABILITIES.notification,
  BRIDGE_CAPABILITIES.window,
  BRIDGE_CAPABILITIES.capabilities
];

/** Maps a bridge method to the capability it requires, or `null` when unknown. */
export function capabilityForMethod(method: string): string | null {
  if (method === "os.info") return BRIDGE_CAPABILITIES.info;
  if (method === "os.theme.get") return BRIDGE_CAPABILITIES.theme;
  if (method === "os.notification") return BRIDGE_CAPABILITIES.notification;
  if (method.startsWith("os.window.")) return BRIDGE_CAPABILITIES.window;
  if (method === "os.capabilities.invoke") return BRIDGE_CAPABILITIES.capabilities;
  if (method.startsWith("os.warnings.")) return BRIDGE_CAPABILITIES.warnings;
  if (method === "os.files.read") return BRIDGE_CAPABILITIES.filesRead;
  if (method === "os.files.write") return BRIDGE_CAPABILITIES.filesWrite;
  // The picker is not tied to one permission: it is available to apps that
  // declared either filesystem permission. The shell gates it itself.
  if (method === "os.files.request") return BRIDGE_CAPABILITIES.filesRead;
  if (method.startsWith("pets.")) return BRIDGE_CAPABILITIES.pets;
  return null;
}

export type BridgeScheme = "light" | "dark";
export type BridgeToastVariant = "success" | "warning" | "info" | "error" | "loading";

export interface BridgeAppInfo {
  app: { id: string; title: string; version: string };
  account: { displayName: string };
  mode: "desktop" | "launcher" | "mobile";
}

/** Semantic colours an app can adopt, mirrored from the shell palette. */
export interface BridgeThemePalette {
  background: string;
  surface: string;
  text: string;
  textMuted: string;
  border: string;
  accent: string;
  onAccent: string;
}

export interface BridgeTheme {
  scheme: BridgeScheme;
  accent: string;
  reducedMotion: boolean;
  palette: BridgeThemePalette;
}

export interface BridgeNotificationButton {
  /** Stable id sent back to the app when the user activates it. */
  id: string;
  label: string;
}

export interface BridgeNotification {
  title?: string;
  message: string;
  /** Secondary copy, rendered as the description line. */
  description?: string;
  variant?: BridgeToastVariant;
  /** HTTPS or data:image URL; rendered as the app notification image. */
  image?: string;
  meta?: string;
  /** 0..1 progress bar for uploads/background jobs. */
  progress?: number;
  /** Milliseconds; omitted uses the OS defaults. `null` keeps it persistent. */
  duration?: number | null;
  buttons?: BridgeNotificationButton[];
  /** Requires the separate `com.rumahl.os.emergency` capability. */
  emergency?: boolean;
}

/** Bridge event sent to the source app when a notification button is clicked. */
export const BRIDGE_NOTIFICATION_ACTION_TOPIC = "os.notification.action" as const;

/** Warning levels understood by the OS warning service. */
export const BRIDGE_WARNING_LEVELS = ["info", "warning", "critical", "emergency"] as const;
export type BridgeWarningLevel = (typeof BRIDGE_WARNING_LEVELS)[number];

/**
 * One warning published to the OS. The OS owns display, expiry and history;
 * the app only supplies bounded data. `expires` is an epoch millisecond value
 * in the future (at most seven days for regular warnings, one hour for tests).
 */
export interface BridgeWeatherWarning {
  /** Stable id within the source; republishing the same id updates the warning. */
  id: string;
  level: BridgeWarningLevel;
  headline: string;
  description?: string;
  region?: string;
  event?: string;
  severity?: string;
  sender?: string;
  instruction?: string;
  area?: string;
  /** Epoch milliseconds; defaults to the publish time. */
  effective?: number;
  /** Epoch milliseconds; required and always in the future. */
  expires: number;
  /** Clearly marked test warning. Forced by `os.warnings.test`. */
  test?: boolean;
}

/** Source identity returned after registration. */
export interface BridgeWarningSource {
  id: string;
  name: string;
  registeredAt: number;
}

export interface BridgeWarningRegistration {
  source: BridgeWarningSource;
}

export interface BridgeWarningPublishResult {
  accepted: number;
}

export interface BridgeWarningClearResult {
  cleared: number;
}

export interface BridgeNotificationAction {
  notificationId: string;
  actionId: string;
}
/** Public state of the active pet. Appearances remain shell-owned. */
export interface BridgePetState {
  id: string;
  name: string;
  quiet: boolean;
  render: "2d" | "3d";
  temperament: "calm" | "balanced" | "playful";
  mood: "content" | "curious" | "cozy" | "playful" | "sleepy" | "hungry" | "lonely" | "sleeping";
  vitality: number;
  hunger: number;
  bond: number;
  sleeping: boolean;
}

/** Built-in pet interactions an app may trigger through `pets.interact`. */
export const BRIDGE_PET_ACTIONS = [
  "pet",
  "play",
  "feed",
  "nap",
  "dance",
  "talk",
  "bath",
  "boop",
  "spin",
  "stretch",
  "sing",
  "peek",
  "meditate"
] as const;
export type BridgePetAction = (typeof BRIDGE_PET_ACTIONS)[number];

/** Icon ids allowed for an app-contributed pet menu action. */
export const BRIDGE_PET_ICONS = [
  "spark",
  "heart",
  "bolt",
  "star",
  "leaf",
  "music",
  "chat",
  "moon",
  "cookie",
  "wave",
  "orbit",
  "pulse"
] as const;
export type BridgePetIcon = (typeof BRIDGE_PET_ICONS)[number];

/** Metadata an app registers for an extra pet menu action. */
export interface BridgePetActionRegistration {
  id: string;
  label: string;
  icon?: BridgePetIcon;
}

/** Bridge event sent to an app when its registered pet action is clicked. */
export const BRIDGE_PET_ACTION_TOPIC = "pets.action.triggered" as const;

/** Bridge event broadcast whenever the companion reacts. */
export const BRIDGE_PET_INTERACTION_TOPIC = "pets.interaction" as const;

export interface BridgePetInteraction {
  action: string;
  petId: string;
  source?: string;
}


/** Which filesystem permission an app wants to exercise through the picker. */
export type BridgeFilesAccess = "read" | "write";

/** Params for `os.files.request`. */
export interface BridgeFilesRequestParams {
  access: BridgeFilesAccess;
}

/** One personal file the user picked for the app. */
export interface BridgeFilesFile {
  /** Personal-file id; only ever learned from the user's own picker choice. */
  id: string;
  name: string;
  size: number;
}

/** Result of `os.files.request` once the user granted access. */
export interface BridgeFilesRequestResult {
  files: BridgeFilesFile[];
}

/** Result of `os.files.read`: the file's content, base64-encoded. */
export interface BridgeFilesReadResult {
  name: string;
  size: number;
  contentBase64: string;
}

/** Params for `os.files.write`. */
export interface BridgeFilesWriteParams {
  key: string;
  contentBase64: string;
}

/** Result of `os.files.write`. */
export interface BridgeFilesWriteResult {
  name: string;
  size: number;
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
