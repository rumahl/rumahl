import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../i18n";
import { useShell } from "./ShellContext";
import { useAppCatalog } from "../apps/AppCatalog";
import { closeKeyPalette } from "./keyboard";
import { openEmergency } from "./emergency";

interface KeyCommand { id: string; title: string; hint?: string; keywords: string; run: () => void }

/**
 * The dynamic keyboard command palette (leader `r`). It lists the commands that
 * are available right now — recovery, the in-shell center, navigation, the
 * current mode and every installed app — and filters them as you type.
 */
export function KeyPalette() {
  const { t } = useI18n();
  const { open, setMode } = useShell();
  const { apps } = useAppCatalog();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);

  const commands = useMemo<KeyCommand[]>(() => {
    const list: KeyCommand[] = [
      { id: "recovery", title: t("keycmd.recovery"), hint: "Ctrl+Alt+E", keywords: "emergency notfall recovery crash wiederherstellung", run: () => window.location.assign("/recovery") },
      { id: "center", title: t("keycmd.center"), keywords: "center maintenance wartung", run: () => { closeKeyPalette(); openEmergency(); } },
      { id: "activity", title: t("keycmd.activity"), keywords: "log audit protokoll aktivitaet", run: () => { closeKeyPalette(); open("/activity"); } },
      { id: "home", title: t("keycmd.home"), keywords: "home desktop start", run: () => { closeKeyPalette(); open("/"); } },
      { id: "apps", title: t("keycmd.apps"), keywords: "apps applications", run: () => { closeKeyPalette(); open("/apps"); } },
      { id: "settings", title: t("keycmd.settings"), keywords: "settings einstellungen", run: () => { closeKeyPalette(); open("/settings"); } },
      { id: "mode.launcher", title: t("keycmd.launcher"), keywords: "mode launcher springboard", run: () => { closeKeyPalette(); setMode("launcher"); } },
      { id: "mode.desktop", title: t("keycmd.desktop"), keywords: "mode desktop", run: () => { closeKeyPalette(); setMode("desktop"); } }
    ];
    for (const app of apps.filter((entry) => entry.launchable)) {
      list.push({ id: `app:${app.id}`, title: t("keycmd.openApp", { name: app.title }), keywords: `app open ${app.id} ${app.title}`, run: () => { closeKeyPalette(); open(`/app/${app.id}`); } });
    }
    return list;
  }, [t, apps, open, setMode]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return commands;
    return commands.filter((command) => `${command.title} ${command.keywords}`.toLowerCase().includes(needle));
  }, [commands, query]);

  useEffect(() => { setActive(0); }, [query]);
  useEffect(() => { input.current?.focus(); }, []);

  function onKeyDown(event: ReactKeyboardEvent) {
    if (event.key === "Escape") { event.preventDefault(); closeKeyPalette(); return; }
    if (event.key === "ArrowDown") { event.preventDefault(); setActive((value) => Math.min(value + 1, filtered.length - 1)); return; }
    if (event.key === "ArrowUp") { event.preventDefault(); setActive((value) => Math.max(value - 1, 0)); return; }
    if (event.key === "Enter") { event.preventDefault(); filtered[active]?.run(); }
  }

  return createPortal(
    <div className="keypalette" role="dialog" aria-modal="true" aria-label={t("keycmd.title")} onClick={(event) => { if (event.target === event.currentTarget) closeKeyPalette(); }}>
      <div className="keypalette__card" onKeyDown={onKeyDown}>
        <div className="keypalette__lead"><kbd>r</kbd></div>
        <input
          ref={input}
          className="keypalette__input"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("keycmd.placeholder")}
          aria-label={t("keycmd.title")}
          spellCheck={false}
        />
        <ul className="keypalette__list">
          {filtered.length === 0
            ? <li className="keypalette__empty">{t("keycmd.noMatch")}</li>
            : filtered.map((command, position) => <li key={command.id}>
              <button type="button" className={`keypalette__item${position === active ? " is-active" : ""}`} onMouseEnter={() => setActive(position)} onClick={() => command.run()}>
                <span>{command.title}</span>
                {command.hint ? <kbd>{command.hint}</kbd> : null}
              </button>
            </li>)}
        </ul>
      </div>
    </div>,
    document.body
  );
}
