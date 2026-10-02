import { useWorkspace } from "../preferences/Workspace";
import { useState } from "react";
import { useI18n } from "../i18n";
import { AppTile } from "./AppTile";
import { useAppCatalog } from "./AppCatalog";
import { useShellApps } from "./useShellApps";
export function AppGrid({ searchable = true, onOpen }: { searchable?: boolean; onOpen?: () => void }) {
  const { t } = useI18n();
  const { effective } = useWorkspace();
  const catalog = useAppCatalog();
  const [folder, setFolder] = useState<string | null>(null);
  const currentFolder = effective.folders.find(f => f.id === folder);
  const [query, setQuery] = useState("");
  const apps = useShellApps();
  const results = apps.filter(app => query.trim() ? true : currentFolder ? currentFolder.apps.includes(app.id) : !effective.folders.some(f => f.apps.includes(app.id))).filter((app) => `${app.title} ${app.id}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));
  return <div className="app-browser">
    {searchable ? <label className="app-search"><span>{t("launcher.search")}</span><input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("launcher.searchPlaceholder")} /></label> : null}
    {currentFolder ? <button onClick={() => setFolder(null)}>{t("workspace.back")} · {currentFolder.name}</button> : null}
    <div className="app-grid">{!query.trim() && !currentFolder ? effective.folders.map(f => <button className="app-tile" key={f.id} onClick={() => setFolder(f.id)}><span className="app-tile__icon" aria-hidden="true">▤</span><span>{f.name}</span></button>) : null}{results.map((app) => <AppTile app={app} key={app.id} onNavigate={onOpen} />)}</div>
    {results.length === 0 && (query.trim() || currentFolder || effective.folders.length === 0) ? <p role="status">{t("launcher.noResults")}</p> : null}
    {catalog.status === "loading" ? <p role="status">{t("apps.loading")}</p> : catalog.status === "unavailable" ? <p role="status">{t("apps.catalogUnavailable")}</p> : catalog.apps.length === 0 ? <p className="app-browser__note">{t("apps.empty")}</p> : null}
  </div>;
}
