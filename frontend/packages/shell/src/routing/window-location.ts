import { createContext, useContext } from "react";

/** The location of the window the calling component is rendered in. */
export const WindowLocationContext = createContext<string | null>(null);

export function useWindowLocation(): string | null {
  return useContext(WindowLocationContext);
}
