import { useEffect, useRef, useState } from "react";
import type { InstalledApp } from "../apps/client";
import { requestSurfaceFrame, SafeModeError } from "../apps/client";
import { useI18n } from "../i18n";
import { useShell } from "./ShellContext";

export interface SurfaceFrameProps {
  app: InstalledApp;
  surfaceId: string;
  size: "small" | "medium" | "large";
  kind: "companion" | "badge" | "panel";
  title: string;
  onFallback: () => void;
}

/**
 * Tier B: the app draws its own surface in an isolated, script-only frame.
 *
 * The shell resolves a per-surface lease on the app origin, never exposes the
 * app's bridge and never grants same-origin access. If the descriptor cannot be
 * resolved, or safe mode blocks it, the host falls back to the declarative card.
 */
export function SurfaceFrame({ app, surfaceId, size, kind, title, onFallback }: SurfaceFrameProps) {
  const { live } = useShell();
  const { t } = useI18n();
  const [frameUrl, setFrameUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const loaded = useRef(false);
  const onFallbackRef = useRef(onFallback);
  onFallbackRef.current = onFallback;

  useEffect(() => {
    if (!live) return;
    const request = live.request;
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout> | undefined;
    let lease: string | undefined;
    let active = true;
    setFrameUrl(null);
    setFailed(false);
    loaded.current = false;

    async function renew() {
      try {
        const frame = await requestSurfaceFrame(
          request, app, surfaceId, window.location.origin, controller.signal, lease
        );
        if (!active) return;
        lease = frame.lease;
        setFrameUrl(frame.frameUrl);
        timer = setTimeout(() => { void renew(); }, frame.renewAfterSeconds * 1000);
      } catch (failure) {
        if (!active || controller.signal.aborted) return;
        setFrameUrl(null);
        // Safe mode hides the surface entirely; the parent unmounts it on the
        // next recovery-settings poll. Never downgrade a blocked frame to a
        // visible declarative card.
        if (failure instanceof SafeModeError) return;
        setFailed(true);
        onFallbackRef.current();
      }
    }

    void renew();
    return () => {
      active = false;
      controller.abort();
      if (timer !== undefined) clearTimeout(timer);
    };
  }, [live, app.id, app.installationId, app.version, surfaceId]);

  useEffect(() => {
    if (!frameUrl) return;
    loaded.current = false;
    const timer = setTimeout(() => {
      if (!loaded.current) {
        setFrameUrl(null);
        setFailed(true);
        onFallbackRef.current();
      }
    }, 15_000);
    return () => clearTimeout(timer);
  }, [frameUrl]);

  if (failed || !frameUrl) return null;

  return (
    <div
      className={`installed-surface-frame installed-surface-frame--${kind} installed-surface-frame--${size}`}
      data-surface={surfaceId}
    >
      <iframe
        className="installed-surface-frame__iframe"
        src={frameUrl}
        title={t("surfaces.frame", { app: title })}
        sandbox="allow-scripts"
        referrerPolicy="no-referrer"
        allow=""
        onLoad={() => { loaded.current = true; }}
      />
    </div>
  );
}
