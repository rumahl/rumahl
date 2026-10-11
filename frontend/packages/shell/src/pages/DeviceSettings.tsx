import { useEffect, useState, type ReactNode } from "react";
import { SettingsHeading } from "../components/SettingsHeading";
import { Button } from "../components/Button";
import { useI18n } from "../i18n";
import { useShell } from "../shell/ShellContext";
import { registerServiceWorker, serviceWorkerError, workerDetails } from "../service-worker";

export function DeviceSettings() {
  const { t } = useI18n();
  const { snapshot } = useShell();
  const [registration, setRegistration] = useState<ServiceWorkerRegistration>();
  const [details, setDetails] = useState<Awaited<ReturnType<typeof workerDetails>>>();
  const [error, setError] = useState(serviceWorkerError);
  const [busy, setBusy] = useState(false);
  const [revision, refresh] = useState(0);
  const [browser, setBrowser] = useState({ agent: "—", language: "—", screen: "—", mode: "browser", online: true, secure: false, supported: false });
  useEffect(() => {
    let disposed = false;
    const cleanups: (() => void)[] = [];
    const update = () => {
      setBrowser({ agent: navigator.userAgent, language: navigator.language,
        screen: `${window.innerWidth} × ${window.innerHeight} · ${window.devicePixelRatio}×`,
        mode: document.fullscreenElement || window.matchMedia?.("(display-mode: fullscreen)").matches ? "fullscreen" : window.matchMedia?.("(display-mode: standalone)").matches ? "standalone" : "browser",
        online: navigator.onLine, secure: window.isSecureContext, supported: "serviceWorker" in navigator });
    };
    update();
    const registrationChanged = () => { setError(serviceWorkerError); refresh(value => value + 1); };
    window.addEventListener("rumahl:service-worker", registrationChanged);
    cleanups.push(() => window.removeEventListener("rumahl:service-worker", registrationChanged));
    for (const name of ["online", "offline", "resize", "fullscreenchange"]) {
      window.addEventListener(name, update);
      cleanups.push(() => window.removeEventListener(name, update));
    }
    if ("serviceWorker" in navigator) {
      const changed = () => { if (!disposed) refresh(value => value + 1); };
      navigator.serviceWorker.addEventListener("controllerchange", changed);
      cleanups.push(() => navigator.serviceWorker.removeEventListener("controllerchange", changed));
      void navigator.serviceWorker.getRegistration("/").then(async found => {
        if (disposed) return;
        // Do not offer controls for another application's registration.
        const worker = found?.active ?? found?.waiting ?? found?.installing;
        if (!found || !worker || worker.scriptURL !== new URL("/service-worker.js", location.origin).href) { setRegistration(undefined); setDetails(undefined); return; }
        setRegistration(found);
        found.addEventListener("updatefound", changed);
        cleanups.push(() => found.removeEventListener("updatefound", changed));
        for (const candidate of [found.active, found.waiting, found.installing]) {
          candidate?.addEventListener("statechange", changed);
          cleanups.push(() => candidate?.removeEventListener("statechange", changed));
        }
        if (found.active) {
          try { const info = await workerDetails(found.active); if (!disposed) setDetails(info); }
          catch (failure) { if (!disposed) setError(String(failure)); }
        }
      }).catch(failure => { if (!disposed) setError(String(failure)); });
    }
    return () => { disposed = true; cleanups.forEach(cleanup => cleanup()); };
  }, [revision]);

  const check = async () => {
    setBusy(true); setError("");
    try {
      const found = await registerServiceWorker();
      setRegistration(found);
      await found.update();
    } catch (failure) { setError(failure instanceof Error ? failure.message : String(failure)); }
    finally { setBusy(false); refresh(value => value + 1); }
  };
  const row = (label: string, value: ReactNode) => <div className="setting setting--value"><div className="copy"><strong>{label}</strong></div><div className="visual"><span style={{ overflowWrap: "anywhere", whiteSpace: "normal" }}>{value}</span></div></div>;
  const supported = browser.supported;
  const secure = browser.secure;
  const controlled = supported && Boolean(registration?.active) && navigator.serviceWorker.controller?.scriptURL === registration?.active?.scriptURL;
  const status = !secure ? "insecure" : !supported ? "unsupported" : registration?.installing ? "installing" : registration?.waiting ? "waiting" : controlled ? "active" : registration?.active ? "uncontrolled" : "missing";
  return <section className="display-settings">
    <SettingsHeading title={t("device.title")} help={t("device.help")} back="/settings" />
    <h3 className="section-title">{t("device.browser")}</h3>
    <div className="settings-group">
      {row(t("settings.system.version"), snapshot.shellBuildId)}
      {row(t("device.browser"), browser.agent)}
      {row(t("device.language"), browser.language)}
      {row(t("device.screen"), browser.screen)}
      {row(t("device.mode"), t(`device.${browser.mode as "browser" | "fullscreen" | "standalone"}`))}
      {row(t("device.connection"), t(browser.online ? "device.online" : "device.offline"))}
    </div>
    <p className="settings-footnote">{t("device.kioskHelp")}</p>
    <h3 className="section-title">Service Worker</h3>
    <div className="settings-group">
      {row(t("device.status"), <span role="status">{t(`device.${status}`)}</span>)}
      {row(t("device.workerVersion"), details?.version ?? "—")}
      {row(t("device.offlineKit"), details ? `${details.cached} / ${details.total}` : "—")}
      {row(t("device.scope"), registration?.scope ?? "—")}
      <div className="setting"><div className="copy"><strong>{t("device.updates")}</strong><p>{t("device.updateHelp")}</p></div><div className="visual"><Button size="sm" disabled={busy || !secure || !supported} onClick={() => void check()}>{t(busy ? "device.checking" : registration ? "device.check" : "device.register")}</Button></div></div>
    </div>
    {error && <div role="alert"><p>{t("device.errorHelp")}</p><p style={{ overflowWrap: "anywhere" }}>{error}</p></div>}
    <p className="settings-footnote">{t("device.offlineHelp")}</p>
  </section>;
}
