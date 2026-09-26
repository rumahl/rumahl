import { useEffect, useMemo, useState } from "react";
import { SearchIcon } from "../icons";
import { useI18n } from "../i18n";
import { useShellPreferences } from "../preferences/ShellPreferences";
import { useShell } from "./ShellContext";

export function Topbar() {
  const { snapshot, state, dispatch, mode, setMode } = useShell();
  const preferences = useShellPreferences();
  const { t, locale } = useI18n();
  const [timeZone, setTimeZone] = useState("UTC");
  useEffect(() => {
    document.documentElement.lang = locale;
    setTimeZone(Intl.DateTimeFormat().resolvedOptions().timeZone);
  }, [locale]);
  const formatter = useMemo(() => new Intl.DateTimeFormat(locale, {
    hour: "2-digit", minute: "2-digit", timeZone
  }), [locale, timeZone]);
  return (
        <header className="topbar">
          <button
            aria-expanded={state.commandPaletteOpen}
            className="search-trigger"
            onClick={() => dispatch({ type: "toggle-command-palette" })}
            type="button"
          >
            <SearchIcon />
            <span>{t("search.system")}</span>
            <kbd>⌘ K</kbd>
          </button>
          <div className="topbar__profile">
            <label>{t("mode.label")}
              <select disabled={!preferences.ready || preferences.saving} title={t("preferences.quickMode")} aria-label={t("mode.label")} value={mode} onChange={(event) => setMode(event.target.value === "launcher" ? "launcher" : "desktop")}>
                <option value="desktop">{t("mode.desktop")}</option>
                <option value="launcher">{t("mode.launcher")}</option>
              </select>
            </label>
            {preferences.error ? <span role="status">{t(`preferences.${preferences.error}`)}</span> : null}
            <form action="/logout" method="post"><button type="submit">{t("session.signOut")}</button></form>
            <span>{formatter.format(new Date(snapshot.systemStatus.observedAtUnixMs))}</span>
            <button aria-label={t("profile.open")} type="button">
              {snapshot.user.displayName.slice(0, 1).toUpperCase()}
            </button>
          </div>
        </header>
  );
}
