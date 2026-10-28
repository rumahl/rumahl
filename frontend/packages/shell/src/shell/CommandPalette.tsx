import { DESIGN_URI } from "../routing/internal";
import { useEffect, useId, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { SearchIcon, GridIcon, TerminalIcon } from "../icons";
import { useI18n } from "../i18n";
import { useShellApps } from "../apps/useShellApps";
import { useShell } from "./ShellContext";
import { useShellCommands } from "./commands";
import { useOsModePolicy } from "../preferences/OsMode";

interface Placement { top: number; right: number; width: number }

export function CommandPalette() {
  const { snapshot, state, dispatch, open } = useShell();
  const { t } = useI18n();
  const apps = useShellApps();
  const commands = useShellCommands();
  const policy = useOsModePolicy();
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const [placement, setPlacement] = useState<Placement | null>(null);
  const [material, setMaterial] = useState<{ mode: string; style: CSSProperties }>({ mode: "solid", style: {} });
  const listId = useId();
  const listRef = useRef<HTMLDivElement>(null);
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
      const width = Math.min(640, window.innerWidth - 16);
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
  useEffect(() => { if (isOpen) { setQuery(""); setIndex(0); } }, [isOpen]);

  // Dismiss when clicking outside the panel (the search button still toggles it).
  useEffect(() => {
    if (!isOpen) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Element | null;
      if (target?.closest(".command-palette") || target?.closest(".menubar__spotlight")) return;
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
      setMaterial({ mode: shell.dataset.material ?? "solid", style: {
        "--search-glass": css.getPropertyValue("--surface-fill").trim() || css.getPropertyValue("--menu").trim(),
        "--search-filter": css.getPropertyValue("--surface-filter").trim() || "blur(18px)",
        "--search-base": css.getPropertyValue("--rumahl-ui-color-panel-background").trim(),
        "--search-ink": css.getPropertyValue("--rumahl-ui-color-text-primary").trim(),
        "--search-muted": css.getPropertyValue("--rumahl-ui-color-text-muted").trim()
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
    ...commands.map(command => ({ id: command.id, kind: "command", title: command.title, subtitle: command.hint, keywords: command.keywords, run: command.run })),
    ...snapshot.contributions.flatMap(item => item.kind === "command"
      ? [{ id: item.id, kind: "command" as const, title: item.title, subtitle: item.capability, keywords: item.title, run: () => undefined }]
      : []),
    ...apps.filter(app => app.launchable).map(app => ({ id: `app:${app.id}`, kind: "app", title: app.title, subtitle: app.path, keywords: app.id, run: () => open(app.path) })),
    ...pages.map(page => ({ id: page.path, kind: "page", title: page.title, subtitle: page.path, keywords: page.title, run: () => open(page.path) }))
  ];
  // Undeclared extension capabilities are not executable shell commands.
  const items = entries.filter(item => !term || `${item.title} ${item.subtitle} ${item.keywords}`.toLocaleLowerCase().includes(term) || item.subtitle === normalized).sort((a, b) => Number(b.subtitle.toLocaleLowerCase() === normalized || b.title.toLocaleLowerCase() === term) - Number(a.subtitle.toLocaleLowerCase() === normalized || a.title.toLocaleLowerCase() === term));
  const typed = raw.replace(/^(open|go|goto)\s+/i, "");
  const target = typed.startsWith("app:") ? `/app/${typed.slice(4)}` : typed;
  if (raw && (target.startsWith("/") && !target.startsWith("//")) && target !== "/_design" && !items.some(item => item.subtitle === target)) {
    items.unshift({ id: "typed", kind: "page", title: t("command.openTarget"), subtitle: target, keywords: "", run: () => open(target) });
  }
  if (policy.browseSystemFiles && (term === "design.rl" || raw === DESIGN_URI)) items.unshift({ id: "design.rl", kind: "command", title: t("command.design"), subtitle: "design.rl", keywords: "", run: () => { open(DESIGN_URI); } });
  const selected = items.length ? Math.min(index, items.length - 1) : 0;

  useEffect(() => {
    if (isOpen) listRef.current?.querySelector<HTMLElement>(".is-selected")?.scrollIntoView?.({ block: "nearest" });
  }, [isOpen, selected]);

  if (!isOpen || !placement) return null;
  const close = () => dispatch({ type: "toggle-command-palette" });
  // Preview selection and Enter always refer to the same executable target.
  const execute = (at: number) => {
    const item = items[at];
    if (item) { close(); item.run(); return; }
  };
  const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
  const preview = items[selected];

  return createPortal(
    <section aria-label={t("command.title")} className="command-palette" role="dialog"
      data-material={material.mode}
      style={{ ...material.style, position: "fixed", top: placement.top, right: placement.right, width: placement.width, zIndex: 130 }}>
      <div className="command-palette__input">
        <SearchIcon />
        <input
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
      </div>
      <div className="command-palette__body">
      {items.length > 0 ? <div id={listId} className="command-palette__results" ref={listRef} role="listbox" aria-label={t("command.title")}>
        {items.map((item, i) => <button id={`${listId}-${i}`} tabIndex={-1} key={item.id} type="button" role="option" aria-selected={selected === i}
          className={`command-palette__item${selected === i ? " is-selected" : ""}`}
          onPointerMove={() => setIndex(i)} onPointerDown={event => event.preventDefault()} onClick={() => execute(i)}>
          <span className="command-palette__icon" aria-hidden="true">{item.kind === "app" ? <GridIcon /> : <TerminalIcon />}</span>
          <span className="command-palette__text"><strong>{item.title}</strong></span>
        </button>)}
      </div> : <p role="status" className="command-palette__empty">{t("command.noResults")}</p>}
      {preview ? <aside className="command-palette__preview" aria-label={t("command.preview")}>
        <span className="command-palette__preview-icon" aria-hidden="true">{preview.kind === "app" ? <GridIcon /> : <TerminalIcon />}</span>
        <small>{t(preview.kind === "app" ? "command.tag.app" : preview.kind === "page" ? "command.page" : "command.tag.command")}</small>
        <strong>{preview.title}</strong>
        <p>{preview.subtitle}</p>
        <span className="command-palette__preview-action">{t("command.execute")}</span>
      </aside> : null}
      </div>
    </section>,
    document.body
  );
}
