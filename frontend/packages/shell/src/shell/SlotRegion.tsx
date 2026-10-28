import { windowBelongsToLink } from "./dock-model";
import type { ReactNode } from "react";
import { useTheme } from "@rumahl/ui";
import { renderSlot, type SlotBindings, type SlotComponentProps, type SlotName } from "@rumahl/ui/slots";
import { AppIcon } from "../apps/AppTile";
import { useShellApps } from "../apps/useShellApps";
import { RumahlMark } from "../components/RumahlMark";
import { HomeIcon, GridIcon, PulseIcon, SettingsIcon, SearchIcon, DesktopIcon, ShieldIcon, WindowCloseIcon, WindowMinimizeIcon, WindowMaximizeIcon, WindowRestoreIcon } from "../icons";
import { useI18n } from "../i18n";
import { describeRoute } from "../routing/routes";
import { useShell } from "./ShellContext";
import { useClock } from "./clock";
import { useLocation } from "react-router";

/**
 * Renders a theme-provided region template. Themes supply safe data markup
 * (validated in `@rumahl/ui/slots`); the shell provides the data, the
 * whitelisted components and the actions. Falls back to `null` when the theme
 * does not define the region (the shell then uses its built-in variant).
 */
export function SlotRegion({ name }: { name: SlotName }) {
  const { theme } = useTheme();
  const { t, locale } = useI18n();
  const { snapshot, state, dispatch, open, setMode, mode } = useShell();
  const apps = useShellApps();
  const location = useLocation();
  const now = useClock();
  const template = theme.slots?.[name];
  if (!template) return null;
  const active = describeRoute(location.pathname + location.search + location.hash).id;
  const bindings: SlotBindings = {
    data: {
      mode,
      user: snapshot.user.displayName,
      userInitial: snapshot.user.displayName.slice(0, 1).toUpperCase(),
      locale,
      apps: apps.map((app) => ({ id: app.id, title: app.title, path: app.path, system: app.system === true,
        running: state.windows.some(item => windowBelongsToLink(item, app.path)),
        focused: state.windows.some(item => windowBelongsToLink(item, app.path) && !item.minimized && item.id === state.focusedWindowId)
      })),
      windows: state.windows.filter(item => name !== "dock" || theme.id !== "com.rumahl.classic" || !apps.some(app => windowBelongsToLink(item, app.path))).map((item) => ({ id: item.id, title: item.title, location: item.location ?? "/", minimized: item.minimized, active: !item.minimized && active === item.id })),
      time: now ? new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(now) : ""
    },
    actions: {
      open: (arg) => {
        if (!arg) return;
        const existing = state.windows.find(item => windowBelongsToLink(item, arg));
        if (existing) dispatch({ type: "focus-window", id: existing.id });
        open(existing?.location ?? arg);
      },
      "focus-window": (arg) => { const item = state.windows.find((entry) => entry.id === arg); if (item) { dispatch({ type: "focus-window", id: item.id }); open(item.location ?? "/"); } },
      "toggle-minimize": (arg) => { if (arg) dispatch({ type: "toggle-minimize", id: arg }); },
      "minimize-all": () => { dispatch({ type: "minimize-all" }); open("/"); },
      "toggle-command": () => dispatch({ type: "toggle-command-palette" }),
      "set-mode": (arg) => { if (arg === "desktop" || arg === "launcher") setMode(arg); },
      "open-settings": () => open("/settings"),
      navigate: (arg) => { if (arg) open(arg); },
      "open-app": (arg) => { const found = apps.find((entry) => entry.id === arg); if (found) open(found.path); }
    },
    components: {
      "app-icon": ({ app, ...rest }: SlotComponentProps) => {
        const found = apps.find((entry) => entry.id === app);
        return found ? <span className={rest.className as string | undefined}><AppIcon app={found} /></span> : null;
      },
      brand: (props: SlotComponentProps) => <span className={props.className as string | undefined} aria-label="rumahl OS"><RumahlMark /></span>,
      clock: (props: SlotComponentProps) => <time className={props.className as string | undefined} dateTime={now ? now.toISOString() : undefined}>{bindings.data.time as string}</time>,
      label: (props: SlotComponentProps) => <span className={props.className as string | undefined}>{t("nav.main")}</span>,
      "command-button": (props: SlotComponentProps) => <button type="button" className={props.className as string | undefined} aria-label={t("search.system")} onClick={() => dispatch({ type: "toggle-command-palette" })}>{props.children ?? "⌘K"}</button>,
      "user-badge": (props: SlotComponentProps) => <button type="button" className={props.className as string | undefined} aria-label={t("profile.open")} onClick={() => open("/settings")}>{bindings.data.userInitial as string}</button>,
      icon: (props: SlotComponentProps) => {
        const icons: Record<string, (p: { className?: string | undefined }) => ReactNode> = {
          home: HomeIcon, grid: GridIcon, pulse: PulseIcon, settings: SettingsIcon, search: SearchIcon,
          desktop: DesktopIcon, shield: ShieldIcon, close: WindowCloseIcon, minimize: WindowMinimizeIcon,
          maximize: WindowMaximizeIcon, restore: WindowRestoreIcon
        };
        const Icon = icons[props.name as string];
        return Icon ? <Icon className={props.className as string | undefined} /> : null;
      }
    }
  };
  return <>{renderSlot(template, bindings)}</>;
}
