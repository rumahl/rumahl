import { useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { AppNavLayout, Button, Chip, GroupContainer, GroupRow, IconButton, RumahlInputGroup, RumahlModal, SidenavItem, SidenavSection, type MobileIslandNavItem } from "@rumahl/ui/components";
import { ArrowIcon, ArrowLeftIcon, DesktopIcon, GridIcon, InfoIcon, RefreshIcon, SearchIcon, SettingsIcon, ShieldIcon, SlidersIcon, SparkIcon, StoreIcon, WindowStackIcon } from "../icons";
import { useAppTopbarHost } from "../components/ProtectedWindow";
import { useI18n } from "../i18n";
import { useShell } from "../shell/ShellContext";
import { appPath } from "../routing/paths";
import { useWindowLocation } from "../routing/window-location";
import { useAppCatalog, CATALOG_REFRESH_EVENT } from "./AppCatalog";
import { browserProfile } from "../preferences/storage";
import { useThemeTuning } from "../preferences/theme-tuning";
import { importPackage, type InstalledApp } from "./client";
import { useFeature } from "../features/ShellFeatures";
import { fetchStoreIndex, fetchStorePackage, formatCertificateFingerprint, storeImageUrl, trustStoreCertificate, UntrustedCertificateError, useStoreSources, type StoreAppEntry, type StoreSource } from "../store/sources";
import { showToast } from "../shell/toasts";

interface SourceState {
  status: "loading" | "ready" | "error";
  apps: readonly StoreAppEntry[];
}

type StoreView = "discover" | "all" | "library" | "categories" | "sources" | "search";

const entryKey = (entry: StoreAppEntry) => `${entry.sourceId}:${entry.id}`;

function AppArtwork({ id, title, iconUrl, large = false }: { id: string; title: string; iconUrl?: string | null | undefined; large?: boolean }) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [iconUrl]);
  const initial = title.trim().slice(0, 1).toLocaleUpperCase() || "·";
  return <span className={`store-art${large ? " store-art--large" : ""}`} data-app={id} aria-hidden="true">
    {iconUrl && !failed
      ? <img src={storeImageUrl(iconUrl)} alt="" loading="lazy" onError={() => setFailed(true)} />
      : <span className="store-art__glyph">{initial}</span>}
  </span>;
}

function StorePreview({ entry }: { entry: StoreAppEntry }) {
  const [failed, setFailed] = useState(false);
  const screenshot = entry.screenshots[0];
  useEffect(() => setFailed(false), [screenshot]);
  return screenshot && !failed
    ? <img className="store-preview" src={storeImageUrl(screenshot)} alt="" loading="lazy" onError={() => setFailed(true)} />
    : <AppArtwork id={entry.id} title={entry.title} iconUrl={entry.iconUrl} large />;
}

function ratingLabel(value: number): string {
  return `★ ${value.toFixed(1)}`;
}

function downloadsLabel(locale: string, value: number): string {
  return new Intl.NumberFormat(locale, { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

function publisherLabel(entry: StoreAppEntry): string {
  return entry.publisher.trim() || entry.sourceName;
}

function publisherKey(entry: StoreAppEntry): string {
  return entry.publisher.trim().toLocaleLowerCase();
}

function StoreAction({ entry, app, installing, busy, onInstall, onOpen, compact = false }: {
  entry: StoreAppEntry;
  app: InstalledApp | undefined;
  installing: boolean;
  busy: boolean;
  onInstall: (entry: StoreAppEntry) => void;
  onOpen: (app: InstalledApp) => void;
  compact?: boolean;
}) {
  const { t } = useI18n();
  const size = compact ? "sm" : "md";
  const variant = compact ? "secondary" : "primary";
  if (app) return <Button variant={variant} size={size} rightIcon={<ArrowIcon />} onClick={() => onOpen(app)}>{t("store.open")}</Button>;
  return <Button variant={variant} size={size} loading={installing} disabled={busy} onClick={() => onInstall(entry)}>{installing ? t("store.installing") : t("store.install")}</Button>;
}

function StoreShot({ entry, className, alt = "" }: { entry: StoreAppEntry; className: string; alt?: string }) {
  const [failed, setFailed] = useState(false);
  const screenshot = entry.screenshots[0];
  useEffect(() => setFailed(false), [screenshot]);
  return screenshot && !failed
    ? <img className={className} src={storeImageUrl(screenshot)} alt={alt} loading="lazy" onError={() => setFailed(true)} />
    : <span className={`${className} is-fallback`} aria-hidden="true"><AppArtwork id={entry.id} title={entry.title} iconUrl={entry.iconUrl} large /></span>;
}

function StoreBanner({ entries, index, onChange, app, installing, busy, onDetails, onInstall, onOpen }: {
  entries: readonly StoreAppEntry[];
  index: number;
  onChange: (index: number) => void;
  app: InstalledApp | undefined;
  installing: boolean;
  busy: boolean;
  onDetails: (entry: StoreAppEntry) => void;
  onInstall: (entry: StoreAppEntry) => void;
  onOpen: (app: InstalledApp) => void;
}) {
  const { t, locale } = useI18n();
  const entry = entries[index] ?? entries[0];
  const screenshot = entry?.screenshots[0];
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [screenshot]);
  if (!entry) return null;
  const hasArt = Boolean(screenshot) && !failed;
  const move = (next: number) => onChange((next + entries.length) % entries.length);
  return <div className="store-banner-shell">
    <section className="store-banner" aria-roledescription="carousel" aria-label={t("store.featured")}>
    {hasArt
      ? <img key={screenshot} className="store-banner__art" src={storeImageUrl(screenshot!)} alt="" onError={() => setFailed(true)} />
      : <span className="store-banner__art is-fallback" aria-hidden="true"><AppArtwork id={entry.id} title={entry.title} iconUrl={entry.iconUrl} large /></span>}
    <span className="store-banner__scrim" aria-hidden="true" />
    <div className="store-banner__content" key={entryKey(entry)} aria-live="polite">
      <div className="store-banner__eyebrow">
        <span className="store-banner__icon"><AppArtwork id={entry.id} title={entry.title} iconUrl={entry.iconUrl} /></span>
        <p className="store-kicker">{entry.featured ? t("store.featured") : t("store.spotlight")} <span>·</span> {entry.sourceName}</p>
      </div>
      <h2>{entry.title}</h2>
      <p className="store-banner__summary">{entry.summary || t("store.noDescription")}</p>
      <div className="store-banner__meta">
        {entry.categories[0] ? <span>{entry.categories[0]}</span> : null}
        {entry.rating !== null ? <span>{ratingLabel(entry.rating)}</span> : null}
        {entry.downloads !== null ? <span>{downloadsLabel(locale, entry.downloads)}</span> : null}
      </div>
      <div className="store-banner__actions">
        <StoreAction entry={entry} app={app} installing={installing} busy={busy} onInstall={onInstall} onOpen={onOpen} />
        <button type="button" className="store-banner__details" onClick={() => onDetails(entry)}>{t("store.details")}<ArrowIcon aria-hidden="true" /></button>
      </div>
    </div>
  </section>
    {entries.length > 1 ? <div className="store-banner__controls">
      <span className="store-banner__position"><b>{String(index + 1).padStart(2, "0")}</b><span>/ {String(entries.length).padStart(2, "0")}</span></span>
      <div className="store-banner__dots">
        {entries.map((item, dotIndex) => <button key={entryKey(item)} type="button" className={`store-banner__dot${dotIndex === index ? " is-active" : ""}`} aria-label={t("store.coverSlide", { index: dotIndex + 1, count: entries.length })} aria-current={dotIndex === index ? "true" : undefined} onClick={() => onChange(dotIndex)} />)}
      </div>
      <div className="store-banner__arrows">
        <button type="button" aria-label={t("store.previousFeatured")} onClick={() => move(index - 1)}><ArrowLeftIcon /></button>
        <button type="button" aria-label={t("store.nextFeatured")} onClick={() => move(index + 1)}><ArrowIcon /></button>
      </div>
    </div> : null}
  </div>;
}

function StoreSpotlight({ entry, lead = false, onDetails }: {
  entry: StoreAppEntry;
  lead?: boolean;
  onDetails: (entry: StoreAppEntry) => void;
}) {
  const { t } = useI18n();
  return <button type="button" className={`store-spotlight${lead ? " store-spotlight--lead" : ""}`} onClick={() => onDetails(entry)} aria-label={entry.title}>
    <StoreShot entry={entry} className="store-spotlight__art" />
    <span className="store-spotlight__scrim" aria-hidden="true" />
    <span className="store-spotlight__copy">
      <span className="store-spotlight__kicker">{entry.featured ? t("store.featured") : entry.sourceName}</span>
      <strong>{entry.title}</strong>
      {lead ? <span className="store-spotlight__summary">{entry.summary || t("store.noDescription")}</span> : <small>{entry.categories[0] ?? entry.sourceName}</small>}
    </span>
    <span className="store-spotlight__arrow" aria-hidden="true"><ArrowIcon /></span>
  </button>;
}

function StoreShowcase({ entries, index, onChange, spotlightEntries, installing, busy, installed, onDetails, onInstall, onOpen }: {
  entries: readonly StoreAppEntry[];
  index: number;
  onChange: (index: number) => void;
  spotlightEntries: readonly StoreAppEntry[];
  installing: string | null;
  busy: boolean;
  installed: (entry: StoreAppEntry) => InstalledApp | undefined;
  onDetails: (entry: StoreAppEntry) => void;
  onInstall: (entry: StoreAppEntry) => void;
  onOpen: (app: InstalledApp) => void;
}) {
  const entry = entries[index] ?? entries[0];
  const side = spotlightEntries.slice(0, 3);
  if (!entry) return null;
  return <section className={`store-showcase${side.length ? "" : " store-showcase--solo"}`}>
    <div className="store-showcase__main">
      <StoreBanner entries={entries} index={index} onChange={onChange} app={installed(entry)} installing={installing === entryKey(entry)} busy={busy} onDetails={onDetails} onInstall={onInstall} onOpen={onOpen} />
    </div>
    {side.length ? <div className="store-showcase__side">
      <StoreSpotlight entry={side[0]!} lead onDetails={onDetails} />
      {side.length > 1 ? <div className="store-showcase__side-row">
        {side.slice(1).map((item) => <StoreSpotlight key={entryKey(item)} entry={item} onDetails={onDetails} />)}
      </div> : null}
    </div> : null}
  </section>;
}

function StoreSectionHead({ kicker, title, action }: { kicker: string; title: string; action?: ReactNode }) {
  return <header className="store-section-head">
    <div><p className="store-kicker">{kicker}</p><h2>{title}</h2></div>
    {action}
  </header>;
}

function StoreEditCard({ entry, app, installing, busy, lead = false, onDetails, onInstall, onOpen }: {
  entry: StoreAppEntry;
  app: InstalledApp | undefined;
  installing: boolean;
  busy: boolean;
  lead?: boolean;
  onDetails: (entry: StoreAppEntry) => void;
  onInstall: (entry: StoreAppEntry) => void;
  onOpen: (app: InstalledApp) => void;
}) {
  const { t, locale } = useI18n();
  return <article className={`store-edit-card${lead ? " store-edit-card--lead" : ""}`}>
    <button type="button" className="store-edit-card__media" onClick={() => onDetails(entry)}>
      <StoreShot entry={entry} className="store-edit-card__shot" alt={entry.title} />
    </button>
    <div className="store-edit-card__body">
      <div className="store-edit-card__head">
        <AppArtwork id={entry.id} title={entry.title} iconUrl={entry.iconUrl} />
        <span className="store-edit-card__copy">
          <button type="button" className="store-edit-card__title" onClick={() => onDetails(entry)}>{entry.title}</button>
          <small>{publisherLabel(entry)}</small>
        </span>
        <StoreAction entry={entry} app={app} installing={installing} busy={busy} onInstall={onInstall} onOpen={onOpen} compact />
      </div>
      <p className="store-edit-card__summary">{entry.summary || t("store.noDescription")}</p>
      <div className="store-meta">
        {entry.categories[0] ? <span>{entry.categories[0]}</span> : null}
        {entry.rating !== null ? <span>{ratingLabel(entry.rating)}</span> : null}
        {entry.downloads !== null ? <span>{downloadsLabel(locale, entry.downloads)}</span> : null}
      </div>
    </div>
  </article>;
}

function StoreQuickAction({ entry, app, installing, busy, onInstall, onOpen }: {
  entry: StoreAppEntry;
  app: InstalledApp | undefined;
  installing: boolean;
  busy: boolean;
  onInstall: (entry: StoreAppEntry) => void;
  onOpen: (app: InstalledApp) => void;
}) {
  const { t } = useI18n();
  const label = app ? t("store.open") : installing ? t("store.installing") : t("store.get");
  return <button type="button" className="store-quick-action" disabled={!app && busy} aria-busy={installing || undefined}
    onClick={() => app ? onOpen(app) : onInstall(entry)}>{label}</button>;
}

function StoreChartRow({ entry, rank, app, installing, busy, onDetails, onInstall, onOpen }: {
  entry: StoreAppEntry;
  rank: number;
  app: InstalledApp | undefined;
  installing: boolean;
  busy: boolean;
  onDetails: (entry: StoreAppEntry) => void;
  onInstall: (entry: StoreAppEntry) => void;
  onOpen: (app: InstalledApp) => void;
}) {
  return <article className="store-chart-row">
    <span className="store-chart-row__rank">{String(rank).padStart(2, "0")}</span>
    <button type="button" className="store-chart-row__main" onClick={() => onDetails(entry)}>
      <AppArtwork id={entry.id} title={entry.title} iconUrl={entry.iconUrl} />
      <span className="store-chart-row__copy"><strong>{entry.title}</strong><small>{entry.categories[0] ?? entry.sourceName}{entry.rating !== null ? ` · ${ratingLabel(entry.rating)}` : ""}</small></span>
    </button>
    <StoreQuickAction entry={entry} app={app} installing={installing} busy={busy} onInstall={onInstall} onOpen={onOpen} />
  </article>;
}

function StoreCategoryCard({ category, apps, lead, index, onSelect }: {
  category: string;
  apps: readonly StoreAppEntry[];
  lead: StoreAppEntry | undefined;
  index: number;
  onSelect: (category: string) => void;
}) {
  const { t } = useI18n();
  return <button type="button" className={`store-category-card store-category-card--${index % 5}`} onClick={() => onSelect(category)}>
    {lead ? <StoreShot entry={lead} className="store-category-card__art" /> : <span className="store-category-card__art is-fallback" aria-hidden="true" />}
    <span className="store-category-card__scrim" aria-hidden="true" />
    <span className="store-category-card__inner">
      <span className="store-category-card__top"><span>{t("store.collection")}</span><span className="store-category-card__arrow"><ArrowIcon aria-hidden="true" /></span></span>
      <span className="store-category-card__title">{category}</span>
      <span className="store-category-card__bottom"><span className="store-category-card__icons">{apps.slice(0, 2).map((entry) => <AppArtwork key={entryKey(entry)} id={entry.id} title={entry.title} iconUrl={entry.iconUrl} />)}</span>{t("store.sourceApps", { count: apps.length })}</span>
    </span>
  </button>;
}

function StoreRailSection({ kicker, title, action, children, className = "" }: {
  kicker: string;
  title: string;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  const { t } = useI18n();
  const railRef = useRef<HTMLDivElement>(null);
  const [edges, setEdges] = useState({ previous: false, next: false });
  const measure = () => {
    const rail = railRef.current;
    if (!rail) return;
    const previous = rail.scrollLeft > 2;
    const next = rail.scrollLeft + rail.clientWidth < rail.scrollWidth - 2;
    setEdges((current) => current.previous === previous && current.next === next ? current : { previous, next });
  };
  const scroll = (direction: -1 | 1) => {
    const rail = railRef.current;
    if (!rail) return;
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches || document.querySelector('.shell')?.getAttribute("data-animations") === "off";
    rail.scrollBy({ left: direction * rail.clientWidth * 0.82, behavior: reduced ? "auto" : "smooth" });
  };
  useEffect(() => {
    const rail = railRef.current;
    if (!rail) return;
    measure();
    rail.addEventListener("scroll", measure, { passive: true });
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    observer?.observe(rail);
    window.addEventListener("resize", measure);
    return () => {
      rail.removeEventListener("scroll", measure);
      observer?.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, []);
  useEffect(measure, [children]);
  const controls = edges.previous || edges.next;
  return <section className={`store-section store-rail-section ${className}`.trim()}>
    <StoreSectionHead kicker={kicker} title={title} action={<div className="store-rail-actions">
      {action}
      {controls ? <div className="store-rail-buttons">
        <IconButton label={t("store.railPrevious")} size="xs" shape="circle" variant="ghost" disabled={!edges.previous} onClick={() => scroll(-1)}><ArrowLeftIcon /></IconButton>
        <IconButton label={t("store.railNext")} size="xs" shape="circle" variant="ghost" disabled={!edges.next} onClick={() => scroll(1)}><ArrowIcon /></IconButton>
      </div> : null}
    </div>} />
    <div className="store-rail" ref={railRef}>{children}</div>
  </section>;
}

function StoreRegistryRow({ entry, onDetails }: { entry: StoreAppEntry; onDetails: (entry: StoreAppEntry) => void }) {
  const { t } = useI18n();
  return <button type="button" className="store-registry-row" onClick={() => onDetails(entry)}>
    <AppArtwork id={entry.id} title={entry.title} iconUrl={entry.iconUrl} />
    <span className="store-registry-row__copy"><strong>{entry.title}</strong><span>{entry.summary || t("store.noDescription")}</span></span>
    <span className="store-registry-row__category">{entry.categories[0] ?? entry.sourceName}</span>
  </button>;
}

function StoreGridCard({ entry, app, installing, busy, onDetails, onInstall, onOpen }: {
  entry: StoreAppEntry;
  app: InstalledApp | undefined;
  installing: boolean;
  busy: boolean;
  onDetails: (entry: StoreAppEntry) => void;
  onInstall: (entry: StoreAppEntry) => void;
  onOpen: (app: InstalledApp) => void;
}) {
  const { t, locale } = useI18n();
  return <article className="store-grid-card">
    <button type="button" className="store-grid-card__media" onClick={() => onDetails(entry)} aria-label={entry.title}>
      <StoreShot entry={entry} className="store-grid-card__shot" alt={entry.title} />
    </button>
    <div className="store-grid-card__body">
      <div className="store-grid-card__head">
        <button type="button" className="store-grid-card__identity" onClick={() => onDetails(entry)}>
          <AppArtwork id={entry.id} title={entry.title} iconUrl={entry.iconUrl} />
          <span className="store-grid-card__title"><strong>{entry.title}</strong><small>{entry.sourceName}</small></span>
        </button>
        <StoreAction entry={entry} app={app} installing={installing} busy={busy} onInstall={onInstall} onOpen={onOpen} compact />
      </div>
      <p className="store-grid-card__summary">{entry.summary || t("store.noDescription")}</p>
      <div className="store-grid-card__meta">
        {entry.rating !== null ? <span className="store-rating">{ratingLabel(entry.rating)}</span> : null}
        {entry.downloads !== null ? <span>{downloadsLabel(locale, entry.downloads)}</span> : null}
        {entry.categories[0] ? <span className="store-tag">{entry.categories[0]}</span> : null}
        {app ? <span className="store-pill">{t("store.installedShort")}</span> : null}
      </div>
    </div>
  </article>;
}

function StoreResultRow({ entry, app, installing, busy, onDetails, onInstall, onOpen }: {
  entry: StoreAppEntry;
  app: InstalledApp | undefined;
  installing: boolean;
  busy: boolean;
  onDetails: (entry: StoreAppEntry) => void;
  onInstall: (entry: StoreAppEntry) => void;
  onOpen: (app: InstalledApp) => void;
}) {
  const { t } = useI18n();
  return <article className="store-row">
    <AppArtwork id={entry.id} title={entry.title} iconUrl={entry.iconUrl} />
    <div className="store-row__copy">
      <div className="store-row__title"><h3>{entry.title}</h3>{app ? <span className="store-pill">{t("store.installedShort")}</span> : null}{entry.categories[0] ? <span className="store-tag">{entry.categories[0]}</span> : null}</div>
      <p>{entry.summary || t("store.noDescription")}</p>
      <small>{entry.sourceName} · v{entry.version}{entry.rating !== null ? ` · ${ratingLabel(entry.rating)}` : ""}</small>
    </div>
    <div className="store-row__actions">
      <Button variant="secondary" size="sm" onClick={() => onDetails(entry)}>{t("store.details")}</Button>
      <StoreAction entry={entry} app={app} installing={installing} busy={busy} onInstall={onInstall} onOpen={onOpen} compact />
    </div>
  </article>;
}

function StoreLightbox({ title, images, index, onIndexChange, onClose, returnFocus }: {
  title: string;
  images: readonly { src: string; alt: string }[];
  index: number;
  onIndexChange: (index: number) => void;
  onClose: () => void;
  returnFocus?: HTMLElement | null;
}) {
  const { t } = useI18n();
  const titleId = useId();
  const panel = useRef<HTMLDivElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  const previousFocus = useRef<HTMLElement | null>(returnFocus ?? (typeof document !== "undefined" && document.activeElement instanceof HTMLElement ? document.activeElement : null));
  const [zoomed, setZoomed] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);
  const current = images[index];
  const storeApp = typeof document === "undefined" ? null : document.querySelector<HTMLElement>(".store-app");
  const host = storeApp?.closest<HTMLElement>(".shell-window") ?? storeApp;
  const move = (next: number) => {
    setZoomed(false);
    setFailed(null);
    onIndexChange((next + images.length) % images.length);
  };
  useEffect(() => {
    const frame = host?.querySelectorAll<HTMLElement>(".shell-window__titlebar, .shell-window__content") ?? [];
    frame.forEach((element) => element.setAttribute("inert", ""));
    closeButton.current?.focus();
    return () => {
      frame.forEach((element) => element.removeAttribute("inert"));
      if (previousFocus.current?.isConnected) previousFocus.current.focus();
    };
  }, [host]);
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        onClose();
        return;
      }
      if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
        event.preventDefault();
        move(index + (event.key === "ArrowLeft" ? -1 : 1));
        return;
      }
      if (event.key !== "Tab") return;
      const focusable = [...(panel.current?.querySelectorAll<HTMLElement>("button, [tabindex]:not([tabindex='-1'])") ?? [])];
      if (focusable.length === 0) return;
      const first = focusable[0]!;
      const last = focusable[focusable.length - 1]!;
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [index, images.length, onClose]);
  if (!current || !host) return null;
  return createPortal(<div className="store-lightbox" role="dialog" aria-modal="true" aria-labelledby={titleId} onClick={(event) => { if (event.target === event.currentTarget) onClose(); }}>
    <div className="store-lightbox__surface" ref={panel}>
      <header className="store-lightbox__head">
        <div className="store-lightbox__identity">
          <span>{t("store.screenshots")}</span>
          <h2 id={titleId}>{title}</h2>
        </div>
        <div className="store-lightbox__tools">
          <span className="store-lightbox__count" aria-live="polite">{index + 1} / {images.length}</span>
          <button type="button" className="store-lightbox__tool" aria-pressed={zoomed} onClick={() => setZoomed(!zoomed)}>{zoomed ? t("store.zoomOut") : t("store.zoomIn")}</button>
          <button type="button" className="store-lightbox__close" ref={closeButton} aria-label={t("store.close")} onClick={onClose}>×</button>
        </div>
      </header>
      <div className="store-lightbox__stage" data-zoomed={zoomed || undefined} onDoubleClick={() => setZoomed(!zoomed)}>
        {failed === current.src
          ? <p role="status">{t("store.imageUnavailable")}</p>
          : <img key={current.src} src={current.src} alt={current.alt} onError={() => setFailed(current.src)} />}
        {images.length > 1 ? <>
          <button type="button" className="store-lightbox__nav store-lightbox__nav--prev" aria-label={t("store.previousImage")} onClick={() => move(index - 1)}><ArrowLeftIcon /></button>
          <button type="button" className="store-lightbox__nav store-lightbox__nav--next" aria-label={t("store.nextImage")} onClick={() => move(index + 1)}><ArrowIcon /></button>
        </> : null}
      </div>
      <footer className="store-lightbox__footer">
        <p className="store-lightbox__caption">{current.alt}</p>
        {images.length > 1 ? <div className="store-lightbox__thumbnails">{images.map((image, thumbIndex) => <button type="button" key={`${image.src}:${thumbIndex}`} aria-label={image.alt} aria-pressed={thumbIndex === index} onClick={() => move(thumbIndex)}><img src={image.src} alt="" loading="lazy" /></button>)}</div> : null}
      </footer>
    </div>
  </div>, host);
}

function StoreDeveloper({ developer, entries, backLabel, installing, busy, installed, onBack, onDetails, onInstall, onOpen }: {
  developer: string;
  entries: readonly StoreAppEntry[];
  backLabel: string;
  installing: string | null;
  busy: boolean;
  installed: (entry: StoreAppEntry) => InstalledApp | undefined;
  onBack: () => void;
  onDetails: (entry: StoreAppEntry) => void;
  onInstall: (entry: StoreAppEntry) => void;
  onOpen: (app: InstalledApp) => void;
}) {
  const { t } = useI18n();
  const lead = entries.find((entry) => entry.screenshots.length > 0) ?? entries[0];
  const sourceCount = new Set(entries.map((entry) => entry.sourceName)).size;
  const initial = developer.trim().slice(0, 1).toLocaleUpperCase() || "·";
  return <div className="store-developer-page">
    <div className="store-detail-bar">
      <Button variant="ghost" size="sm" leftIcon={<ArrowLeftIcon />} onClick={onBack}>{t("store.backTo", { place: backLabel })}</Button>
      <span>{t("store.developer")}</span>
    </div>

    <header className="store-developer-hero">
      {lead ? <div className="store-developer-hero__backdrop" aria-hidden="true"><StoreShot entry={lead} className="store-developer-hero__bg" /></div> : null}
      <span className="store-developer-hero__scrim" aria-hidden="true" />
      <div className="store-developer-hero__identity">
        <span className="store-developer-hero__mark" aria-hidden="true">{initial}</span>
        <div>
          <p className="store-kicker">{t("store.developer")}</p>
          <h1>{developer}</h1>
          <p>{t("store.appCount", { count: entries.length })} <span>·</span> {t("store.sourceCount", { count: sourceCount })}</p>
        </div>
      </div>
    </header>

    <section className="store-developer-apps">
      <div className="store-section-head">
        <div><p className="store-kicker">{t("store.catalog")}</p><h2>{t("store.appsByDeveloper", { developer })}</h2></div>
        <span>{t("store.appCount", { count: entries.length })}</span>
      </div>
      <p className="store-developer-help">{t("store.developerHelp")}</p>
      <div className="store-grid store-developer-grid">
        {entries.map((entry) => <StoreGridCard key={entryKey(entry)} entry={entry} app={installed(entry)} installing={installing === entryKey(entry)} busy={busy} onDetails={onDetails} onInstall={onInstall} onOpen={onOpen} />)}
      </div>
    </section>
  </div>;
}

function StoreDetail({ entry, app, source, more, installing, busy, backLabel, onSource, onBack, onDetails, onDeveloper, onInstall, onOpen }: {
  entry: StoreAppEntry;
  app: InstalledApp | undefined;
  source: StoreSource | undefined;
  more: readonly StoreAppEntry[];
  installing: boolean;
  busy: boolean;
  backLabel: string;
  onSource: () => void;
  onBack: () => void;
  onDetails: (entry: StoreAppEntry) => void;
  onDeveloper: (entry: StoreAppEntry) => void;
  onInstall: (entry: StoreAppEntry) => void;
  onOpen: (app: InstalledApp) => void;
}) {
  const { t, locale } = useI18n();
  const sectionPrefix = useId();
  const sectionId = (name: string) => `${sectionPrefix}-${name}`;
  const jump = (name: string) => document.getElementById(sectionId(name))?.scrollIntoView({ block: "start" });
  const [shotIndex, setShotIndex] = useState<number | null>(null);
  const shotOpener = useRef<HTMLButtonElement | null>(null);
  useEffect(() => setShotIndex(null), [entry]);
  const closeShot = () => {
    setShotIndex(null);
    requestAnimationFrame(() => shotOpener.current?.focus());
  };
  const sourceHost = source ? new URL(source.url).hostname : null;
  const developerApps = entry.publisher.trim() ? more.filter((item) => item.publisher.trim() === entry.publisher.trim()) : [];
  const permissionFact = entry.permissions.length ? t("store.permissionsDeclared", { count: entry.permissions.length }) : t("store.noneDeclared");
  return <div className="store-detail-page">
    <div className="store-detail-bar">
      <Button variant="ghost" size="sm" leftIcon={<ArrowLeftIcon />} onClick={onBack}>{t("store.backTo", { place: backLabel })}</Button>
      <Button variant="ghost" size="sm" onClick={onSource}>{entry.sourceName}<ArrowIcon aria-hidden="true" /></Button>
    </div>

    <section className="store-product-hero" id={sectionId("overview")}>
      <div className="store-product-hero__backdrop" aria-hidden="true"><StoreShot entry={entry} className="store-product-hero__bg" /></div>
      <span className="store-product-hero__scrim" aria-hidden="true" />
      <AppArtwork id={entry.id} title={entry.title} iconUrl={entry.iconUrl} large />
      <div className="store-product-hero__identity">
        <h1>{entry.title}{app ? <span className="store-pill">{t("store.installedShort")}</span> : null}</h1>
        <button type="button" className="store-developer-link" onClick={() => onDeveloper(entry)}>{publisherLabel(entry)}</button>
        <p className="store-product-hero__category">{entry.categories.join(" · ") || t("store.category")} <span>·</span> {entry.sourceName}</p>
      </div>
      <div className="store-product-hero__action">
        <StoreAction entry={entry} app={app} installing={installing} busy={busy} onInstall={onInstall} onOpen={onOpen} />
      </div>
    </section>

    <div className="store-product-glance">
      <p className="store-product-glance__summary">{entry.summary || t("store.noDescription")}</p>
      <div className="store-product-glance__meta">
        <div className="store-product-stats">
          {entry.rating !== null ? <div><strong>{ratingLabel(entry.rating)}</strong><span>{t("store.labelRating")}</span></div> : null}
          {entry.downloads !== null ? <div><strong>{downloadsLabel(locale, entry.downloads)}</strong><span>{t("store.labelDownloads")}</span></div> : null}
          <div><strong>v{entry.version}</strong><span>{t("store.labelVersion")}</span></div>
        </div>
        <div className="store-trust-row">
          <span><DesktopIcon aria-hidden="true" />{t("store.runsLocally")}</span>
          <span><ShieldIcon aria-hidden="true" />{t("store.signed")}</span>
        </div>
        <a className="store-package-link" href={`#${sectionId("package")}`} onClick={(event) => { event.preventDefault(); jump("package"); }}>{t("store.packageDetails")}</a>
      </div>
    </div>

    <nav className="store-detail-jumps" aria-label={t("store.onThisPage")}>
      {[["overview", t("store.overview")], ["preview", t("store.screenshots")], ["about", t("store.about")], ["permissions", t("store.permissions")], ["package", t("store.packageDetails")]].filter(([name]) => name !== "preview" || entry.screenshots.length).map(([name, label]) => <Button key={name} variant="ghost" size="sm" onClick={() => jump(name!)}>{label}</Button>)}
    </nav>

    <section className="store-requirements" aria-labelledby="store-requirements-title">
      <div className="store-requirements__head">
        <div><p className="store-kicker">{t("store.beforeInstall")}</p><h2 id="store-requirements-title">{t("store.requirementsTitle")}</h2></div>
        <p>{t("store.requirementsHelp")}</p>
      </div>
      <div className="store-facts">
        <div className="store-fact"><span className="store-fact__icon"><DesktopIcon aria-hidden="true" /></span><div><span>{t("store.runtime")}</span><strong>{t("store.runsLocally")}</strong><p>{t("store.installedOnDevice")}</p></div></div>
        <div className="store-fact"><span className="store-fact__icon"><InfoIcon aria-hidden="true" /></span><div><span>{t("store.permissions")}</span><strong>{permissionFact}</strong><p>{entry.permissions.length ? t("store.declared") : t("store.noSpecialPermissions")}</p><a href={`#${sectionId("permissions")}`} onClick={(event) => { event.preventDefault(); jump("permissions"); }}>{t("store.seePermissions")}</a></div></div>
        <div className="store-fact"><span className="store-fact__icon"><StoreIcon aria-hidden="true" /></span><div><span>{t("store.source")}</span><strong>{entry.sourceName}</strong><p>{sourceHost ?? ""}</p><a href={`#${sectionId("package")}`} onClick={(event) => { event.preventDefault(); jump("package"); }}>{t("store.packageDetails")}</a></div></div>
      </div>
      <p className="store-requirements__note">{t("store.requirementsNote")}</p>
    </section>

    <section className="store-detail-section" id={sectionId("preview")}>
      <div className="store-detail-section-head"><div><p className="store-kicker">{t("store.preview")}</p><h2>{t("store.seeInAction", { title: entry.title })}</h2></div><p>{t("store.previewHelp")}</p></div>
      <div className="store-preview-grid">
        <div className="store-preview-frame">{entry.screenshots.length ? <button type="button" className="store-shot-open" onClick={(event) => { shotOpener.current = event.currentTarget; setShotIndex(0); }} aria-label={t("store.enlargeScreenshot")}><StorePreview entry={entry} /><span>{t("store.enlargeScreenshot")}</span></button> : <StorePreview entry={entry} />}</div>
        <div className="store-preview-note">
          <span className="store-preview-note__icon"><DesktopIcon aria-hidden="true" /></span>
          <p className="store-kicker">{t("store.localByDesign")}</p>
          <strong>{t("store.localByDesignTitle")}</strong>
          <p>{t("store.localByDesignHelp")}</p>
          <a href={`#${sectionId("permissions")}`} onClick={(event) => { event.preventDefault(); jump("permissions"); }}>{t("store.seePermissions")}<ArrowIcon aria-hidden="true" /></a>
        </div>
      </div>
    </section>

    {entry.screenshots.length > 1 ? <section className="store-detail-section">
      <h2>{t("store.screenshots")}</h2>
      <div className="store-shots">{entry.screenshots.map((shot, index) => <button type="button" key={shot} onClick={(event) => { shotOpener.current = event.currentTarget; setShotIndex(index); }} aria-label={t("store.screenshotAlt", { title: entry.title, index: index + 1 })}><img src={storeImageUrl(shot)} alt="" loading="lazy" /></button>)}</div>
    </section> : null}
    {shotIndex !== null ? <StoreLightbox images={entry.screenshots.map((shot, index) => ({ src: storeImageUrl(shot), alt: t("store.screenshotAlt", { title: entry.title, index: index + 1 }) }))} index={shotIndex} onIndexChange={setShotIndex} onClose={closeShot} title={entry.title} returnFocus={shotOpener.current} /> : null}

    <div className="store-detail-grid">
      <div className="store-detail-main">
        <section className="store-panel" id={sectionId("about")}><p className="store-panel__kicker">{t("store.about")}</p><h3>{entry.title}</h3><p className="store-description">{entry.description || entry.summary || t("store.noDescription")}</p></section>
        {entry.releaseNotes ? <section className="store-panel"><div className="store-panel__head"><h3>{t("store.whatsNew")}</h3><span className="store-tag">v{entry.version}</span></div><p className="store-description">{entry.releaseNotes}</p></section> : null}
        <section className="store-panel" id={sectionId("permissions")}>
          <div className="store-panel__head"><h3>{t("store.permissions")}</h3><span className="store-empty-note">{entry.permissions.length ? t("store.declared") : t("store.nothingRequested")}</span></div>
          <p>{t("store.permissionsHelp")}</p>
          {entry.permissions.length ? <div className="store-tags">{entry.permissions.map((permission) => <span key={permission}>{permission}</span>)}</div> : null}
        </section>
        {developerApps.length ? <section className="store-panel">
          <button type="button" className="store-developer-panel-link" onClick={() => onDeveloper(entry)}><span><span className="store-panel__kicker">{t("store.developer")}</span><strong>{publisherLabel(entry)}</strong></span><ArrowIcon aria-hidden="true" /></button>
          <p>{t("store.moreByDeveloper")}</p>
          <div className="store-more-grid">{developerApps.slice(0, 3).map((item) => <button type="button" className="store-more-card" key={entryKey(item)} onClick={() => onDetails(item)}><AppArtwork id={item.id} title={item.title} iconUrl={item.iconUrl} /><span><strong>{item.title}</strong><small>{item.categories[0] ?? item.sourceName}</small></span></button>)}</div>
        </section> : null}
        {more.filter((item) => !developerApps.includes(item)).length ? <section className="store-panel">
          <div className="store-panel__head"><h3>{t("store.moreFrom", { source: entry.sourceName })}</h3></div>
          <div className="store-more-grid">{more.filter((item) => !developerApps.includes(item)).slice(0, 3).map((item) => <button type="button" className="store-more-card" key={entryKey(item)} onClick={() => onDetails(item)}>
            <AppArtwork id={item.id} title={item.title} iconUrl={item.iconUrl} />
            <span><strong>{item.title}</strong><small>{item.categories[0] ?? item.sourceName}</small></span>
          </button>)}</div>
        </section> : null}
      </div>
      <aside className="store-detail-aside">
        <div className="store-install-panel">
          <StoreAction entry={entry} app={app} installing={installing} busy={busy} onInstall={onInstall} onOpen={onOpen} />
          <p className="store-install-note"><ShieldIcon aria-hidden="true" />{t("store.signedAndVerified")}</p>
        </div>
        <div className="store-package-panel" id={sectionId("package")}>
          <div className="store-package-panel__head"><span><InfoIcon aria-hidden="true" /></span><strong>{t("store.sourcePackage")}</strong></div>
          <GroupContainer>
            <GroupRow title={t("store.labelSource")} value={entry.sourceName} description={sourceHost ?? undefined} />
            <GroupRow title={t("store.labelPublisher")} value={entry.publisher || "—"} />
            <GroupRow title={t("store.labelVersion")} value={`v${entry.version}`} />
            {entry.categories.length ? <GroupRow title={t("store.category")} value={entry.categories.join(", ")} /> : null}
            {entry.rating !== null ? <GroupRow title={t("store.labelRating")} value={ratingLabel(entry.rating)} /> : null}
            {entry.downloads !== null ? <GroupRow title={t("store.labelDownloads")} value={downloadsLabel(locale, entry.downloads)} /> : null}
            <GroupRow title={t("store.labelAppId")} description={entry.id} />
          </GroupContainer>
        </div>
        <div className="store-trust"><ShieldIcon aria-hidden="true" /><div><strong>{t("store.trustedPackages")}</strong><p>{t("store.trustedPackagesHelp")}</p></div></div>
      </aside>
    </div>
  </div>;
}

export function StoreApp() {
  const { t, locale } = useI18n();
  const { live, open, state } = useShell();
  const { tuning } = useThemeTuning();
  const windowLocation = useWindowLocation();
  const topbarHost = useAppTopbarHost();
  const sources = useStoreSources();
  const catalog = useAppCatalog();
  const installFeature = useFeature("app.install");
  const [states, setStates] = useState<Record<string, SourceState>>({});
  const [sourceFilter, setSourceFilter] = useState<string>("all");
  const [categoryFilter, setCategoryFilter] = useState<string | null>(null);
  const [view, setView] = useState<StoreView>("discover");
  const [query, setQuery] = useState("");
  const [searchFocusTick, setSearchFocusTick] = useState(0);
  const [reload, setReload] = useState(0);
  const [installing, setInstalling] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [developerKey, setDeveloperKey] = useState<string | null>(null);
  const [developerOriginKey, setDeveloperOriginKey] = useState<string | null>(null);
  const [coverIndex, setCoverIndex] = useState(0);
  const [certificate, setCertificate] = useState<{ host: string; fingerprint: string; sourceName: string } | null>(null);
  const [trusting, setTrusting] = useState(false);
  const [trustError, setTrustError] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);
  const searchFieldRef = useRef<HTMLDivElement>(null);
  const returnScroll = useRef<number | null>(null);
  const fullscreen = windowLocation !== null && state.windows.some((window) => window.location === windowLocation && window.fullscreen === true);

  useEffect(() => {
    if (!live) {
      setStates({});
      return;
    }
    const controller = new AbortController();
    const enabled = sources.sources.filter((source) => source.enabled);
    setStates((previous) => Object.fromEntries(enabled.map((source) => [source.id, previous[source.id] ?? { status: "loading", apps: [] }])));
    let alive = true;
    void Promise.all(enabled.map(async (source) => {
      try {
        const index = await fetchStoreIndex(live.request, source, controller.signal);
        if (alive) setStates((previous) => ({ ...previous, [source.id]: { status: "ready", apps: index.apps } }));
      } catch (failure) {
        if (alive) {
          if (failure instanceof UntrustedCertificateError) {
            setCertificate((current) => current ?? { host: failure.host, fingerprint: failure.fingerprint, sourceName: source.name });
          }
          setStates((previous) => ({ ...previous, [source.id]: { status: "error", apps: [] } }));
        }
      }
    }));
    return () => { alive = false; controller.abort(); };
  }, [live, sources.sources, reload]);

  useEffect(() => {
    if (sourceFilter !== "all" && !sources.sources.some((source) => source.id === sourceFilter && source.enabled)) {
      setSourceFilter("all");
      setSelectedKey(null);
      setDeveloperKey(null);
      setDeveloperOriginKey(null);
    }
  }, [sourceFilter, sources.sources]);

  const scrollSurface = () => contentRef.current;
  useEffect(() => {
    if (!searchFocusTick) return;
    requestAnimationFrame(() => {
      const input = searchFieldRef.current?.querySelector("input");
      if (input && input.offsetParent !== null) input.focus();
    });
  }, [searchFocusTick]);
  const resetDeveloper = () => {
    setDeveloperKey(null);
    setDeveloperOriginKey(null);
  };
  const openDetails = (entry: StoreAppEntry) => {
    if (!selectedKey) returnScroll.current = scrollSurface()?.scrollTop ?? 0;
    setSelectedKey(entryKey(entry));
    scrollSurface()?.scrollTo({ top: 0 });
  };
  const openDeveloper = (entry: StoreAppEntry) => {
    const key = publisherKey(entry);
    if (!key) return;
    setDeveloperOriginKey(selectedKey);
    setSelectedKey(null);
    setDeveloperKey(key);
    scrollSurface()?.scrollTo({ top: 0 });
  };
  const closeDeveloper = () => {
    const origin = developerOriginKey;
    setDeveloperKey(null);
    setDeveloperOriginKey(null);
    setSelectedKey(origin);
    scrollSurface()?.scrollTo({ top: 0 });
  };
  const closeDetails = () => {
    returnScroll.current = returnScroll.current ?? 0;
    setSelectedKey(null);
  };
  useEffect(() => {
    if (selectedKey) return;
    const scroller = scrollSurface();
    if (returnScroll.current !== null) {
      const top = returnScroll.current;
      returnScroll.current = null;
      requestAnimationFrame(() => scroller?.scrollTo({ top }));
      return;
    }
    scroller?.scrollTo({ top: 0 });
  }, [view, sourceFilter, categoryFilter, query, selectedKey]);

  useEffect(() => {
    if (!selectedKey) return;
    const close = (event: KeyboardEvent) => { if (event.key === "Escape" && !document.querySelector("dialog[open], .store-lightbox")) { returnScroll.current = returnScroll.current ?? 0; setSelectedKey(null); } };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, [selectedKey]);

  const enabledSources = useMemo(() => sources.sources.filter((source) => source.enabled), [sources.sources]);
  const allEntries = useMemo(() => enabledSources.flatMap((source) => states[source.id]?.apps ?? []), [enabledSources, states]);
  const loading = Boolean(live) && enabledSources.some((source) => !states[source.id] || states[source.id]?.status === "loading");
  const failing = enabledSources.filter((source) => states[source.id]?.status === "error");
  const installed = (entry: StoreAppEntry) => catalog.apps.find((app) => app.id === entry.id);
  const selected = selectedKey ? allEntries.find((entry) => entryKey(entry) === selectedKey) ?? null : null;
  const selectedApp = selected ? installed(selected) : undefined;
  const selectedSource = selected ? enabledSources.find((source) => source.id === selected.sourceId) : undefined;
  const developerEntries = useMemo(() => developerKey ? allEntries.filter((entry) => publisherKey(entry) === developerKey) : [], [allEntries, developerKey]);
  const developer = developerKey && developerEntries.length ? { name: publisherLabel(developerEntries[0]!), entries: developerEntries } : null;
  const developerOrigin = developerOriginKey ? allEntries.find((entry) => entryKey(entry) === developerOriginKey) ?? null : null;
  const developerBackLabel = developerOrigin?.title ?? t("store.allApps");
  const home = !selected && !developer && view === "discover" && sourceFilter === "all" && !query.trim() && categoryFilter === null;

  useEffect(() => {
    if (selectedKey && !loading && !selected) setSelectedKey(null);
  }, [loading, selected, selectedKey]);

  const prioritizedEntries = useMemo(() => [...allEntries].sort((left, right) =>
    Number(right.featured) - Number(left.featured) ||
    (right.downloads ?? -1) - (left.downloads ?? -1) ||
    (right.rating ?? 0) - (left.rating ?? 0) ||
    left.title.localeCompare(right.title)), [allEntries]);
  const coverEntries = useMemo(() => {
    const featured = prioritizedEntries.filter((entry) => entry.featured);
    const pool = featured.length ? featured : prioritizedEntries;
    const picks: StoreAppEntry[] = [];
    for (const source of enabledSources) {
      const pick = pool.find((entry) => entry.sourceId === source.id && !picks.some((chosen) => entryKey(chosen) === entryKey(entry)));
      if (pick) picks.push(pick);
      if (picks.length === 3) break;
    }
    for (const entry of pool) {
      if (picks.length === 3) break;
      if (!picks.some((chosen) => entryKey(chosen) === entryKey(entry))) picks.push(entry);
    }
    return picks;
  }, [enabledSources, prioritizedEntries]);
  const coverIds = useMemo(() => new Set(coverEntries.map(entryKey)), [coverEntries]);
  const spotlightEntries = useMemo(() => prioritizedEntries.filter((entry) => !coverIds.has(entryKey(entry))).slice(0, 3), [coverIds, prioritizedEntries]);
  const coverKey = coverEntries.map(entryKey).join("|");
  useEffect(() => setCoverIndex(0), [coverKey]);
  const chartEntries = useMemo(() => {
    const ranked = [...allEntries]
      .filter((entry) => entry.downloads !== null)
      .sort((left, right) => (right.downloads ?? 0) - (left.downloads ?? 0))
      .slice(0, 5);
    return ranked.length >= 3 ? ranked : [];
  }, [allEntries]);
  const chartIds = useMemo(() => new Set(chartEntries.map(entryKey)), [chartEntries]);
  const editEntries = useMemo(() => {
    const picks: StoreAppEntry[] = [];
    const used = new Set(coverIds);
    const represented = new Set<string>(coverEntries.flatMap((entry) => entry.categories));
    const add = (entry: StoreAppEntry) => {
      if (picks.length >= 3 || used.has(entryKey(entry))) return false;
      picks.push(entry);
      used.add(entryKey(entry));
      entry.categories.forEach((category) => represented.add(category));
      return true;
    };
    for (const entry of prioritizedEntries) {
      if (picks.length >= 3) break;
      if (!entry.featured || chartIds.has(entryKey(entry))) continue;
      add(entry);
    }
    for (const entry of prioritizedEntries) {
      if (picks.length >= 3) break;
      if (used.has(entryKey(entry)) || chartIds.has(entryKey(entry))) continue;
      if (entry.categories.some((category) => !represented.has(category))) add(entry);
    }
    for (const entry of prioritizedEntries) {
      if (picks.length >= 3) break;
      if (used.has(entryKey(entry)) || chartIds.has(entryKey(entry))) continue;
      add(entry);
    }
    return picks;
  }, [chartIds, coverEntries, coverIds, prioritizedEntries]);
  const editIds = useMemo(() => new Set(editEntries.map(entryKey)), [editEntries]);
  const registryEntries = useMemo(() => {
    const candidates = prioritizedEntries
      .filter((entry) => !coverIds.has(entryKey(entry)) && !editIds.has(entryKey(entry)) && !chartIds.has(entryKey(entry)))
      .sort((left, right) => (right.rating ?? 0) - (left.rating ?? 0) || (right.downloads ?? 0) - (left.downloads ?? 0) || left.title.localeCompare(right.title))
      .slice(0, 6);
    return candidates.length >= 2 ? candidates : [];
  }, [chartIds, coverIds, editIds, prioritizedEntries]);
  const categoryEntries = useMemo(() => {
    const grouped = new Map<string, StoreAppEntry[]>();
    for (const entry of allEntries) for (const category of entry.categories) {
      const apps = grouped.get(category) ?? [];
      apps.push(entry);
      grouped.set(category, apps);
    }
    return Array.from(grouped, ([category, apps]) => {
      const lead = apps.find((entry) => entry.screenshots.length > 0) ?? apps[0];
      return { category, apps, lead };
    }).sort((left, right) => right.apps.length - left.apps.length || left.category.localeCompare(right.category));
  }, [allEntries]);
  const sourceEntries = useMemo(() => sourceFilter === "all" ? allEntries : allEntries.filter((entry) => entry.sourceId === sourceFilter), [allEntries, sourceFilter]);
  const categories = useMemo(() => Array.from(new Set(sourceEntries.flatMap((entry) => entry.categories))).sort((left, right) => left.localeCompare(right)), [sourceEntries]);
  const entries = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    return sourceEntries
      .filter((entry) => categoryFilter === null || entry.categories.includes(categoryFilter))
      .filter((entry) => !normalized || `${entry.title} ${entry.id} ${entry.publisher} ${entry.sourceName} ${entry.categories.join(" ")}`.toLocaleLowerCase().includes(normalized))
      .sort((left, right) => left.title.localeCompare(right.title));
  }, [sourceEntries, categoryFilter, query]);
  const moreFromSource = useMemo(() => selected ? allEntries.filter((entry) => entry.sourceId === selected.sourceId && entryKey(entry) !== selectedKey) : [], [allEntries, selected, selectedKey]);
  const libraryEntries = useMemo(() => catalog.apps.map((app) => ({ app, entry: allEntries.find((entry) => entry.id === app.id) })).filter(({ app, entry }) => !query.trim() || `${app.title} ${app.id} ${entry?.publisher ?? ""}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())), [allEntries, catalog.apps, query]);
  const today = new Intl.DateTimeFormat(locale, { weekday: "long", month: "long", day: "numeric" }).format(new Date());
  const sourceName = sourceFilter === "all" ? t("store.allSources") : enabledSources.find((source) => source.id === sourceFilter)?.name ?? t("store.sources");
  const primarySource = enabledSources.length === 1 ? enabledSources[0] : null;
  const selectedSourceHost = selectedSource ? new URL(selectedSource.url).hostname : null;

  const discover = () => {
    setView("discover");
    setSourceFilter("all");
    setCategoryFilter(null);
    setQuery("");
    setSelectedKey(null);
    resetDeveloper();
  };
  const showOverview = (next: "categories" | "sources") => {
    setView(next);
    setSourceFilter("all");
    setCategoryFilter(null);
    setQuery("");
    setSelectedKey(null);
    resetDeveloper();
  };
  const browseAll = () => {
    setView("all");
    setSourceFilter("all");
    setCategoryFilter(null);
    setQuery("");
    setSelectedKey(null);
    resetDeveloper();
  };
  const openSearch = () => {
    setView("search");
    setSourceFilter("all");
    setCategoryFilter(null);
    setSelectedKey(null);
    resetDeveloper();
    setSearchFocusTick((value) => value + 1);
  };
  const mobileIslandItems: MobileIslandNavItem[] = [
    { id: "discover", label: t("store.discover"), icon: <SparkIcon aria-hidden="true" /> },
    { id: "all", label: t("store.allApps"), icon: <GridIcon aria-hidden="true" /> },
    { id: "categories", label: t("store.categories"), icon: <SlidersIcon aria-hidden="true" /> },
    { id: "library", label: t("store.myApps"), icon: <WindowStackIcon aria-hidden="true" /> },
    { id: "sources", label: t("store.sources"), icon: <StoreIcon aria-hidden="true" /> },
    { id: "search", label: t("store.searchTab"), icon: <SearchIcon aria-hidden="true" /> }
  ];
  const navigateFromIsland = (id: string) => {
    if (id === "discover") discover();
    else if (id === "all") browseAll();
    else if (id === "categories") showOverview("categories");
    else if (id === "library") showLibrary();
    else if (id === "sources") showOverview("sources");
    else if (id === "search") openSearch();
  };
  const showLibrary = () => {
    setView("library");
    setSourceFilter("all");
    setCategoryFilter(null);
    setQuery("");
    setSelectedKey(null);
    resetDeveloper();
  };
  const selectSource = (id: string) => {
    setView("all");
    setSourceFilter(id);
    setCategoryFilter(null);
    setQuery("");
    setSelectedKey(null);
    resetDeveloper();
  };
  const selectCategory = (category: string) => {
    setView("all");
    setCategoryFilter(category);
    setQuery("");
    setSelectedKey(null);
    resetDeveloper();
  };
  const clearCategory = () => {
    setCategoryFilter(null);
    setQuery("");
    setSelectedKey(null);
    resetDeveloper();
  };
  const updateQuery = (value: string) => {
    setQuery(value);
    setSelectedKey(null);
    resetDeveloper();
    if (view !== "library" && view !== "search") setView("search");
  };

  const openInstalled = (app: InstalledApp) => {
    open(appPath(app.id));
  };

  const install = async (entry: StoreAppEntry) => {
    if (!live || installing || installFeature.state !== "ready") return;
    setInstalling(entryKey(entry));
    setError(null);
    const controller = new AbortController();
    try {
      const files = await fetchStorePackage(live.request, entry, controller.signal);
      await importPackage(live.request, files, browserProfile().id, controller.signal, "store");
      window.dispatchEvent(new Event(CATALOG_REFRESH_EVENT));
      showToast({ title: entry.title, message: t("store.installed"), variant: "success" });
    } catch (failure) {
      if (failure instanceof UntrustedCertificateError) {
        setCertificate({ host: failure.host, fingerprint: failure.fingerprint, sourceName: entry.sourceName });
      } else {
        setError(t("store.installFailed", { title: entry.title }));
      }
    } finally {
      setInstalling(null);
    }
  };

  const confirmCertificate = async () => {
    if (!live || !certificate || trusting) return;
    setTrusting(true);
    setTrustError(false);
    const controller = new AbortController();
    try {
      await trustStoreCertificate(live.request, certificate.host, certificate.fingerprint, controller.signal);
      setCertificate(null);
      setReload((value) => value + 1);
    } catch {
      setTrustError(true);
    } finally {
      setTrusting(false);
    }
  };

  const searching = view === "search";
  const returnLabel = searching ? query.trim() ? t("store.results") : t("store.search") : query.trim() ? t("store.results") : view === "library" ? t("store.myApps") : sourceFilter !== "all" ? sourceName : categoryFilter ?? (view === "all" ? t("store.allApps") : t("store.discover"));
  const detailBackLabel = developer ? developer.name : returnLabel;
  const headerKicker = searching ? query.trim() ? t("store.results") : t("store.searchTab") : view === "categories" ? t("store.browse") : view === "sources" ? t("store.catalog") : selected ? t("store.catalog") : query.trim() ? t("store.results") : view === "library" ? t("store.library") : sourceFilter !== "all" ? t("store.source") : categoryFilter ? t("store.category") : view === "all" ? t("store.catalog") : t("store.today");
  const headerTitle = searching ? query.trim() ? t("store.resultsFor", { query: query.trim() }) : t("store.search") : view === "categories" ? t("store.categories") : view === "sources" ? t("store.sources") : selected ? selected.sourceName : query.trim() ? t("store.resultsFor", { query: query.trim() }) : view === "library" ? t("store.myApps") : sourceFilter !== "all" ? sourceName : categoryFilter ?? (view === "all" ? t("store.allApps") : t("store.discover"));
  const headerHelp = searching ? query.trim() ? t("store.appCount", { count: entries.length }) : t("store.searchHelp") : view === "categories" ? t("store.categoriesHelp") : view === "sources" ? t("store.sourcesHelp") : selected ? selectedSourceHost ?? selected.sourceName : query.trim() ? t("store.appCount", { count: entries.length }) : view === "library" ? t("store.libraryHelp") : sourceFilter !== "all" ? t("store.appCount", { count: sourceEntries.length }) : categoryFilter ? t("store.appCount", { count: entries.length }) : view === "all" ? t("store.allAppsHelp") : t("store.headlineHelp");
  const headerAside = searching ? loading ? t("store.loading") : query.trim() ? "" : t("store.appCount", { count: allEntries.length }) : view === "sources" || view === "categories" ? "" : loading ? t("store.loading") : view === "library" ? t("store.appCount", { count: libraryEntries.length }) : query.trim() || sourceFilter !== "all" || categoryFilter ? "" : view === "all" ? t("store.appCount", { count: entries.length }) : today;

  const searchLabel = view === "library" ? t("store.searchInstalled") : sourceFilter !== "all" ? t("store.searchInSource", { source: sourceName }) : t("store.search");
  const topbar = <div className="store-topbar">
    <span className="store-topbar__nav-surface" aria-hidden="true" />
    <div className="store-topbar__brand">
      <span className="store-topbar__mark" aria-hidden="true"><StoreIcon /></span>
      <strong>{t("store.title")}</strong>
    </div>
    <div className="store-topbar__search">
      <RumahlInputGroup prefix={<SearchIcon aria-hidden="true" />} type="search" value={query} onChange={(event) => updateQuery(event.target.value)} placeholder={searchLabel} aria-label={searchLabel}
        suffix={query ? <IconButton label={t("store.clearSearch")} size="xs" variant="ghost" onClick={() => updateQuery("")}>×</IconButton> : undefined} />
    </div>
    <IconButton label={t("store.refresh")} title={t("store.refresh")} size="sm" variant="ghost" loading={loading} onClick={() => setReload((value) => value + 1)}><RefreshIcon /></IconButton>
  </div>;

  return <>
    {topbarHost ? createPortal(topbar, topbarHost) : null}
    <main className="store-app" data-background-motion={tuning.wallpaperMotion === false ? "false" : "true"}>
      {fullscreen || !topbarHost ? <div className="store-inline-tools">
        <div className="store-inline-tools__search"><RumahlInputGroup prefix={<SearchIcon aria-hidden="true" />} type="search" value={query} onChange={(event) => updateQuery(event.target.value)} placeholder={searchLabel} aria-label={searchLabel} /></div>
        <IconButton label={t("store.refresh")} title={t("store.refresh")} size="sm" variant="ghost" loading={loading} onClick={() => setReload((value) => value + 1)}><RefreshIcon /></IconButton>
      </div> : null}

      <AppNavLayout
        className="store-nav-layout"
        contentClassName="store-content"
        contentRef={contentRef}
        sidebar={
        <aside className="rumahl-sidenav store-sidebar">
          <div className="store-sidebar__inner">
          <SidenavSection>
            <SidenavItem as="button" type="button" aria-current={view === "discover" ? "page" : undefined} onClick={discover} icon={<SparkIcon />}>{t("store.discover")}</SidenavItem>
            <SidenavItem as="button" type="button" aria-current={view === "all" && sourceFilter === "all" ? "page" : undefined} onClick={browseAll} icon={<GridIcon />}>{t("store.allApps")}</SidenavItem>
            <SidenavItem as="button" type="button" current={view === "categories"} onClick={() => showOverview("categories")} icon={<GridIcon />}>{t("store.categories")}</SidenavItem>
          </SidenavSection>
          <SidenavSection title={t("store.onDevice")}>
            <SidenavItem as="button" type="button" aria-current={view === "library" ? "page" : undefined} onClick={showLibrary} icon={<WindowStackIcon />}>{t("store.myApps")}</SidenavItem>
          </SidenavSection>
          <SidenavSection title={t("store.sources")}>
            <SidenavItem as="button" type="button" current={view === "sources"} onClick={() => showOverview("sources")} icon={<StoreIcon />}>{t("store.sourcesOverview")}</SidenavItem>
            {enabledSources.map((source) => {
              const sourceState = states[source.id];
              const count = sourceState?.apps.length ?? 0;
              return <SidenavItem as="button" type="button" key={source.id} className="store-source-item" aria-current={sourceFilter === source.id ? "page" : undefined} onClick={() => selectSource(source.id)}
                icon={<span className="store-status-dot" data-state={sourceState?.status ?? "loading"} />}>
                <span className="store-source-name">{source.name}</span>
                <span className="store-source-count">{sourceState?.status === "error" ? "!" : sourceState?.status === "loading" ? "…" : count}</span>
              </SidenavItem>;
            })}
            <SidenavItem as="button" type="button" onClick={() => open("/settings/store")} icon={<SettingsIcon />}>{t("store.manageSources")}</SidenavItem>
          </SidenavSection>
          </div>
        </aside>
        }
        mobileNav={{
          label: t("store.navigation"),
          items: mobileIslandItems,
          value: view,
          onChange: navigateFromIsland
        }}
      >
          {selected ? <StoreDetail entry={selected} app={selectedApp} source={selectedSource} more={moreFromSource} installing={installing === entryKey(selected)} busy={installing !== null || installFeature.state !== "ready"}
            backLabel={detailBackLabel} onSource={() => selectSource(selected.sourceId)} onBack={closeDetails} onDetails={openDetails} onDeveloper={openDeveloper} onInstall={install} onOpen={openInstalled} /> : developer ? <StoreDeveloper developer={developer.name} entries={developer.entries} backLabel={developerBackLabel} installing={installing} busy={installing !== null || installFeature.state !== "ready"} installed={installed} onBack={closeDeveloper} onDetails={openDetails} onInstall={install} onOpen={openInstalled} /> : <>
            <header className="store-page-head">
              <div>
                <p className="store-kicker">{headerKicker}</p>
                <h1>{headerTitle}</h1>
                <p>{headerHelp}</p>
              </div>
              <div className="store-page-head__aside"><span>{headerAside}</span>{home && !loading ? <small>{t("store.appCount", { count: allEntries.length })} · {t("store.sourceCount", { count: enabledSources.length })}</small> : null}</div>
            </header>

            {!home && view === "all" ? <div className="store-browse-context">
              <Chip selected={sourceFilter === "all"} onClick={() => setSourceFilter("all")}>{t("store.allSources")}</Chip>
              {sourceFilter !== "all" ? <Chip selected onClick={() => selectSource(sourceFilter)} onRemove={() => setSourceFilter("all")} removeLabel={t("store.clearSource")}>{sourceName}</Chip> : null}
              {categoryFilter ? <Chip selected onRemove={clearCategory} removeLabel={t("store.clearCategory")}>{categoryFilter}</Chip> : null}
              <span>{t("store.appCount", { count: entries.length })}</span>
            </div> : null}

            {failing.map((source) => <p className="store-source-error" role="status" key={source.id}>{t("store.sourceUnavailable", { name: source.name })}</p>)}
            {error ? <p className="store-source-error" role="alert">{error}</p> : null}
            {installFeature.state !== "ready" ? <p className="store-source-error" role="status">{t("store.installUnavailable")}</p> : null}
            {view !== "library" && sources.sources.length === 0 ? <section className="store-empty"><StoreIcon aria-hidden="true" /><h2>{t("store.noSources")}</h2><p>{t("store.noSourcesHelp")}</p><Button variant="primary" onClick={() => open("/settings/store")}>{t("store.addSource")}</Button></section> : null}
            {view !== "library" && loading && allEntries.length === 0 ? <div className="store-skeleton" role="status" aria-label={t("store.loading")}><div /><div /></div> : null}

            {view === "search" ? <div className="store-search-page">
              <div className="store-search-page__field" ref={searchFieldRef}>
                <RumahlInputGroup prefix={<SearchIcon aria-hidden="true" />} type="search" value={query} onChange={(event) => updateQuery(event.target.value)} placeholder={t("store.search")} aria-label={t("store.search")}
                  suffix={query ? <IconButton label={t("store.clearSearch")} size="xs" variant="ghost" onClick={() => updateQuery("")}>×</IconButton> : undefined} />
              </div>
              {query.trim()
                ? entries.length === 0 && !loading ? <section className="store-empty"><SearchIcon aria-hidden="true" /><h2>{t("store.empty")}</h2><p>{t("store.emptyHelp")}</p></section>
                  : <div className="store-result-list">{entries.map((entry) => <StoreResultRow key={entryKey(entry)} entry={entry} app={installed(entry)} installing={installing === entryKey(entry)} busy={installing !== null || installFeature.state !== "ready"} onDetails={openDetails} onInstall={install} onOpen={openInstalled} />)}</div>
                : <section className="store-search-prompt"><span><SearchIcon aria-hidden="true" /></span><p className="store-kicker">{t("store.searchTab")}</p><h2>{t("store.exploreApps")}</h2><p>{t("store.searchPrompt")}</p></section>}
            </div> : view === "sources" ? <>
              <section className="store-panel"><h2>{t("store.howStoreWorks")}</h2><p>{t("store.sourceWorkflow")}</p></section>
              <div className="store-source-overview">{sources.sources.map((source) => <section className="store-panel" key={source.id}>
                <div className="store-panel__head"><h3>{source.name}</h3><span className="store-tag">{!source.enabled ? t("store.sourceDisabled") : states[source.id]?.status === "error" ? t("store.sourceFailed") : states[source.id]?.status === "ready" ? t("store.appCount", { count: states[source.id]?.apps.length ?? 0 }) : t("store.loading")}</span></div>
                <p className="store-source-url">{source.url}</p>
                <div className="store-source-overview__actions"><Button size="sm" disabled={!source.enabled} onClick={() => selectSource(source.id)}>{t("store.browseSource")}</Button><Button variant="ghost" size="sm" onClick={() => open("/settings/store")}>{t("store.manageSources")}</Button></div>
              </section>)}</div>
            </> : view === "categories" ? <section className="store-category-overview">
              {categoryEntries.map((group, index) => <StoreCategoryCard key={group.category} category={group.category} apps={group.apps} lead={group.lead} index={index} onSelect={selectCategory} />)}
            </section> : home ? <>
              <div className="store-home">
                {coverEntries.length ? <StoreShowcase entries={coverEntries} index={coverIndex} onChange={setCoverIndex} spotlightEntries={spotlightEntries} installing={installing} busy={installing !== null || installFeature.state !== "ready"} installed={installed} onDetails={openDetails} onInstall={install} onOpen={openInstalled} /> : null}

                {(editEntries.length || chartEntries.length) ? <section className="store-section store-edit-section">
                  <div className={`store-edit-layout${editEntries.length && chartEntries.length ? "" : " store-edit-layout--solo"}`}>
                    {editEntries.length ? <div className="store-edit-main">
                      <StoreSectionHead kicker={t("store.curated")} title={t("store.theEdit")} action={<Button variant="ghost" size="sm" onClick={browseAll} rightIcon={<ArrowIcon />}>{t("store.viewAll")}</Button>} />
                      <div className="store-edit-grid">{editEntries.map((entry, index) => <StoreEditCard key={entryKey(entry)} lead={index === 0} entry={entry} app={installed(entry)} installing={installing === entryKey(entry)} busy={installing !== null || installFeature.state !== "ready"} onDetails={openDetails} onInstall={install} onOpen={openInstalled} />)}</div>
                    </div> : null}
                    {chartEntries.length ? <aside className="store-chart-column">
                      <StoreSectionHead kicker={t("store.popular")} title={t("store.topCharts")} action={<Button variant="ghost" size="sm" onClick={browseAll} rightIcon={<ArrowIcon />}>{t("store.viewAll")}</Button>} />
                      <div className="store-chart-panel"><div className="store-chart-list">{chartEntries.map((entry, index) => <StoreChartRow key={entryKey(entry)} entry={entry} rank={index + 1} app={installed(entry)} installing={installing === entryKey(entry)} busy={installing !== null || installFeature.state !== "ready"} onDetails={openDetails} onInstall={install} onOpen={openInstalled} />)}</div></div>
                      <button type="button" className="store-chart-source" onClick={() => open("/settings/store")}>
                        <span className="store-chart-source__icon"><ShieldIcon aria-hidden="true" /></span>
                        <span><strong>{primarySource?.name ?? t("store.trustedPackages")}</strong><small>{primarySource ? `${t("store.appCount", { count: allEntries.length })} · ${t("store.signedBeforeInstall")}` : t("store.sourceCount", { count: enabledSources.length })}</small></span>
                      </button>
                      <div className="store-principle">
                        <span className="store-principle__icon"><ShieldIcon aria-hidden="true" /></span>
                        <p className="store-kicker">{t("store.principle")}</p>
                        <strong>{t("store.principleTitle")}</strong>
                        <p>{t("store.principleHelp")}</p>
                        <button type="button" onClick={() => showOverview("sources")}>{t("store.howStoreWorks")}<ArrowIcon aria-hidden="true" /></button>
                      </div>
                    </aside> : null}
                  </div>
                </section> : null}

                {categoryEntries.length >= 2 ? <StoreRailSection kicker={t("store.browse")} title={t("store.collections")} action={<Button variant="ghost" size="sm" onClick={() => showOverview("categories")} rightIcon={<ArrowIcon />}>{t("store.allCategories")}</Button>}>
                  {categoryEntries.slice(0, 12).map((category, index) => <StoreCategoryCard key={category.category} category={category.category} apps={category.apps} lead={category.lead} index={index} onSelect={selectCategory} />)}
                </StoreRailSection> : null}

                {categoryEntries.filter((group) => group.apps.length >= 2).slice(0, 3).map((group) => <StoreRailSection key={group.category} className="store-discover-apps" kicker={t("store.exploreCategory")} title={group.category} action={<Button variant="ghost" size="sm" rightIcon={<ArrowIcon />} onClick={() => selectCategory(group.category)}>{t("store.viewAll")}</Button>}>
                  {group.apps.slice(0, 6).map((entry) => <StoreGridCard key={entryKey(entry)} entry={entry} app={installed(entry)} installing={installing === entryKey(entry)} busy={installing !== null || installFeature.state !== "ready"} onDetails={openDetails} onInstall={install} onOpen={openInstalled} />)}
                </StoreRailSection>)}

                {registryEntries.length ? <section className="store-section">
                  <StoreSectionHead kicker={t("store.registry")} title={t("store.fromRegistry")} action={<Button variant="ghost" size="sm" onClick={browseAll} rightIcon={<ArrowIcon />}>{t("store.viewAll")}</Button>} />
                  <div className="store-registry-panel"><div className="store-registry-grid">{registryEntries.map((entry) => <StoreRegistryRow key={entryKey(entry)} entry={entry} onDetails={openDetails} />)}</div></div>
                </section> : null}

                <footer className="store-trust-footer">
                  <span className="store-trust-footer__icon"><ShieldIcon aria-hidden="true" /></span>
                  <span className="store-trust-footer__copy"><strong>{t("store.trustedPackages")}</strong><span>{t("store.trustedPackagesHelp")}</span></span>
                  <Button variant="ghost" size="sm" onClick={() => open("/settings/store")}>{t("store.manageSources")}</Button>
                </footer>
              </div>
            </> : view === "library" ? <>
              {catalog.status === "loading" ? <div className="store-skeleton" role="status" aria-label={t("store.loading")}><div /><div /></div>
                : libraryEntries.length === 0 ? <section className="store-empty"><StoreIcon aria-hidden="true" /><h2>{query.trim() ? t("store.empty") : t("store.noInstalled")}</h2><p>{query.trim() ? t("store.emptyHelp") : t("store.noInstalledHelp")}</p><Button variant="primary" onClick={browseAll}>{t("store.allApps")}</Button></section>
                : <section className="store-grid store-library-grid">{libraryEntries.map(({ app, entry }) => <article className="store-grid-card store-library-card" key={app.id}>
                  <div className="store-grid-card__main">
                    <span className="store-grid-card__head">
                      <AppArtwork id={app.id} title={app.title} iconUrl={entry?.iconUrl} />
                      <span className="store-grid-card__title"><strong>{app.title}</strong><small>{entry?.sourceName ?? t("store.sourceNotInCatalog")}</small></span>
                    </span>
                    <span className="store-grid-card__summary">{entry?.summary || t("store.installedApp")}</span>
                    <span className="store-grid-card__meta"><span className="store-tag">v{app.version}</span>{entry?.rating !== null && entry?.rating !== undefined ? <span className="store-rating">{ratingLabel(entry.rating)}</span> : null}</span>
                  </div>
                  <div className="store-library-actions">{entry ? <Button variant="ghost" size="sm" onClick={() => openDetails(entry)}>{t("store.details")}</Button> : null}<Button variant="secondary" size="sm" rightIcon={<ArrowIcon />} onClick={() => openInstalled(app)}>{t("store.open")}</Button></div>
                </article>)}</section>}
            </> : <>
              {!query.trim() && categories.length ? <div className="store-categories" role="group" aria-label={t("store.categories")}>
                <Chip selected={categoryFilter === null} onClick={clearCategory}>{t("store.allCategories")}</Chip>
                {categories.map((category) => <Chip key={category} selected={categoryFilter === category} onClick={() => selectCategory(category)}>{category}</Chip>)}
              </div> : null}
              {entries.length === 0 && !loading ? <section className="store-empty"><h2>{t("store.empty")}</h2><p>{t("store.emptyHelp")}</p></section>
                : query.trim() ? <div className="store-result-list">{entries.map((entry) => <StoreResultRow key={entryKey(entry)} entry={entry} app={installed(entry)} installing={installing === entryKey(entry)} busy={installing !== null || installFeature.state !== "ready"} onDetails={openDetails} onInstall={install} onOpen={openInstalled} />)}</div>
                : <section className="store-grid">{entries.map((entry) => <StoreGridCard key={entryKey(entry)} entry={entry} app={installed(entry)} installing={installing === entryKey(entry)} busy={installing !== null || installFeature.state !== "ready"} onDetails={openDetails} onInstall={install} onOpen={openInstalled} />)}</section>}
            </>}
            {view !== "library" && sources.sources.length > 0 && !loading && allEntries.length === 0 && failing.length === 0 ? <section className="store-empty"><h2>{t("store.empty")}</h2><p>{t("store.emptyHelp")}</p></section> : null}
          </>}
      </AppNavLayout>

      {certificate ? <RumahlModal open kind="alert" onClose={() => { if (!trusting) setCertificate(null); }} title={t("store.certificate.title")} description={t("store.certificate.help", { host: certificate.host })} application={t("store.brandName")} closeLabel={t("store.certificate.cancel")}><div className="store-certificate">
        <dl>
          <div><dt>{t("store.certificate.source")}</dt><dd>{certificate.sourceName}</dd></div>
          <div><dt>{t("store.certificate.host")}</dt><dd>{certificate.host}</dd></div>
          <div><dt>{t("store.certificate.fingerprint")}</dt><dd><code>{formatCertificateFingerprint(certificate.fingerprint)}</code></dd></div>
        </dl>
        <p className="store-certificate__warning">{t("store.certificate.warning")}</p>
        {trustError ? <p className="store-source-error" role="alert">{t("store.certificate.failed")}</p> : null}
        <div className="store-certificate__actions">
          <Button variant="secondary" disabled={trusting} onClick={() => setCertificate(null)}>{t("store.certificate.cancel")}</Button>
          <Button variant="primary" disabled={trusting} onClick={() => void confirmCertificate()}>{trusting ? t("store.certificate.trusting") : t("store.certificate.trust")}</Button>
        </div>
      </div></RumahlModal> : null}
    </main>
  </>;
}
