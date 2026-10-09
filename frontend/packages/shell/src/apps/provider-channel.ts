import { BRIDGE_PROVIDER_METHOD, RUMAHL_BRIDGE, parseBridgeMessage, type BridgeCapabilityResource } from "@rumahl/contracts/bridge";

const DEFAULT_TIMEOUT_MS = 5000;

export interface ProviderChannel {
  invoke(capability: string, resource?: BridgeCapabilityResource): Promise<unknown>;
  dispose(): void;
}

/**
 * Delivers capability invocations to an app that *provides* them and awaits the
 * app's response. The channel is bound to the concrete `iframe.contentWindow`
 * because the sandboxed app has an opaque origin (`event.origin` is always
 * `"null"`), so neither side may trust the origin.
 */
export function attachProviderChannel(iframe: HTMLIFrameElement, timeoutMs = DEFAULT_TIMEOUT_MS): ProviderChannel {
  const source = iframe.contentWindow;
  const pending = new Map<string, { resolve: (value: unknown) => void; reject: (error: Error) => void; timer: ReturnType<typeof setTimeout> }>();
  let sequence = 0;
  const onMessage = (event: MessageEvent): void => {
    if (event.source !== source) return;
    const message = parseBridgeMessage(event.data);
    if (!message || message.kind !== "response") return;
    const entry = pending.get(message.id);
    if (!entry) return;
    pending.delete(message.id);
    clearTimeout(entry.timer);
    if (message.ok) entry.resolve(message.result);
    else entry.reject(new Error(message.error?.message ?? "the provider rejected the invocation"));
  };
  window.addEventListener("message", onMessage);
  return {
    invoke: (capability, resource) => {
      if (!source) return Promise.reject(new Error("the provider is not running"));
      const id = `p-${Date.now().toString(36)}-${(++sequence).toString(36)}`;
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => { pending.delete(id); reject(new Error("the provider did not answer")); }, timeoutMs);
        pending.set(id, { resolve, reject, timer });
        const params = resource ? { capability, resource } : { capability };
        try {
          source.postMessage({ bridge: RUMAHL_BRIDGE, kind: "request", id, method: BRIDGE_PROVIDER_METHOD, params }, "*");
        } catch {
          pending.delete(id);
          clearTimeout(timer);
          reject(new Error("the provider is not running"));
        }
      });
    },
    dispose: () => {
      window.removeEventListener("message", onMessage);
      for (const entry of pending.values()) { clearTimeout(entry.timer); entry.reject(new Error("the provider channel was disposed")); }
      pending.clear();
    }
  };
}
