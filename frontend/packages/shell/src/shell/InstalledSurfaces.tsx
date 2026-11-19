import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import type { ExtensionContribution, SurfaceIcon } from "@rumahl/contracts";
import { useShell } from "./ShellContext";
import { useShellApps } from "../apps/useShellApps";
import { useAppCatalog } from "../apps/AppCatalog";
import type { InstalledApp } from "../apps/client";
import { useDisabledApps } from "../preferences/disabled-apps";
import { useSafeMode } from "../preferences/safe-mode";
import { useI18n } from "../i18n";
import { appPath } from "../routing/paths";
import { SurfaceFrame } from "./SurfaceFrame";
import "./installed-surfaces.css";

type Surface = Extract<ExtensionContribution, { kind: "surface" }>;
type Placement = Surface["placement"];
const PLACEMENTS: readonly Placement[] = ["top-left", "top-right", "bottom-left", "bottom-right"];

/**
 * Declarative shell surfaces from installed apps.
 *
 * The contribution is data only: the shell owns the card, icon, placement and
 * click target. App code never runs here; it opens the app's normal isolated
 * window. Safe mode and disabled apps hide their surfaces.
 */
export function InstalledSurfaces() {
  const { snapshot, mode } = useShell();
  const { t } = useI18n();
  const navigate = useNavigate();
  const apps = useShellApps();
  const catalog = useAppCatalog();
  const disabled = useDisabledApps();
  const safeMode = useSafeMode();

  if (safeMode || mode === "mobile") return null;

  const surfaces = snapshot.contributions.filter((item): item is Surface => item.kind === "surface")
    .filter((item) =>
      (item.slot === "both" || item.slot === mode) &&
      !disabled.includes(item.app) &&
      apps.some((app) => app.id === item.app && app.launchable)
    );
  if (surfaces.length === 0) return null;

  return (
    <div className="installed-surfaces" role="region" aria-label={t("surfaces.region")}>
      {PLACEMENTS.map((placement) => {
        const group = surfaces.filter((item) => item.placement === placement);
        if (group.length === 0) return null;
        return (
          <div key={placement} className={`installed-surface-group installed-surface-group--${placement}`}>
            {group.map((surface) => (
              <SurfaceHost
                key={surface.id}
                surface={surface}
                app={catalog.apps.find((candidate) => candidate.id === surface.app)}
                onOpen={() => navigate(appPath(surface.app))}
              />
            ))}
          </div>
        );
      })}
    </div>
  );
}

function SurfaceHost({ surface, app, onOpen }: {
  surface: Surface;
  app: InstalledApp | undefined;
  onOpen: () => void;
}) {
  const { t } = useI18n();
  // Tier B needs both a signed entrypoint and an installed frame target.
  const [fallback, setFallback] = useState(surface.entrypoint === undefined || app === undefined);
  useEffect(() => {
    if (surface.entrypoint !== undefined && app !== undefined) setFallback(false);
  }, [surface.entrypoint, app?.id, app?.installationId, app?.version]);
  const frame = surface.entrypoint !== undefined && app !== undefined && !fallback;

  if (frame) {
    return (
      <SurfaceFrame
        app={app}
        surfaceId={surface.id}
        size={surface.size}
        kind={surface.surface}
        title={surface.title}
        onFallback={() => setFallback(true)}
      />
    );
  }

  return (
    <button
      type="button"
      className={`installed-surface installed-surface--${surface.surface} installed-surface--${surface.size} installed-surface-tone-${surface.tone}`}
      aria-label={t("surfaces.open", { app: surface.title })}
      title={surface.title}
      onClick={onOpen}
    >
      <span className="installed-surface__icon" aria-hidden="true">
        <SurfaceGlyph icon={surface.icon} />
      </span>
      <span className="installed-surface__copy">
        <strong>{surface.title}</strong>
        <small>{t("surfaces.label")}</small>
      </span>
    </button>
  );
}

function SurfaceGlyph({ icon }: { icon: SurfaceIcon }) {
  const shape =
    icon === "star" ? (
      <path d="M12 3.5 14 9.5 20.5 12 14 14.5 12 20.5 10 14.5 3.5 12 10 9.5Z" fill="currentColor" />
    ) : icon === "bolt" ? (
      <path d="M13.5 2.5 5 13h5.5L10 21.5 19 10h-5.5Z" fill="currentColor" />
    ) : icon === "leaf" ? (
      <path d="M20 4C10 4 4 9 4 17c0 1.2.2 2.3.5 3.2C6 12 11 8 18 7c-5 2.4-8.7 6.4-10.5 12.3C17.5 20 20 13 20 4Z" fill="currentColor" />
    ) : icon === "moon" ? (
      <path d="M20 14.5A8.5 8.5 0 1 1 9.5 4 6.6 6.6 0 0 0 20 14.5Z" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
    ) : icon === "heart" ? (
      <path d="M12 20.2S4.8 15.8 4.8 10.4A4.1 4.1 0 0 1 12 7.5a4.1 4.1 0 0 1 7.2 2.9c0 5.4-7.2 9.8-7.2 9.8Z" fill="currentColor" />
    ) : icon === "orbit" ? (
      <g fill="none" stroke="currentColor" strokeWidth="1.8">
        <circle cx="12" cy="12" r="3.2" />
        <ellipse cx="12" cy="12" rx="9" ry="4.2" transform="rotate(-28 12 12)" />
      </g>
    ) : icon === "pulse" ? (
      <path d="M2.5 12h4l2-5 3.5 11 2.5-7 2 1h5" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" />
    ) : (
      <circle cx="12" cy="12" r="5" fill="currentColor" />
    );
  return (
    <svg className="installed-surface__glyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      {shape}
    </svg>
  );
}
