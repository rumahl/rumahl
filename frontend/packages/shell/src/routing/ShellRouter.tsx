import type { PropsWithChildren } from "react";
import { BrowserRouter, MemoryRouter, StaticRouter } from "react-router";

export interface ShellRouterOptions {
  router?: "browser" | "static" | "memory";
  initialLocation?: string;
}
export function ShellRouter({ children, router = "memory", initialLocation = "/" }: PropsWithChildren<ShellRouterOptions>) {
  if (router === "browser") return <BrowserRouter>{children}</BrowserRouter>;
  if (router === "static") return <StaticRouter location={initialLocation}>{children}</StaticRouter>;
  return <MemoryRouter initialEntries={[initialLocation]}>{children}</MemoryRouter>;
}
