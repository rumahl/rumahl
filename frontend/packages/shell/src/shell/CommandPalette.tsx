import { useEffect } from "react";
import { SearchIcon } from "../icons";
import { useI18n } from "../i18n";
import { useShell } from "./ShellContext";

export function CommandPalette() {
  const { snapshot, state, dispatch } = useShell();
  const { t } = useI18n();
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
  if (!state.commandPaletteOpen) return null;
  return (
          <div className="command-backdrop" role="presentation">
            <section aria-label={t("command.title")} className="command-palette" role="dialog">
              <div className="command-palette__input">
                <SearchIcon />
                <input
                  aria-label={t("command.input")}
                  autoFocus
                  placeholder={t("command.placeholder")}
                />
                <kbd>Esc</kbd>
              </div>
              <div className="command-palette__results">
                <p>{t("command.available")}</p>
                {snapshot.contributions
                  .filter((item) => item.kind === "command")
                  .map((command) => (
                    <button key={command.id} type="button">
                      <span>{command.title}</span>
                      <small>{command.capability}</small>
                    </button>
                  ))}
              </div>
            </section>
          </div>
  );
}
