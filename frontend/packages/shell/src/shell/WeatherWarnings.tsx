import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { ShellLiveSource } from "../live-updates";
import { useI18n } from "../i18n";
import {
  fetchWeatherWarnings,
  type WeatherWarning,
  type WeatherWarningLevel,
  type WeatherWarningsState
} from "../apps/warnings-client";
import { EXTREME_WARNING_IDLE_MS, isExtremeWeatherWarning } from "../apps/weather-warning-policy";

const RANK: Record<WeatherWarningLevel, number> = {
  info: 0,
  warning: 1,
  critical: 2,
  emergency: 3
};

function levelLabel(level: WeatherWarningLevel, t: ReturnType<typeof useI18n>["t"]): string {
  return t(`warnings.level.${level}` as Parameters<typeof t>[0]);
}

function sortWarnings(warnings: readonly WeatherWarning[]): WeatherWarning[] {
  return [...warnings].sort((left, right) => RANK[right.level] - RANK[left.level] || left.expires - right.expires || left.id.localeCompare(right.id));
}

function warningKey(warning: WeatherWarning): string {
  return `${warning.source.id}:${warning.id}`;
}

function sameWarning(left: WeatherWarning, right: WeatherWarning): boolean {
  return warningKey(left) === warningKey(right);
}

function WarningGlyph({ level }: { level: WeatherWarningLevel }) {
  if (level === "info") {
    return <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 10.5V17M12 7.5h.01" />
    </svg>;
  }
  return <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <path d="M12 3.5 21 20H3Z" />
    <path d="M12 9.2v4.7M12 17.1h.01" />
  </svg>;
}

export function WeatherWarnings({ live, revision = 0 }: { live: ShellLiveSource | undefined; revision?: number }) {
  const { t, locale } = useI18n();
  const [state, setState] = useState<WeatherWarningsState | null>(null);
  const [panelOpen, setPanelOpen] = useState(false);
  const [fullscreenWarning, setFullscreenWarning] = useState<WeatherWarning | null>(null);
  const activeRefresh = useRef<AbortController | null>(null);
  const bannerRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLElement>(null);
  const lastInteraction = useRef(Date.now());
  const formatTime = useMemo(() => new Intl.DateTimeFormat(locale, {
    weekday: "short",
    hour: "2-digit",
    minute: "2-digit"
  }), [locale]);
  const formatIssued = useMemo(() => new Intl.DateTimeFormat(locale, {
    dateStyle: "short",
    timeStyle: "short"
  }), [locale]);
  const warnings = useMemo(() => state ? sortWarnings(state.warnings) : [], [state]);
  const extreme = warnings.find(isExtremeWeatherWarning);

  const refresh = useCallback(async () => {
    if (!live) return;
    activeRefresh.current?.abort();
    const controller = new AbortController();
    activeRefresh.current = controller;
    try {
      const next = await fetchWeatherWarnings(live.request, controller.signal);
      if (controller.signal.aborted) return;
      setState(next);
      if (!next.warnings.length) {
        setPanelOpen(false);
        setFullscreenWarning(null);
      }
    } catch {
      // Keep the last known warning set. The next poll or event retries.
    }
  }, [live]);

  useEffect(() => {
    if (!live) return;
    void refresh();
    const poll = () => {
      if (!document.hidden) void refresh();
    };
    const timer = window.setInterval(poll, 60_000);
    const visibility = () => {
      if (!document.hidden) void refresh();
    };
    document.addEventListener("visibilitychange", visibility);
    window.addEventListener("focus", poll);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", visibility);
      window.removeEventListener("focus", poll);
      activeRefresh.current?.abort();
    };
  }, [live, refresh, revision]);

  // Any real interaction postpones the fullscreen takeover. The topbar keeps
  // blinking in the meantime; idle devices get the full warning screen.
  useEffect(() => {
    const mark = () => { lastInteraction.current = Date.now(); };
    const events = ["pointerdown", "pointermove", "keydown", "touchstart", "wheel"] as const;
    for (const event of events) window.addEventListener(event, mark, { passive: true });
    return () => {
      for (const event of events) window.removeEventListener(event, mark);
    };
  }, []);

  useEffect(() => {
    if (!extreme) return;
    const check = () => {
      if (document.hidden || fullscreenWarning) return;
      if (Date.now() - lastInteraction.current < EXTREME_WARNING_IDLE_MS) return;
      setPanelOpen(false);
      setFullscreenWarning(extreme);
    };
    check();
    const timer = window.setInterval(check, 5_000);
    return () => window.clearInterval(timer);
  }, [extreme, fullscreenWarning]);

  // Keep the fullscreen copy current and close it when the warning disappears.
  useEffect(() => {
    if (!fullscreenWarning) return;
    const current = warnings.find((warning) => sameWarning(warning, fullscreenWarning));
    if (!current) {
      setFullscreenWarning(null);
    } else if (current.updatedAt !== fullscreenWarning.updatedAt) {
      setFullscreenWarning(current);
    }
  }, [warnings, fullscreenWarning]);

  useEffect(() => {
    if (!panelOpen) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (bannerRef.current?.contains(target) || panelRef.current?.contains(target)) return;
      setPanelOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [panelOpen]);

  useEffect(() => {
    if (!panelOpen && !fullscreenWarning) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      setPanelOpen(false);
      setFullscreenWarning(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [panelOpen, fullscreenWarning]);

  if (!warnings.length) return null;
  const highest = extreme ?? warnings[0]!;
  const label = levelLabel(highest.level, t);
  const attention = highest.level === "critical" || highest.level === "emergency";
  const announcement = attention ? "alert" : "status";
  const openFullscreen = (warning: WeatherWarning) => {
    setPanelOpen(false);
    setFullscreenWarning(warning);
  };

  return <>
    <div ref={bannerRef} className={`weather-warning-banner weather-warning-banner--${highest.level}${attention ? " weather-warning-banner--attention" : ""}`} role={announcement}
      title={[highest.region, t("warnings.validUntil", { time: formatTime.format(highest.expires) })]
        .filter(Boolean)
        .join(" · ")}>
      <span className="weather-warning-banner__icon"><WarningGlyph level={highest.level} /></span>
      <div className="weather-warning-banner__copy">
        <span className="weather-warning-banner__eyebrow">
          {label}
          {highest.test ? <span className="weather-warning__test">{t("warnings.test")}</span> : null}
          {warnings.length > 1 ? <span className="weather-warning-banner__more">{t("warnings.summary", { count: warnings.length })}</span> : null}
        </span>
        <strong className="weather-warning-banner__headline">{highest.headline}</strong>
      </div>
      <button type="button" className="weather-warning-banner__details" aria-expanded={panelOpen}
        aria-controls="weather-warning-panel" onClick={() => setPanelOpen((value) => !value)}>
        {t("warnings.details")}
      </button>
    </div>

    {panelOpen && typeof document !== "undefined" ? createPortal(
      <section ref={panelRef} id="weather-warning-panel" className={`weather-warning-panel weather-warning-panel--${highest.level}`}
        role="dialog" aria-labelledby="weather-warning-title">
        <header className="weather-warning-panel__header">
          <div className="weather-warning-panel__heading">
            <span className="weather-warning-panel__icon"><WarningGlyph level={highest.level} /></span>
            <div>
              <span className="weather-warning-panel__eyebrow">{t("warnings.panel.label")}</span>
              <h2 id="weather-warning-title">{t("warnings.panel.title")}</h2>
            </div>
            <span className="weather-warning-panel__count">{warnings.length}</span>
          </div>
          <button type="button" className="weather-warning-panel__close" aria-label={t("warnings.panel.close")}
            onClick={() => setPanelOpen(false)}>✕</button>
        </header>
        <div className="weather-warning-panel__list">
          {warnings.map((warning) => <article key={warningKey(warning)} className={`weather-warning-item weather-warning-item--${warning.level}`}>
            <header className="weather-warning-item__header">
              <span className="weather-warning-item__level">{levelLabel(warning.level, t)}</span>
              {warning.test ? <span className="weather-warning__test">{t("warnings.test")}</span> : null}
              <time className="weather-warning-item__time" dateTime={new Date(warning.expires).toISOString()}>
                {t("warnings.validUntil", { time: formatTime.format(warning.expires) })}
              </time>
            </header>
            <h3 className="weather-warning-item__headline">{warning.headline}</h3>
            <p className="weather-warning-item__source">
              {[warning.source.name, warning.region, warning.event].filter(Boolean).join(" · ")}
            </p>
            {warning.description ? <>
              <span className="weather-warning-item__label">{t("warnings.description")}</span>
              <p className="weather-warning-item__description">{warning.description}</p>
            </> : null}
            {warning.instruction ? <div className="weather-warning-item__instruction">
              <strong>{t("warnings.instruction")}</strong>
              <p>{warning.instruction}</p>
            </div> : null}
            <footer className="weather-warning-item__footer">
              <div className="weather-warning-item__meta">
                <span>{warning.sender || warning.source.name}</span>
                <span>{t("warnings.issued", { time: formatIssued.format(warning.publishedAt) })}</span>
              </div>
              <button type="button" className="weather-warning-item__fullscreen" onClick={() => openFullscreen(warning)}>
                {t("warnings.fullscreen")}
              </button>
            </footer>
          </article>)}
        </div>
      </section>, document.body) : null}

    {fullscreenWarning && typeof document !== "undefined" ? createPortal(
      <div className={`weather-warning-fullscreen weather-warning-fullscreen--${fullscreenWarning.level}${isExtremeWeatherWarning(fullscreenWarning) ? " weather-warning-fullscreen--extreme" : ""}`}
        role="alertdialog" aria-modal="true" aria-labelledby="weather-warning-fullscreen-title">
        <div className="weather-warning-fullscreen__stripe" aria-hidden="true" />
        <section className="weather-warning-fullscreen__panel">
          <span className="weather-warning-fullscreen__icon"><WarningGlyph level={fullscreenWarning.level} /></span>
          <div className="weather-warning-fullscreen__eyebrow">
            <span>{t("warnings.fullscreen.label")}</span>
            <span>{levelLabel(fullscreenWarning.level, t)}</span>
            {fullscreenWarning.test ? <span className="weather-warning__test">{t("warnings.test")}</span> : null}
          </div>
          <h2 id="weather-warning-fullscreen-title" className="weather-warning-fullscreen__headline">
            {fullscreenWarning.headline}
          </h2>
          <p className="weather-warning-fullscreen__meta">
            {[fullscreenWarning.region, fullscreenWarning.event, fullscreenWarning.sender || fullscreenWarning.source.name]
              .filter(Boolean)
              .join(" · ")}
          </p>
          {fullscreenWarning.description ? <div className="weather-warning-fullscreen__body">
            {fullscreenWarning.description}
          </div> : null}
          {fullscreenWarning.instruction ? <div className="weather-warning-fullscreen__instruction">
            <strong>{t("warnings.instruction")}</strong>
            <p>{fullscreenWarning.instruction}</p>
          </div> : null}
          {isExtremeWeatherWarning(fullscreenWarning) ? <p className="weather-warning-fullscreen__persist">
            {t("warnings.fullscreen.persist")}
          </p> : null}
          <dl className="weather-warning-fullscreen__facts">
            <div>
              <dt>{t("warnings.source")}</dt>
              <dd>{fullscreenWarning.source.name}</dd>
            </div>
            {fullscreenWarning.region ? <div>
              <dt>{t("warnings.region")}</dt>
              <dd>{fullscreenWarning.region}</dd>
            </div> : null}
            {fullscreenWarning.area ? <div>
              <dt>{t("warnings.area")}</dt>
              <dd>{fullscreenWarning.area}</dd>
            </div> : null}
            <div>
              <dt>{t("warnings.validity")}</dt>
              <dd>{formatTime.format(fullscreenWarning.expires)}</dd>
            </div>
          </dl>
          <footer className="weather-warning-fullscreen__footer">
            <span>{t("warnings.issued", { time: formatIssued.format(fullscreenWarning.publishedAt) })}</span>
            <button type="button" className="weather-warning-fullscreen__close" onClick={() => setFullscreenWarning(null)}>
              {isExtremeWeatherWarning(fullscreenWarning) ? t("warnings.fullscreen.minimize") : t("warnings.fullscreen.close")}
            </button>
          </footer>
        </section>
      </div>, document.body) : null}
  </>;
}
