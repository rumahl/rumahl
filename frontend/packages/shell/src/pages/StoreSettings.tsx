import { Button } from "@rumahl/ui/components";
import { useState } from "react";
import { SettingsHeading } from "../components/SettingsHeading";
import { StoreIcon } from "../icons";
import { useI18n } from "../i18n";
import { useShell } from "../shell/ShellContext";
import { useStoreSources } from "../store/sources";

export function StoreSettings() {
  const { t } = useI18n();
  const { open } = useShell();
  const { sources, add, remove, setEnabled } = useStoreSources();
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const addSource = () => {
    setError(null);
    try {
      add(name, url);
      setName("");
      setUrl("");
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : t("storeSources.invalid"));
    }
  };
  return <section className="display-settings store-settings">
    <SettingsHeading title={t("storeSources.title")} help={t("storeSources.help")} back="/settings" />
    <div className="store-source-note"><StoreIcon aria-hidden="true" /><p>{t("storeSources.trust")}</p></div>
    <h3 className="section-title">{t("storeSources.sources")}</h3>
    <div className="settings-group">
      {sources.length === 0 ? <div className="setting"><div className="copy"><strong>{t("storeSources.empty")}</strong><p>{t("storeSources.emptyHelp")}</p></div></div> : null}
      {sources.map((source) => <div className="setting store-source-row" key={source.id}>
        <div className="copy"><strong>{source.name}</strong><p>{source.url}</p></div>
        <div className="visual store-source-actions">
          <input type="checkbox" role="switch" aria-label={t("storeSources.enabled", { name: source.name })} checked={source.enabled} onChange={(event) => setEnabled(source.id, event.target.checked)} />
          <Button onClick={() => remove(source.id)}>{t("storeSources.remove")}</Button>
        </div>
      </div>)}
    </div>
    <h3 className="section-title">{t("storeSources.addTitle")}</h3>
    <div className="settings-group">
      <div className="setting store-source-form">
        <div className="copy"><strong>{t("storeSources.name")}</strong><p>{t("storeSources.nameHelp")}</p></div>
        <div className="visual"><input type="text" value={name} onChange={(event) => setName(event.target.value)} placeholder={t("storeSources.namePlaceholder")} aria-label={t("storeSources.name")} /></div>
      </div>
      <div className="setting store-source-form">
        <div className="copy"><strong>{t("storeSources.url")}</strong><p>{t("storeSources.urlHelp")}</p></div>
        <div className="visual"><input type="url" value={url} onChange={(event) => setUrl(event.target.value)} placeholder="https://store.example/index.json" aria-label={t("storeSources.url")} /></div>
      </div>
      {error ? <p className="store-source-error" role="alert">{error}</p> : null}
      <div className="setting"><div className="copy" /><div className="visual"><Button variant="primary" onClick={addSource} disabled={!url.trim()}>{t("storeSources.add")}</Button></div></div>
    </div>
    <div className="settings-group">
      <div className="setting"><div className="copy"><strong>{t("storeSources.open")}</strong><p>{t("storeSources.openHelp")}</p></div><div className="visual"><Button onClick={() => open("/app/store")}>{t("storeSources.openAction")}</Button></div></div>
    </div>
  </section>;
}
