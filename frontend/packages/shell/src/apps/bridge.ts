import {
  BRIDGE_CAPABILITIES,
  BRIDGE_METHODS,
  BRIDGE_PET_ACTIONS,
  BRIDGE_WARNING_LEVELS,
  BRIDGE_PET_ICONS,
  RUMAHL_BRIDGE,
  RUMAHL_BRIDGE_VERSION,
  capabilityForMethod,
  parseBridgeMessage,
  type BridgeCapabilityResource,
  type BridgeCapabilityResult,
  type BridgeEvent,
  type BridgeFilesAccess,
  type BridgeFilesReadResult,
  type BridgeFilesRequestResult,
  type BridgeFilesWriteResult,
  type BridgeMessage,
  type BridgeNetworkRequest,
  type BridgeNetworkResponse,
  type BridgeSettings,
  type BridgeSettingsWrite,
  type BridgeNotification,
  type BridgePetAction,
  type BridgePetActionRegistration,
  type BridgePetIcon,
  type BridgePetState,
  type BridgeRequest,
  type BridgeResponse,
  type BridgeTheme,
  type BridgeWarningClearResult,
  type BridgeWarningLevel,
  type BridgeWarningPublishResult,
  type BridgeWarningRegistration,
  type BridgeWeatherWarning,
  type BridgeWindowAction
} from "@rumahl/contracts/bridge";
import { sanitizePetActionLabel, validPetActionId } from "./pets/pet-actions";

/** Everything the bridge needs from the shell realm to serve one app window. */
export interface AppBridgeHandlers {
  appId: string;
  appTitle: string;
  version: string;
  accountName: string;
  mode: "desktop" | "launcher" | "mobile";
  /** Permission ids the app declared. A method is refused unless its capability is listed. */
  capabilities: readonly string[];
  theme(): BridgeTheme;
  notify(notification: BridgeNotification): void;
  windowControl(action: BridgeWindowAction): void;
  /** Invokes a capability provided by another installed app, on behalf of the user. */
  invokeCapability(capability: string, resource?: BridgeCapabilityResource): Promise<BridgeCapabilityResult>;
  /** Active pet state for apps with the `rumahl.pets.interact` capability. */
  petState?: () => BridgePetState | null;
  /** Triggers a built-in interaction; false when the action is unknown. */
  petInteract?: (action: BridgePetAction) => boolean;
  /** Registers an extra pet menu action for this app. */
  petRegisterAction?: (action: BridgePetActionRegistration) => (() => void) | null;
  /** Removes one previously registered pet action. */
  petUnregisterAction?: (id: string) => boolean;
  /**
   * Opens the shell's file picker so the user can grant this app access to
   * personal files. Resolves with the picked files, or `null` when the user
   * declined. The app only ever learns file ids through this consent.
   */
  filesRequest?: (access: BridgeFilesAccess) => Promise<BridgeFilesRequestResult | null>;
  /** Reads one granted personal file on the app's behalf. */
  filesRead?: (key: string) => Promise<BridgeFilesReadResult>;
  /** Writes one granted personal file on the app's behalf. */
  filesWrite?: (key: string, contentBase64: string) => Promise<BridgeFilesWriteResult>;
  /** Reads the app's own non-secret settings and configured secret keys. */
  settings?: () => Promise<BridgeSettings>;
  /** Writes the app's own settings; secret values are write-only. */
  setSettings?: (input: BridgeSettingsWrite) => Promise<BridgeSettings>;
  /** Performs a brokered HTTP(S) request to a user-approved origin. */
  netRequest?: (input: BridgeNetworkRequest) => Promise<BridgeNetworkResponse>;
  /** Registers this installed app as an OS weather-warning source. */
  warningRegister?: () => Promise<BridgeWarningRegistration>;
  /** Publishes or updates warnings; the server re-checks the app capability. */
  warningPublish?: (warnings: BridgeWeatherWarning[]) => Promise<BridgeWarningPublishResult>;
  /** Clears one warning, a set, or every warning of this app when omitted. */
  warningClear?: (ids?: string[]) => Promise<BridgeWarningClearResult>;
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
        if (!notification) return respond(false, { error: { code: "invalid-params", message: "a valid notification is required" } });
        if (notification.emergency && !handlers.capabilities.includes(BRIDGE_CAPABILITIES.emergency)) {
          return respond(false, { error: { code: "denied", message: `missing capability: ${BRIDGE_CAPABILITIES.emergency}` } });
        }
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
      case "pets.state":
        return respond(true, { result: { active: handlers.petState?.() ?? null } });
      case "pets.interact": {
        const action = normalizePetInteraction(message.params);
        if (!action) return respond(false, { error: { code: "invalid-params", message: "an unknown pet action was requested" } });
        if (!handlers.petInteract?.(action)) return respond(false, { error: { code: "denied", message: "pet interaction unavailable" } });
        return respond(true, { result: { action } });
      }
      case "pets.action.register": {
        const action = normalizePetActionRegistration(message.params);
        if (!action) return respond(false, { error: { code: "invalid-params", message: "invalid pet action registration" } });
        if (!handlers.petRegisterAction?.(action)) return respond(false, { error: { code: "denied", message: "pet action could not be registered" } });
        return respond(true, { result: { registered: true, id: action.id } });
      }
      case "pets.action.unregister": {
        const id = normalizePetActionId(message.params);
        if (!id) return respond(false, { error: { code: "invalid-params", message: "invalid pet action id" } });
        if (!handlers.petUnregisterAction?.(id)) return respond(false, { error: { code: "denied", message: "pet action not found" } });
        return respond(true, { result: { unregistered: true, id } });
      }
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
  if (record.title !== undefined) {
    if (typeof record.title !== "string" || record.title.length === 0 || record.title.length > 128) return null;
    notification.title = record.title;
  }
  if (record.description !== undefined) {
    if (typeof record.description !== "string" || record.description.length === 0 || record.description.length > 512) return null;
    notification.description = record.description;
  }
  if (record.variant !== undefined) {
    if (record.variant !== "success" && record.variant !== "warning" && record.variant !== "info" && record.variant !== "error" && record.variant !== "loading") return null;
    notification.variant = record.variant;
  }
  if (record.meta !== undefined) {
    if (typeof record.meta !== "string" || record.meta.length === 0 || record.meta.length > 64) return null;
    notification.meta = record.meta;
  }
  if (record.image !== undefined) {
    if (typeof record.image !== "string" || record.image.length > 2048 || !(record.image.startsWith("https://") || record.image.startsWith("data:image/"))) return null;
    notification.image = record.image;
  }
  if (record.duration === null) notification.duration = null;
  else if (record.duration !== undefined) {
    if (typeof record.duration !== "number" || !Number.isFinite(record.duration) || record.duration < 0 || record.duration > 3_600_000) return null;
    notification.duration = record.duration;
  }
  if (record.progress !== undefined) {
    if (typeof record.progress !== "number" || !Number.isFinite(record.progress) || record.progress < 0 || record.progress > 1) return null;
    notification.progress = record.progress;
  }
  if (record.emergency !== undefined) {
    if (typeof record.emergency !== "boolean") return null;
    if (record.emergency) notification.emergency = true;
  }
  if (Array.isArray(record.buttons)) {
    if (record.buttons.length > 2) return null;
    const buttons = [];
    const ids = new Set<string>();
    for (const value of record.buttons) {
      if (typeof value !== "object" || value === null) return null;
      const button = value as Record<string, unknown>;
      if (typeof button.id !== "string" || !/^[A-Za-z0-9._-]{1,64}$/.test(button.id) || ids.has(button.id)) return null;
      if (typeof button.label !== "string" || button.label.trim().length === 0 || button.label.length > 64) return null;
      ids.add(button.id);
      buttons.push({ id: button.id, label: button.label.trim() });
    }
    if (buttons.length) notification.buttons = buttons;
  }
  return notification;
}

function normalizePetInteraction(value: unknown): BridgePetAction | null {
  if (typeof value !== "object" || value === null) return null;
  const action = (value as Record<string, unknown>).action;
  return typeof action === "string" && (BRIDGE_PET_ACTIONS as readonly string[]).includes(action)
    ? (action as BridgePetAction)
    : null;
}

function normalizePetActionId(value: unknown): string | null {
  if (typeof value !== "object" || value === null) return null;
  const id = (value as Record<string, unknown>).id;
  return validPetActionId(id) ? id : null;
}

function normalizePetActionRegistration(value: unknown): BridgePetActionRegistration | null {
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  if (!validPetActionId(record.id)) return null;
  const label = sanitizePetActionLabel(record.label);
  if (!label) return null;
  if (record.icon !== undefined && (typeof record.icon !== "string" || !(BRIDGE_PET_ICONS as readonly string[]).includes(record.icon))) {
    return null;
  }
  return {
    id: record.id,
    label,
    ...(record.icon ? { icon: record.icon as BridgePetIcon } : {})
  };
}

function normalizeSettingsWrite(value: unknown): BridgeSettingsWrite | null {
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  if (Object.keys(record).some((key) => key !== "values")) return null;
  if (typeof record.values !== "object" || record.values === null || Array.isArray(record.values)) return null;
  const entries = Object.entries(record.values);
  if (entries.length > 64) return null;
  const values: Record<string, string | number | boolean | null> = {};
  for (const [key, entry] of entries) {
    if (!/^[a-z0-9._-]{1,64}$/.test(key)) return null;
    if (entry === null) { values[key] = null; continue; }
    if (typeof entry === "string") {
      if (entry.length > 64 * 1024) return null;
    } else if (typeof entry === "number") {
      if (!Number.isFinite(entry)) return null;
    } else if (typeof entry !== "boolean") {
      return null;
    }
    values[key] = entry;
  }
  return { values };
}

function normalizeNetworkRequest(value: unknown): BridgeNetworkRequest | null {
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  const allowed = new Set(["url", "method", "headers", "body", "secret", "maxBytes"]);
  if (Object.keys(record).some((key) => !allowed.has(key))) return null;
  if (typeof record.url !== "string" || record.url.length === 0 || record.url.length > 4096) return null;
  const request: BridgeNetworkRequest = { url: record.url };
  if (record.method !== undefined) {
    if (typeof record.method !== "string" ||
        !["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"].includes(record.method)) return null;
    request.method = record.method as NonNullable<BridgeNetworkRequest["method"]>;
  }
  if (record.headers !== undefined) {
    if (typeof record.headers !== "object" || record.headers === null || Array.isArray(record.headers)) return null;
    const entries = Object.entries(record.headers);
    if (entries.length > 16) return null;
    const headers: Record<string, string> = {};
    const allowedHeaders = new Set(["accept", "content-type", "cache-control", "if-none-match", "if-modified-since"]);
    for (const [name, entry] of entries) {
      if (!/^[A-Za-z0-9-]{1,128}$/.test(name) || !allowedHeaders.has(name.toLowerCase())) return null;
      if (typeof entry !== "string" || entry.length > 1024) return null;
      headers[name] = entry;
    }
    request.headers = headers;
  }
  if (record.body !== undefined) {
    if (typeof record.body !== "string" || record.body.length > 256 * 1024) return null;
    request.body = record.body;
  }
  if (record.secret !== undefined) {
    if (typeof record.secret !== "string" || !/^[a-z0-9._-]{1,64}$/.test(record.secret)) return null;
    request.secret = record.secret;
  }
  if (record.maxBytes !== undefined) {
    if (typeof record.maxBytes !== "number" || !Number.isInteger(record.maxBytes) ||
        record.maxBytes < 1024 || record.maxBytes > 2 * 1024 * 1024) return null;
    request.maxBytes = record.maxBytes;
  }
  return request;
}

/**
 * Handles a request, awaiting capability invocations. Synchronous methods
 * resolve immediately; `os.capabilities.invoke` calls the OS on the user's
 * behalf and may take a round-trip.
 */
const ASYNC_METHODS: ReadonlySet<string> = new Set([
  "os.capabilities.invoke",
  "os.settings.get",
  "os.settings.set",
  "os.net.request",
  "os.warnings.register",
  "os.warnings.publish",
  "os.warnings.test",
  "os.warnings.clear",
  "os.files.request",
  "os.files.read",
  "os.files.write"
]);

export async function handleBridgeRequestAsync(message: BridgeRequest, handlers: AppBridgeHandlers): Promise<BridgeResponse> {
  if (!ASYNC_METHODS.has(message.method)) return handleBridgeRequest(message, handlers);
  const respond = (ok: boolean, extra: Partial<BridgeResponse>): BridgeResponse => ({
    bridge: RUMAHL_BRIDGE,
    kind: "response",
    id: message.id,
    ok,
    ...extra
  });
  try {
    // The picker is the consent surface: it is available to any app that
    // declared a filesystem permission, and it never grants more than the
    // user picked. The generic single-capability gate cannot express
    // "either", so it runs first.
    if (message.method === "os.files.request") {
      const access = normalizeFilesAccess(message.params);
      if (!access) return respond(false, { error: { code: "invalid-params", message: "a files access of read or write is required" } });
      const declared = [BRIDGE_CAPABILITIES.filesRead, BRIDGE_CAPABILITIES.filesWrite] as const;
      if (!declared.some((permission) => handlers.capabilities.includes(permission))) {
        return respond(false, { error: { code: "denied", message: `missing capability: ${BRIDGE_CAPABILITIES.filesRead} or ${BRIDGE_CAPABILITIES.filesWrite}` } });
      }
      if (!handlers.filesRequest) return respond(false, { error: { code: "denied", message: "file access unavailable" } });
      const picked = await handlers.filesRequest(access);
      if (!picked) return respond(false, { error: { code: "denied", message: "the user declined file access" } });
      return respond(true, { result: picked });
    }
  } catch (error) {
    return respond(false, { error: { code: "failed", message: error instanceof Error ? error.message : String(error) } });
  }
  const capability = capabilityForMethod(message.method);
  if (capability === null) return respond(false, { error: { code: "unknown-method", message: `unknown method: ${message.method}` } });
  if (!handlers.capabilities.includes(capability)) return respond(false, { error: { code: "denied", message: `missing capability: ${capability}` } });
  try {
    switch (message.method) {
      case "os.files.read": {
        const key = normalizeFilesKey(message.params);
        if (!key) return respond(false, { error: { code: "invalid-params", message: "a personal file key is required" } });
        if (!handlers.filesRead) return respond(false, { error: { code: "denied", message: "file access unavailable" } });
        return respond(true, { result: await handlers.filesRead(key) });
      }
      case "os.files.write": {
        const write = normalizeFilesWrite(message.params);
        if (!write) return respond(false, { error: { code: "invalid-params", message: "a file key and base64 content are required" } });
        if (!handlers.filesWrite) return respond(false, { error: { code: "denied", message: "file access unavailable" } });
        const result = await handlers.filesWrite(write.key, write.contentBase64);
        return respond(true, { result: result });
      }
      case "os.capabilities.invoke": {
        const invocation = normalizeCapabilityInvocation(message.params);
        if (!invocation) return respond(false, { error: { code: "invalid-params", message: "a capability id is required" } });
        return respond(true, { result: await handlers.invokeCapability(invocation.capability, invocation.resource) });
      }
      case "os.settings.get": {
        if (!handlers.settings) return respond(false, { error: { code: "denied", message: "settings unavailable" } });
        return respond(true, { result: await handlers.settings() });
      }
      case "os.settings.set": {
        const write = normalizeSettingsWrite(message.params);
        if (!write) return respond(false, { error: { code: "invalid-params", message: "invalid settings write" } });
        if (!handlers.setSettings) return respond(false, { error: { code: "denied", message: "settings unavailable" } });
        return respond(true, { result: await handlers.setSettings(write) });
      }
      case "os.net.request": {
        const request = normalizeNetworkRequest(message.params);
        if (!request) return respond(false, { error: { code: "invalid-params", message: "invalid network request" } });
        if (!handlers.netRequest) return respond(false, { error: { code: "denied", message: "network unavailable" } });
        return respond(true, { result: await handlers.netRequest(request) });
      }
      case "os.warnings.register": {
        if (!handlers.warningRegister) return respond(false, { error: { code: "denied", message: "weather warnings unavailable" } });
        return respond(true, { result: await handlers.warningRegister() });
      }
      case "os.warnings.publish":
      case "os.warnings.test": {
        const warnings = normalizeWarningPublish(message.params, message.method === "os.warnings.test");
        if (!warnings) return respond(false, { error: { code: "invalid-params", message: "a valid warning batch is required" } });
        if (!handlers.warningPublish) return respond(false, { error: { code: "denied", message: "weather warnings unavailable" } });
        return respond(true, { result: await handlers.warningPublish(warnings) });
      }
      case "os.warnings.clear": {
        const clear = normalizeWarningClear(message.params);
        if (!clear) return respond(false, { error: { code: "invalid-params", message: "invalid warning ids" } });
        if (!handlers.warningClear) return respond(false, { error: { code: "denied", message: "weather warnings unavailable" } });
        return respond(true, { result: await handlers.warningClear(clear.ids) });
      }
      default:
        return respond(false, { error: { code: "unknown-method", message: `unknown method: ${message.method}` } });
    }
  } catch (error) {
    return respond(false, { error: { code: "failed", message: error instanceof Error ? error.message : String(error) } });
  }
}

const WARNING_ID = /^[A-Za-z0-9._:-]{1,64}$/;

function normalizeWarningPublish(value: unknown, forceTest: boolean): BridgeWeatherWarning[] | null {
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  const keys = Object.keys(record);
  if (keys.some((key) => key !== "warnings") || !Array.isArray(record.warnings)) return null;
  if (record.warnings.length === 0 || record.warnings.length > 8) return null;
  const ids = new Set<string>();
  const warnings: BridgeWeatherWarning[] = [];
  for (const entry of record.warnings) {
    if (typeof entry !== "object" || entry === null) return null;
    const warning = entry as Record<string, unknown>;
    const allowed = new Set(["id", "level", "headline", "description", "region", "event", "severity", "sender", "instruction", "area", "effective", "expires", "test"]);
    if (Object.keys(warning).some((key) => !allowed.has(key))) return null;
    if (typeof warning.id !== "string" || !WARNING_ID.test(warning.id) || ids.has(warning.id)) return null;
    if (typeof warning.level !== "string" || !(BRIDGE_WARNING_LEVELS as readonly string[]).includes(warning.level)) return null;
    if (typeof warning.headline !== "string" || warning.headline.trim().length === 0 || warning.headline.length > 160) return null;
    if (typeof warning.expires !== "number" || !Number.isFinite(warning.expires) || warning.expires <= 0) return null;
    for (const key of ["description", "region", "event", "severity", "sender", "instruction", "area"] as const) {
      if (warning[key] !== undefined && (typeof warning[key] !== "string" || warning[key].length > warningTextLimit(key))) return null;
    }
    if (warning.effective !== undefined && (typeof warning.effective !== "number" || !Number.isFinite(warning.effective) || warning.effective < 0)) return null;
    if (warning.test !== undefined && typeof warning.test !== "boolean") return null;
    ids.add(warning.id);
    warnings.push({
      id: warning.id,
      level: warning.level as BridgeWarningLevel,
      headline: warning.headline,
      ...(typeof warning.description === "string" ? { description: warning.description } : {}),
      ...(typeof warning.region === "string" ? { region: warning.region } : {}),
      ...(typeof warning.event === "string" ? { event: warning.event } : {}),
      ...(typeof warning.severity === "string" ? { severity: warning.severity } : {}),
      ...(typeof warning.sender === "string" ? { sender: warning.sender } : {}),
      ...(typeof warning.instruction === "string" ? { instruction: warning.instruction } : {}),
      ...(typeof warning.area === "string" ? { area: warning.area } : {}),
      ...(typeof warning.effective === "number" ? { effective: warning.effective } : {}),
      expires: warning.expires,
      ...(warning.test === true || forceTest ? { test: true } : {})
    });
  }
  return warnings;
}

function warningTextLimit(key: "description" | "region" | "event" | "severity" | "sender" | "instruction" | "area"): number {
  if (key === "description") return 2048;
  if (key === "instruction") return 1024;
  if (key === "area") return 512;
  if (key === "region" || key === "event" || key === "sender") return 120;
  return 64;
}

function normalizeWarningClear(value: unknown): { ids?: string[] } | null {
  if (value === undefined) return {};
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  if (Object.keys(record).some((key) => key !== "ids")) return null;
  if (record.ids === undefined) return {};
  if (!Array.isArray(record.ids) || record.ids.length > 32) return null;
  const ids: string[] = [];
  for (const id of record.ids) {
    if (typeof id !== "string" || !WARNING_ID.test(id)) return null;
    ids.push(id);
  }
  return { ids };
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
 * The shell's side of one app iframe channel.
 */
export interface AppBridge {
  dispose(): void;
  /** Resolves once the app completed the handshake (its first `hello`). */
  ready: Promise<void>;
}

/**
 * Binds the bridge to one app iframe. The channel is bound to the concrete
 * `iframe.contentWindow` because the sandboxed app has an opaque origin and
 * `event.origin` is always `"null"`. Returns a handle whose `ready` promise
 * resolves when the app handshakes, so capability delivery can wait for it.
 */
/** Personal-file ids: 32 lowercase hex, as minted by the files API. */
const FILES_KEY = /^[0-9a-f]{32}$/;
/** Decoded bytes an app may write through one bridge call. The platform's
 * own files API accepts 16 MiB; base64 inflates by a third, so the bridge
 * stays below that line. */
const MAX_FILES_CONTENT_BYTES = 12 * 1024 * 1024;
const BASE64 = /^[A-Za-z0-9+/]+={0,2}$/;

function normalizeFilesAccess(value: unknown): BridgeFilesAccess | null {
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  if (Object.keys(record).length !== 1 || typeof record.access !== "string") return null;
  return record.access === "read" || record.access === "write" ? record.access : null;
}

function normalizeFilesKey(value: unknown): string | null {
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  if (Object.keys(record).length !== 1 || typeof record.key !== "string") return null;
  return FILES_KEY.test(record.key) ? record.key : null;
}

function base64DecodedBytes(encoded: string): number {
  const padding = encoded.endsWith("==") ? 2 : encoded.endsWith("=") ? 1 : 0;
  return Math.floor((encoded.length * 3) / 4) - padding;
}

function normalizeFilesWrite(value: unknown): { key: string; contentBase64: string } | null {
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  if (Object.keys(record).some((key) => key !== "key" && key !== "contentBase64")) return null;
  if (typeof record.key !== "string" || !FILES_KEY.test(record.key)) return null;
  if (typeof record.contentBase64 !== "string" || !BASE64.test(record.contentBase64)) return null;
  if (base64DecodedBytes(record.contentBase64) > MAX_FILES_CONTENT_BYTES) return null;
  return { key: record.key, contentBase64: record.contentBase64 };
}

/** Decodes bridge base64 into bytes; rejects malformed or padded-junk input. */
export function decodeBridgeBase64(value: string): Uint8Array | null {
  if (typeof value !== "string" || !BASE64.test(value)) return null;
  if (base64DecodedBytes(value) > MAX_FILES_CONTENT_BYTES) return null;
  try {
    const binary = atob(value);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
    return bytes;
  } catch {
    return null;
  }
}

/** Encodes bytes for the bridge; null when the content is out of bounds. */
export function encodeBridgeBase64(bytes: Uint8Array): string | null {
  if (bytes.length > MAX_FILES_CONTENT_BYTES) return null;
  let binary = "";
  const step = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += step) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + step));
  }
  return btoa(binary);
}

export function attachAppBridge(iframe: HTMLIFrameElement, handlers: AppBridgeHandlers): AppBridge {
  const source = iframe.contentWindow;
  let resolveReady: () => void = () => undefined;
  const ready = new Promise<void>((resolve) => { resolveReady = resolve; });
  if (!source) return { dispose: () => undefined, ready };
  const post = (message: BridgeMessage): void => {
    try { source.postMessage(message, "*"); } catch { /* the frame navigated or was removed */ }
  };
  const onMessage = (event: MessageEvent): void => {
    if (event.source !== source) return;
    const message = parseBridgeMessage(event.data);
    if (!message) return;
    if (message.kind === "hello") {
      resolveReady();
      post({ bridge: RUMAHL_BRIDGE, kind: "welcome", version: RUMAHL_BRIDGE_VERSION, appId: handlers.appId, methods: BRIDGE_METHODS });
      // Push the current theme so the app does not need a round-trip.
      post({ bridge: RUMAHL_BRIDGE, kind: "event", topic: "os.theme.changed", payload: handlers.theme() });
      return;
    }
    if (message.kind !== "request") return;
    void Promise.resolve(handleBridgeRequestAsync(message, handlers)).then((response) => post(response)).catch(() => undefined);
  };
  window.addEventListener("message", onMessage);
  return { dispose: () => window.removeEventListener("message", onMessage), ready };
}

/** Pushes an OS event to an app frame (for example `os.theme.changed`). */
export function postAppBridgeEvent(iframe: HTMLIFrameElement | null, topic: string, payload?: unknown): void {
  const source = iframe?.contentWindow;
  if (!source) return;
  const event: BridgeEvent = { bridge: RUMAHL_BRIDGE, kind: "event", topic, payload };
  try { source.postMessage(event, "*"); } catch { /* ignore */ }
}
