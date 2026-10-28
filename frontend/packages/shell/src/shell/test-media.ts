import { useSyncExternalStore } from "react";

export interface TestMedia {
  image: string | null;
  video: string | null;
}

let state: TestMedia = { image: null, video: null };
const listeners = new Set<() => void>();
const emit = () => { for (const listener of listeners) listener(); };

/** Sets (or clears) the temporary test wallpaper media. Object URLs are revoked. */
export function setTestMedia(next: Partial<TestMedia>): void {
  if (next.image !== undefined && state.image && state.image !== next.image) URL.revokeObjectURL(state.image);
  if (next.video !== undefined && state.video && state.video !== next.video) URL.revokeObjectURL(state.video);
  state = { ...state, ...next };
  emit();
}

export function getTestMedia(): TestMedia {
  return state;
}

export function subscribeTestMedia(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

export function useTestMedia(): TestMedia {
  return useSyncExternalStore(subscribeTestMedia, getTestMedia, getTestMedia);
}
