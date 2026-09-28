import { useEffect, useState } from "react";
import { SearchIcon } from "../icons";
import { useI18n } from "../i18n";
import { useShellApps } from "../apps/useShellApps";
import { useShell } from "./ShellContext";

export function CommandPalette() {
  const { snapshot, state, dispatch, open } = useShell();
  const { t } = useI18n();
  const apps = useShellApps();
  const [query, setQuery] = useState("");
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") ||
          (event.key === "Escape" && state.commandPaletteOpen)) {
        event.preventDefault();
        dispatch({ type: "toggle-command-palette" });
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [dispatch, state.commandPaletteOpen]);
  useEffect(() => { if (state.commandPaletteOpen) setQuery(""); }, [state.commandPaletteOpen]);
  if (!state.commandPaletteOpen) return null;
  const close = () => dispatch({ type: "toggle-command-palette" });
  const normalized = query.trim().toLocaleLowerCase();
  const matches = apps.filter((app) => !normalized || `${app.title} ${app.id}`.toLocaleLowerCase().includes(normalized));
  const commands = snapshot.contributions
    .filter((item) => item.kind === "command")
    .filter((command) => !normalized || command.title.toLocaleLowerCase().includes(normalized));
  return (
          <div className="command-backdrop" role="presentation" onPointerDown={(event) => { if (event.target === event.currentTarget) close(); }}>
            <section aria-label={t("command.title")} className="command-palette" role="dialog">
              <div className="command-palette__input">
                <SearchIcon />
                <input
                  aria-label={t("command.input")}
                  autoFocus
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder={t("command.placeholder")}
                  value={query}
                />
                <kbd>Esc</kbd>
              </div>
              <div className="command-palette__results">
                {matches.length > 0 ? <>
                  <p>{t("command.apps")}</p>
                  {matches.map((app) => (
                    <button key={app.id} type="button" onClick={() => { close(); open(app.path); }}>
                      <span>{app.title}</span>
                      <small>{app.id}</small>
                    </button>
                  ))}
                </> : null}
                {commands.length > 0 ? <>
                  <p>{t("command.available")}</p>
                  {commands.map((command) => (
                    <button key={command.id} type="button">
                      <span>{command.title}</span>
                      <small>{command.capability}</small>
                    </button>
                  ))}
                </> : null}
                {matches.length === 0 && commands.length === 0 ? <p role="status">{t("launcher.noResults")}</p> : null}
              </div>
            </section>
          </div>
  );
}
