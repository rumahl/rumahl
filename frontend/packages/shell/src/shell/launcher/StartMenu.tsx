import { useEffect, useMemo, useRef, useState } from "react";
import { useRecents, resolveRecentApp } from "../../apps/recents";
import { AppTile } from "../../apps/AppTile";
import { useShellApps, type ShellApp } from "../../apps/useShellApps";
import { useI18n } from "../../i18n";
import { useWorkspace } from "../../preferences/Workspace";
import { useShell } from "../ShellContext";

export function StartMenu({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const { open } = useShell();
  const apps = useShellApps();
  const { effective } = useWorkspace();
  const recents = useRecents();
  const [query, setQuery] = useState("");
  const [folder, setFolder] = useState<string | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const element = dialog.current;
    element?.showModal();
    return () => { element?.close(); previous?.focus(); };
  }, []);
  const recentApps = useMemo(() => {
    const seen = new Set<string>();
    const result: ShellApp[] = [];
    for (const path of recents) {
      const app = resolveRecentApp(path, apps);
      if (app && !seen.has(app.id)) { seen.add(app.id); result.push(app); }
    }
    return result.slice(0, 4);
  }, [recents, apps]);
  const currentFolder = effective.folders.find((item) => item.id === folder);
  const normalized = query.trim().toLocaleLowerCase();
  const visible = apps.filter((app) => {
    if (normalized) return `${app.title} ${app.id}`.toLocaleLowerCase().includes(normalized);
    if (currentFolder) return currentFolder.apps.includes(app.id);
    return true;
  });
  return <dialog ref={dialog} className="start-menu" aria-label={t("launcher.open")} onCancel={onClose}
    onKeyDown={(event) => { if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); onClose(); } }}
    onClick={(event) => { if (event.target === event.currentTarget) { const box = event.currentTarget.getBoundingClientRect(); if (event.clientX < box.left || event.clientX > box.right || event.clientY < box.top || event.clientY > box.bottom) onClose(); } }}>
    <header>
      <h2>{t("nav.apps")}</h2>
      <button type="button" onClick={onClose} aria-label={t("launcher.close")}>×</button>
    </header>
    <label className="start-menu__search">
      <span>{t("launcher.search")}</span>
      <input type="search" value={query} onChange={(event) => { setQuery(event.target.value); setFolder(null); }} placeholder={t("launcher.searchPlaceholder")} />
    </label>
    <div className="start-menu__body">
      {!normalized && !currentFolder && recentApps.length > 0 ? <section className="start-menu__section">
        <h3>{t("launcher.recent")}</h3>
        <div className="app-grid">{recentApps.map((app) => <AppTile app={app} key={app.id} onNavigate={onClose} />)}</div>
      </section> : null}
      {!normalized && currentFolder ? <button className="start-menu__back" type="button" onClick={() => setFolder(null)}>{t("workspace.back")} · {currentFolder.name}</button> : null}
      <section className="start-menu__section">
        <h3>{currentFolder ? currentFolder.name : t("launcher.allApps")}</h3>
        <div className="app-grid">
          {!normalized && !currentFolder ? effective.folders.map((item) => <button className="app-tile" key={item.id} type="button" onClick={() => setFolder(item.id)}>
            <span className="app-tile__icon" aria-hidden="true">▤</span><span>{item.name}</span>
          </button>) : null}
          {visible.map((app) => <AppTile app={app} key={app.id} onNavigate={onClose} />)}
        </div>
        {visible.length === 0 ? <p role="status">{t("launcher.noResults")}</p> : null}
      </section>
    </div>
    <footer className="start-menu__footer">
      <button type="button" onClick={() => { open("/settings"); onClose(); }}>{t("nav.settings")}</button>
    </footer>
  </dialog>;
}
