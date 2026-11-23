import { useEffect, useMemo, useState } from "react";
import { Button } from "../components/Button";
import { useI18n } from "../i18n";
import { browserProfile } from "../preferences/storage";
import { useShell } from "../shell/ShellContext";
import { useAppCatalog } from "./AppCatalog";
import {
  addStoreSource,
  fetchStore,
  installStoreApp,
  removeStoreSource,
  type StoreApp,
  type StoreListing,
  type StoreSource,
  type StoreSourceKind,
} from "./store-client";

export function AppStore() {
  const { live, snapshot } = useShell();
  const { t } = useI18n();
  const catalog = useAppCatalog();
  const [listing, setListing] = useState<StoreListing>({ sources: [], apps: [] });
  const [loading, setLoading] = useState(true);
  const [unavailable, setUnavailable] = useState(false);
  const [kind, setKind] = useState<StoreSourceKind>("website");
  const [location, setLocation] = useState("");
  const [adding, setAdding] = useState(false);
  const [addError, setAddError] = useState(false);
  const [removing, setRemoving] = useState<string | null>(null);
  const [installing, setInstalling] = useState<string | null>(null);
  const [installError, setInstallError] = useState(false);
  const [installedNow, setInstalledNow] = useState<ReadonlySet<string>>(new Set());

  useEffect(() => {
    if (!live) { setLoading(false); setUnavailable(true); return; }
    const controller = new AbortController();
    setLoading(true);
    void fetchStore(live.request, controller.signal).then(
      (next) => { if (!controller.signal.aborted) { setListing(next); setUnavailable(false); } },
      () => { if (!controller.signal.aborted) setUnavailable(true); }
    ).finally(() => { if (!controller.signal.aborted) setLoading(false); });
    return () => controller.abort();
  }, [live, snapshot.revision]);

  const installed = useMemo(() => new Set([
    ...(catalog.status === "ready" ? catalog.apps : catalog.preview).map((app) => app.id),
    ...installedNow,
  ]), [catalog, installedNow]);

  async function refresh() {
    if (!live) return;
    const controller = new AbortController();
    setLoading(true);
    try {
      const next = await fetchStore(live.request, controller.signal);
      setListing(next);
      setUnavailable(false);
    } catch {
      setUnavailable(true);
    } finally {
      setLoading(false);
    }
  }

  async function add() {
    if (!live || !location.trim()) return;
    const controller = new AbortController();
    setAdding(true); setAddError(false);
    try {
      await addStoreSource(live.request, kind, location.trim(), controller.signal);
      setLocation("");
      await refresh();
    } catch {
      setAddError(true);
    } finally {
      setAdding(false);
    }
  }

  async function remove(source: StoreSource) {
    if (!live) return;
    const controller = new AbortController();
    setRemoving(source.id);
    try {
      await removeStoreSource(live.request, source.id, controller.signal);
      setListing((current) => ({
        sources: current.sources.filter((item) => item.id !== source.id),
        apps: current.apps.filter((item) => item.sourceId !== source.id),
      }));
    } catch {
      setAddError(true);
    } finally {
      setRemoving(null);
    }
  }

  async function install(app: StoreApp) {
    if (!live) return;
    const controller = new AbortController();
    setInstalling(app.id); setInstallError(false);
    try {
      await installStoreApp(live.request, app.sourceId, app.id, browserProfile().id, controller.signal);
      setInstalledNow((current) => new Set([...current, app.id]));
    } catch {
      setInstallError(true);
    } finally {
      setInstalling(null);
    }
  }

  return <div className="app-store">
    <header className="app-store__intro">
      <p className="eyebrow">{t("appStore.subtitle")}</p>
      <h1>{t("appStore.title")}</h1>
      <p>{t("appStore.help")}</p>
    </header>

    <section className="app-store__add">
      <h2>{t("appStore.addTitle")}</h2>
      <p>{t("appStore.addHelp")}</p>
      <div className="app-store__form">
        <label>
          <span>{t("appStore.kind")}</span>
          <select value={kind} onChange={(event) => setKind(event.target.value as StoreSourceKind)}>
            <option value="website">{t("appStore.website")}</option>
            <option value="github">{t("appStore.github")}</option>
          </select>
        </label>
        <label className="app-store__location">
          <span>{t("appStore.location")}</span>
          <input
            type={kind === "website" ? "url" : "text"}
            value={location}
            autoComplete="off"
            placeholder={kind === "website" ? t("appStore.websiteHint") : t("appStore.githubHint")}
            onChange={(event) => setLocation(event.target.value)}
          />
        </label>
        <Button variant="primary" disabled={adding || !location.trim() || !live} onClick={() => void add()}>
          {adding ? t("appStore.adding") : t("appStore.add")}
        </Button>
      </div>
      {addError ? <p role="alert">{t("appStore.addFailed")}</p> : null}
    </section>

    <section className="app-store__sources">
      <div className="app-store__heading">
        <h2>{t("appStore.sources")}</h2>
        <Button size="sm" disabled={loading || !live} onClick={() => void refresh()}>{t("appStore.refresh")}</Button>
      </div>
      {unavailable ? <p role="status">{t("appStore.unavailable")}</p> : loading ? <p role="status">{t("appStore.loading")}</p> : listing.sources.length === 0 ?
        <p>{t("appStore.noSources")}</p> : listing.sources.map((source) => {
          const apps = listing.apps.filter((app) => app.sourceId === source.id);
          return <article className="app-store__source" key={source.id}>
            <header>
              <div>
                <span className={`app-store__badge is-${source.kind}`}>{source.kind === "website" ? t("appStore.website") : t("appStore.github")}</span>
                <h3>{source.title}</h3>
                <small>{source.location}</small>
              </div>
              <Button size="sm" variant="danger" disabled={removing === source.id} onClick={() => void remove(source)}>
                {removing === source.id ? t("appStore.removing") : t("appStore.remove")}
              </Button>
            </header>
            {source.status === "unavailable" ? <p role="status">{source.error ?? t("appStore.sourceUnavailable")}</p> : apps.length === 0 ?
              <p>{t("appStore.noApps")}</p> : <div className="app-store__apps">{apps.map((app) => {
                const isInstalled = installed.has(app.id);
                return <article className="app-store__app" key={`${source.id}:${app.id}`}>
                  <div className="app-store__app-head">
                    <h4>{app.title}</h4>
                    <span>{app.version}</span>
                  </div>
                  {app.description ? <p>{app.description}</p> : null}
                  {app.publisher ? <small>{t("appStore.by", { publisher: app.publisher })}</small> : null}
                  <div className="app-store__app-actions">
                    <Button size="sm" variant={isInstalled ? "ghost" : "primary"} disabled={isInstalled || installing === app.id} onClick={() => void install(app)}>
                      {isInstalled ? t("appStore.installed") : installing === app.id ? t("appStore.installing") : t("appStore.install")}
                    </Button>
                  </div>
                </article>;
              })}</div>}
          </article>;
        })}
      {installError ? <p role="alert">{t("appStore.installFailed")}</p> : null}
    </section>
  </div>;
}
