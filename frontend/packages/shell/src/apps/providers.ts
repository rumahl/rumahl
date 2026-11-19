import type { BridgeCapabilityResource } from "@rumahl/contracts/bridge";
import type { ProviderChannel } from "./provider-channel";

const channels = new Map<string, ProviderChannel>();

/** Registers the channel for an open app that provides capabilities. */
export function registerProviderChannel(appId: string, channel: ProviderChannel): () => void {
  channels.set(appId, channel);
  return () => {
    if (channels.get(appId) === channel) channels.delete(appId);
  };
}

/** Delivers a capability invocation to an open provider app. */
export function invokeProvider(appId: string, capability: string, resource?: BridgeCapabilityResource): Promise<unknown> {
  const channel = channels.get(appId);
  if (!channel) return Promise.reject(new Error("the provider is not running"));
  return channel.invoke(capability, resource);
}
