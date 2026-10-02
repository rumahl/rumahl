import { useI18n } from "../i18n";
import { useThemeTuning } from "../preferences/theme-tuning";
import { useShell } from "./ShellContext";

export interface ShellCommand {
  id: string;
  title: string;
  /** Short monospace hint shown on the right (like a Windows Run command). */
  hint: string;
  keywords: string;
  run: () => void;
}

interface CommandContext {
  open: (path: string) => void;
  dispatch: (action: { type: "minimize-all" }) => void;
  setShellMode: (mode: "desktop" | "launcher") => void;
  setThemeMode: (mode: "light" | "dark") => void;
  setPerformanceMode: (value: boolean) => void;
  setAnimations: (value: boolean) => void;
  reset: () => void;
}

/**
 * Runs a typed command (`Win+R`-style). These commands are intentionally not
 * shown as suggestions — they only execute when Enter is pressed.
 */
export function runShellCommand(input: string, context: CommandContext): boolean {
  const text = input.trim();
  if (!text) return false;
  const lower = text.toLocaleLowerCase();
  const [command, ...rest] = lower.split(/\s+/);
  const argument = rest.join(" ");
  switch (command) {
    case "open": case "go": case "goto":
      if (argument.startsWith("/")) { context.open(text.slice(command!.length).trim()); return true; }
      if (argument.startsWith("app:")) { context.open(`/app/${text.slice(command!.length).trim().slice(4)}`); return true; }
      return false;
    case "theme":
      if (argument === "dark" || argument === "dunkel") { context.setThemeMode("dark"); return true; }
      if (argument === "light" || argument === "hell") { context.setThemeMode("light"); return true; }
      return false;
    case "mode": case "shell":
      if (argument === "desktop") { context.setShellMode("desktop"); return true; }
      if (argument === "launcher") { context.setShellMode("launcher"); return true; }
      return false;
    case "dark": case "dunkel": context.setThemeMode("dark"); return true;
    case "light": case "hell": context.setThemeMode("light"); return true;
    case "desktop": context.setShellMode("desktop"); return true;
    case "launcher": context.setShellMode("launcher"); return true;
    case "minimize": case "min": context.dispatch({ type: "minimize-all" }); return true;
    case "performance": context.setPerformanceMode(true); return true;
    case "reset": context.reset(); return true;
  }
  if (text.startsWith("/")) { context.open(text); return true; }
  if (text.startsWith("app:")) { context.open(`/app/${text.slice(4)}`); return true; }
  return false;
}

/** Built-in runnable commands for the command palette (Ctrl/Cmd + K). */
export function useShellCommands(): ShellCommand[] {
  const { t } = useI18n();
  const { open, dispatch, setMode } = useShell();
  const tuning = useThemeTuning();
  const performance = tuning.tuning.performanceMode === true;
  const animations = tuning.tuning.animations !== false;
  return [
    { id: "open.settings", title: t("command.run.openSettings"), hint: "/settings", keywords: "settings einstellungen preferences personalization", run: () => open("/settings") },
    { id: "open.apps", title: t("command.run.openApps"), hint: "/apps", keywords: "apps applications anwendungen programs", run: () => open("/apps") },
    { id: "open.activity", title: t("command.run.openActivity"), hint: "/activity", keywords: "activity aktivitaet log", run: () => open("/activity") },
    { id: "open.home", title: t("command.run.openHome"), hint: "/", keywords: "home start startseite desktop", run: () => open("/") },
    { id: "open.appManager", title: t("command.run.openAppManager"), hint: "/app/app-manager", keywords: "app manager verwalten install", run: () => open("/app/app-manager") },
    { id: "theme.dark", title: t("command.run.darkMode"), hint: "dark", keywords: "dark dunkel night nacht", run: () => tuning.setMode("dark") },
    { id: "theme.light", title: t("command.run.lightMode"), hint: "light", keywords: "light hell day tag", run: () => tuning.setMode("light") },
    { id: "shell.desktop", title: t("command.run.desktopMode"), hint: "desktop", keywords: "desktop", run: () => setMode("desktop") },
    { id: "shell.launcher", title: t("command.run.launcherMode"), hint: "launcher", keywords: "launcher", run: () => setMode("launcher") },
    { id: "window.minimizeAll", title: t("command.run.minimizeAll"), hint: "minimize-all", keywords: "minimize minimizeall fenster minimieren", run: () => dispatch({ type: "minimize-all" }) },
    { id: "perf.toggle", title: t(performance ? "command.run.performanceOff" : "command.run.performanceOn"), hint: "performance", keywords: "performance leistung speed", run: () => tuning.setPerformanceMode(!performance) },
    { id: "motion.toggle", title: t(animations ? "command.run.animationsOff" : "command.run.animationsOn"), hint: "animations", keywords: "animations animationen motion bewegung", run: () => tuning.setAnimations(!animations) },
    { id: "appearance.reset", title: t("command.run.resetAppearance"), hint: "reset", keywords: "reset zuruecksetzen standard", run: () => tuning.reset() }
  ];
}

/** Runner for typed (non-suggested) commands. Returns true when it executed. */
export function useCommandRunner(): (input: string) => boolean {
  const { open, dispatch, setMode } = useShell();
  const tuning = useThemeTuning();
  return (input) => runShellCommand(input, {
    open,
    dispatch: (action) => dispatch(action),
    setShellMode: setMode,
    setThemeMode: tuning.setMode,
    setPerformanceMode: tuning.setPerformanceMode,
    setAnimations: tuning.setAnimations,
    reset: tuning.reset
  });
}
