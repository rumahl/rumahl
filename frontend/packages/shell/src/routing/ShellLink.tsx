import { flushSync } from "react-dom";
import { Link, useLocation, useNavigate, type LinkProps, type NavigateOptions } from "react-router";
import { localPath } from "./paths";

let activeTransition: ReturnType<typeof document.startViewTransition> | null = null;
let transitionSequence = 0;

function isSettingsPath(path: string) {
  const pathname = new URL(path, "https://shell.invalid").pathname;
  return pathname === "/settings" || pathname.startsWith("/settings/");
}

function canTransition() {
  if (typeof document === "undefined" || typeof document.startViewTransition !== "function") return false;
  if (typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches) return false;
  const shell = document.querySelector<HTMLElement>(".shell");
  return shell?.dataset.perf !== "ultra" && shell?.dataset.animations !== "off";
}

export function ShellLink({
  to, viewTransition = true, onClick, replace, state, preventScrollReset, relative, mask, reloadDocument, ...props
}: Omit<LinkProps, "to"> & { to: string }) {
  const navigate = useNavigate();
  const location = useLocation();
  const path = localPath(to);
  const settingsNavigation = isSettingsPath(location.pathname) && isSettingsPath(path);
  const navigation: NavigateOptions = {
    replace: replace ?? false,
    preventScrollReset: preventScrollReset ?? false,
    relative: relative ?? "route",
    ...(state !== undefined ? { state } : {}),
    ...(mask !== undefined ? { mask } : {})
  };
  return <Link
    {...props}
    to={path}
    replace={replace ?? false}
    state={state}
    preventScrollReset={preventScrollReset ?? false}
    relative={relative ?? "route"}
    reloadDocument={reloadDocument ?? false}
    onClick={(event) => {
      onClick?.(event);
      if (event.defaultPrevented || reloadDocument || !viewTransition || !settingsNavigation || !canTransition()) return;
      const anchor = event.currentTarget;
      if (event.button !== 0 || event.metaKey || event.altKey || event.ctrlKey || event.shiftKey) return;
      if ((anchor.target && anchor.target !== "_self") || anchor.hasAttribute("download")) return;
      event.preventDefault();
      const update = () => flushSync(() => navigate(path, navigation));
      const root = document.documentElement;
      let cleanupTimer = 0;
      const cleanup = () => {
        window.clearTimeout(cleanupTimer);
        if (transitionSequence === sequence) {
          delete root.dataset.settingsTransition;
          activeTransition = null;
        }
      };
      activeTransition?.skipTransition();
      const sequence = ++transitionSequence;
      root.dataset.settingsTransition = "true";
      try {
        const transition = document.startViewTransition(update);
        activeTransition = transition;
        cleanupTimer = window.setTimeout(cleanup, 2000);
        void transition.finished.catch(() => {}).finally(cleanup);
      } catch {
        cleanup();
        update();
      }
    }}
  />;
}
