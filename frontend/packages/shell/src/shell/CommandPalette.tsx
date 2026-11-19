import { DESIGN_URI } from "../routing/internal";
import { useEffect, useId, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { SearchIcon, GridIcon, TerminalIcon, SettingsIcon, WindowCloseIcon } from "../icons";
import { useI18n } from "../i18n";
import { useShellApps } from "../apps/useShellApps";
import { useShell } from "./ShellContext";
import { useShellCommands } from "./commands";
import { useOsModePolicy } from "../preferences/OsMode";
import { useShellPreferences } from "../preferences/ShellPreferences";
import { emptySearchMemory, parseSearchMemory, rememberSearch, resolveSearchMemory, toggleSearchFavorite, type SearchMemory } from "./search-memory";

interface Placement { top: number; right: number; width: number }

export function CommandPalette() {
  const { snapshot, state, dispatch, open } = useShell();
  const { t } = useI18n();
  const apps = useShellApps();
  const commands = useShellCommands();
  const policy = useOsModePolicy();
  const { preferences } = useShellPreferences();
  const owner = preferences?.ownerId;
  const memoryKey = owner && owner !== "snapshot" ? `rumahl.search.v1:${encodeURIComponent(owner)}` : null;
  const [saved, setSaved] = useState<{ key: string | null; value: SearchMemory }>({ key: null, value: emptySearchMemory() });
  const memory = saved.key === memoryKey ? saved.value : emptySearchMemory();
  const [filter, setFilter] = useState<"all" | "favorites" | "recent">("all");
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const [placement, setPlacement] = useState<Placement | null>(null);
  const [material, setMaterial] = useState<{ mode: string; style: CSSProperties }>({ mode: "solid", style: {} });
  const listId = useId();
  const listRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const restoreFocus = useRef(true);
  const isOpen = state.commandPaletteOpen;

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

  // Anchor the panel under the top bar's search button, like the other menus.
  useEffect(() => {
    if (!isOpen) { setPlacement(null); return; }
    const place = () => {
      if (typeof window === "undefined") return;
      const width = Math.min(720, window.innerWidth - 16);
      const anchor = document.querySelector<HTMLElement>(".menubar__spotlight");
      const rect = anchor?.getBoundingClientRect();
      setPlacement({
        width,
        top: rect ? Math.round(rect.bottom + 6) : 52,
        right: rect ? Math.min(window.innerWidth - width - 8, Math.max(8, Math.round(window.innerWidth - rect.right))) : Math.max(8, Math.round((window.innerWidth - width) / 2))
      });
    };
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => { window.removeEventListener("resize", place); window.removeEventListener("scroll", place, true); };
  }, [isOpen]);
  useEffect(() => { if (isOpen) { setQuery(""); setIndex(0); setFilter("all"); } }, [isOpen]);
  useEffect(() => {
    if (!memoryKey) return;
    const load = () => {
      let value = emptySearchMemory();
      try { value = parseSearchMemory(localStorage.getItem(memoryKey)); } catch { /* Memory-only when storage is blocked. */ }
      setSaved({ key: memoryKey, value });
    };
    load();
    const sync = (event: StorageEvent) => { if (event.key === memoryKey || event.key === null) load(); };
    window.addEventListener("storage", sync);
    return () => window.removeEventListener("storage", sync);
  }, [memoryKey]);
  const saveMemory = (value: SearchMemory) => {
    if (!memoryKey) return;
    setSaved({ key: memoryKey, value });
    try { localStorage.setItem(memoryKey, JSON.stringify(value)); } catch { /* Shortcuts remain usable in memory. */ }
  };

  useEffect(() => {
    if (!isOpen) return;
    restoreFocus.current = true;
    const previous = document.activeElement;
    return () => { if (restoreFocus.current && previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, [isOpen]);

  // Dismiss when clicking outside the panel (the search button still toggles it).
  useEffect(() => {
    if (!isOpen) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Element | null;
      if (target?.closest(".command-palette") || target?.closest(".menubar__spotlight")) return;
      restoreFocus.current = false;
      dispatch({ type: "toggle-command-palette" });
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [isOpen, dispatch]);

  useEffect(() => {
    if (!isOpen) return;
    const shell = document.querySelector<HTMLElement>(".shell");
    if (!shell) return;
    const sync = () => {
      const css = getComputedStyle(shell);
      const groundRadius = shell.dataset.themeBase === "ground" ? css.getPropertyValue("--ground-radius").trim() || "0px" : "";
      setMaterial({ mode: shell.dataset.material ?? "solid", style: {
        "--search-filter": css.getPropertyValue("--surface-filter").trim() || "blur(18px)",
        "--search-base": css.getPropertyValue("--rumahl-ui-color-panel-background").trim(),
        "--search-accent": css.getPropertyValue("--rumahl-ui-color-accent-ink").trim() || css.getPropertyValue("--rumahl-ui-color-accent").trim(),
        "--search-ink": css.getPropertyValue("--rumahl-ui-color-text-primary").trim(),
        "--search-muted": css.getPropertyValue("--rumahl-ui-color-text-muted").trim(),
        ...Object.fromEntries(["radius", "item-radius", "icon-radius", "close-radius"].map(role => {
          const name = `--command-palette-${role}`;
          return [name, css.getPropertyValue(name).trim() || groundRadius];
        }).filter(([, value]) => value))
      } as CSSProperties });
    };
    sync();
    const observer = new MutationObserver(sync);
    for (const element of [shell, document.documentElement, document.body]) observer.observe(element, { attributes: true, attributeFilter: ["style", "class", "data-material", "data-scheme"] });
    return () => observer.disconnect();
  }, [isOpen]);

  const raw = query.trim();
  const term = raw.toLocaleLowerCase();
  const alias = term.replace(/^(theme|mode|shell)\s+/, "");
  const normalized = ({ dunkel: "dark", hell: "light", min: "minimize-all", minimize: "minimize-all" } as Record<string, string>)[alias] ?? alias;
  const pages = [
    { path: "/settings/display", title: t("settings.personalization") },
    { path: "/settings/workspace", title: t("workspace.title") },
    { path: "/settings/accessibility", title: t("settings.accessibility") },
    { path: "/settings/system", title: t("settings.system") }
  ];
  const entries = [
    ...commands.map(command => ({ id: command.id, kind: "command", title: command.title, subtitle: command.hint, keywords: command.keywords, run: command.run, disabled: command.disabled === true })),
    ...snapshot.contributions.flatMap(item => item.kind === "command"
      ? [{ id: item.id, kind: "command" as const, title: item.title, subtitle: item.capability, keywords: item.title, run: () => undefined, disabled: false }]
      : []),
    ...apps.filter(app => app.launchable).map(app => ({ id: `app:${app.id}`, kind: "app", title: app.title, subtitle: app.path, keywords: app.id, run: () => open(app.path), disabled: false })),
    ...pages.map(page => ({ id: page.path, kind: "page", title: page.title, subtitle: page.path, keywords: page.title, run: () => open(page.path), disabled: false }))
  ];
  // Undeclared extension capabilities are not executable shell commands.
  const matches = entries.filter(item => !term || `${item.title} ${item.subtitle} ${item.keywords}`.toLocaleLowerCase().includes(term) || item.subtitle === normalized).sort((a, b) => Number(b.subtitle.toLocaleLowerCase() === normalized || b.title.toLocaleLowerCase() === term) - Number(a.subtitle.toLocaleLowerCase() === normalized || a.title.toLocaleLowerCase() === term));
  const typed = raw.replace(/^(open|go|goto)\s+/i, "");
  const target = typed.startsWith("app:") ? `/app/${typed.slice(4)}` : typed;
  if (raw && (target.startsWith("/") && !target.startsWith("//")) && target !== "/_design" && !matches.some(item => item.subtitle === target)) {
    matches.unshift({ id: "typed", kind: "page", title: t("command.openTarget"), subtitle: target, keywords: "", run: () => open(target), disabled: false });
  }
  if (policy.browseSystemFiles && (term === "design.rl" || raw === DESIGN_URI)) matches.unshift({ id: "design.rl", kind: "command", title: t("command.design"), subtitle: "design.rl", keywords: "", run: () => { open(DESIGN_URI); }, disabled: false });
  // Keep the best matching category first, with the same visual and keyboard order.
  const visible = filter === "all" ? matches : resolveSearchMemory(memory[filter], matches);
  const kinds = term ? [...new Set(visible.map(item => item.kind))]
    : ["app", "page", "command"].filter(kind => visible.some(item => item.kind === kind));
  const items = filter === "all" ? kinds.flatMap(kind => visible.filter(item => item.kind === kind)) : visible;
  const groups = filter === "all" ? kinds.map(kind => ({ id: kind, label: t(kind === "app" ? "command.apps" : kind === "page" ? "command.pages" : "command.available"), items: items.filter(item => item.kind === kind) }))
    : [{ id: filter, label: t(filter === "favorites" ? "command.favorites" : "command.recent"), items }];
  const selected = items.length ? Math.min(index, items.length - 1) : 0;

  useEffect(() => {
    if (isOpen) listRef.current?.querySelector<HTMLElement>(".is-selected")?.scrollIntoView?.({ block: "nearest" });
  }, [isOpen, selected, filter, query]);

  if (!isOpen || !placement) return null;
  const close = () => dispatch({ type: "toggle-command-palette" });
  // Preview selection and Enter always refer to the same executable target.
  const execute = (at: number) => {
    const item = items[at];
    if (item && !item.disabled) { restoreFocus.current = false; saveMemory(rememberSearch(memory, item.id)); close(); item.run(); return; }
  };
  const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
  const preview = items[selected];
  const icon = (kind: string) => kind === "app" ? <GridIcon /> : kind === "page" ? <SettingsIcon /> : <TerminalIcon />;
  const canFavorite = preview && !preview.disabled && preview.id !== "typed" && preview.id !== "design.rl" && memoryKey && saved.key === memoryKey;
  const isFavorite = preview ? memory.favorites.includes(preview.id) : false;

  return createPortal(
    <section aria-label={t("command.title")} className="command-palette" role="dialog"
      data-material={material.mode}
      style={{ ...material.style, position: "fixed", top: placement.top, right: placement.right, width: placement.width, zIndex: 130 }}>
      <div className="command-palette__input">
        <SearchIcon />
        <input
          ref={inputRef}
          aria-label={t("command.input")}
          role="combobox" aria-autocomplete="list" aria-expanded={items.length > 0}
          aria-controls={items.length ? listId : undefined}
          aria-activedescendant={items.length ? `${listId}-${selected}` : undefined}
          autoFocus
          onChange={(event) => { setQuery(event.target.value); setIndex(0); }}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown") { event.preventDefault(); setIndex((current) => items.length ? (current + 1) % items.length : 0); }
            else if (event.key === "ArrowUp") { event.preventDefault(); setIndex((current) => items.length ? (current - 1 + items.length) % items.length : 0); }
            else if (event.key === "Enter") { event.preventDefault(); execute(selected); }
          }}
          placeholder={t("command.placeholder")}
          value={query}
        />
        <kbd className="command-palette__hint">{isMac ? "⌘K" : "Ctrl K"}</kbd>
        <button type="button" className="command-palette__close" aria-label={t("command.close")} onClick={close}><WindowCloseIcon /></button>
      </div>
      <nav className="command-palette__filters" aria-label={t("command.sources")}>
        {(["all", "favorites", "recent"] as const).map(value => <button key={value} type="button" aria-pressed={filter === value}
          onClick={() => { setFilter(value); setIndex(0); }}>
          {t(value === "all" ? "command.all" : value === "favorites" ? "command.favorites" : "command.recent")}
        </button>)}
        {filter === "recent" && memory.recent.length > 0 ? <button type="button" className="command-palette__clear" onClick={() => { saveMemory({ ...memory, recent: [] }); setIndex(0); inputRef.current?.focus(); }}>{t("command.clearRecent")}</button> : null}
      </nav>
      {filter !== "all" ? <p className="command-palette__local-note">{t("command.localOnly")}</p> : null}
      <div className="command-palette__body">
      {items.length > 0 ? <div id={listId} className="command-palette__results" ref={listRef} role="listbox" aria-label={t("command.title")}>
        {groups.map(group => <div key={group.id} role="group" aria-label={group.label}>
          <div className="command-palette__section" aria-hidden="true">{group.label}</div>
        {group.items.map(item => { const i = items.indexOf(item); return <button id={`${listId}-${i}`} tabIndex={-1} key={item.id} type="button" role="option" aria-selected={selected === i}
          className={`command-palette__item${selected === i ? " is-selected" : ""}`}
          onPointerMove={() => setIndex(i)} onPointerDown={event => event.preventDefault()} onClick={() => execute(i)} disabled={item.disabled} title={item.disabled ? t("features.demoUnavailable") : undefined}>
          <span className="command-palette__icon" aria-hidden="true">{icon(item.kind)}</span>
          <span className="command-palette__text"><strong>{item.title}</strong><small>{item.disabled ? t("features.demoUnavailable") : item.subtitle}</small></span>
        </button>; })}
        </div>)}
      </div> : <p role="status" className="command-palette__empty">{t(filter === "favorites" ? "command.emptyFavorites" : filter === "recent" ? "command.emptyRecent" : "command.noResults")}</p>}
      {preview ? <aside className="command-palette__preview" aria-label={t("command.preview")}>
        <span className="command-palette__preview-icon" aria-hidden="true">{icon(preview.kind)}</span>
        <small>{t(preview.kind === "app" ? "command.tag.app" : preview.kind === "page" ? "command.page" : "command.tag.command")}</small>
        <strong>{preview.title}</strong>
        <p>{preview.subtitle}</p>
        {canFavorite ? <button type="button" className="command-palette__favorite" aria-pressed={isFavorite}
          onClick={() => {
            saveMemory(toggleSearchFavorite(memory, preview.id));
            if (filter === "favorites") setIndex(Math.max(0, Math.min(selected, items.length - 2)));
            inputRef.current?.focus();
          }}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m12 3 2.78 5.63L21 9.54l-4.5 4.38 1.06 6.19L12 17.19l-5.56 2.92 1.06-6.19L3 9.54l6.22-.91Z" /></svg>
          {t(isFavorite ? "command.removeFavorite" : "command.addFavorite")}
        </button> : null}
        <span className="command-palette__preview-action">{t("command.execute")}</span>
      </aside> : null}
      </div>
      <footer className="command-palette__footer">
        <span><kbd>↑</kbd><kbd>↓</kbd> {t("command.navigate")}</span>
        <span><kbd>Enter</kbd> {t("command.run")}</span>
        <span><kbd>Esc</kbd> {t("command.close")}</span>
      </footer>
    </section>,
    document.body
  );
}
