import { useEffect, useState } from "react";
import type { ExtensionContribution } from "@rumahl/contracts";
import { parseWidgetFrameDescriptor } from "@rumahl/contracts/widget";
import type { ShellRequest } from "../snapshot-client";
import { useI18n } from "../i18n";

type Widget = Extract<ExtensionContribution, { kind: "widget" }>;

export function WidgetFrame({ widget, request, allowDevelopmentLoopback = false, demoFrameOrigin }: {
  widget: Widget;
  request?: ShellRequest | undefined;
  allowDevelopmentLoopback?: boolean | undefined;
  demoFrameOrigin?: string | undefined;
}) {
  const { t } = useI18n();
  const [frameUrl, setFrameUrl] = useState<string | null>(null);
  const [reachable, setReachable] = useState<"idle" | "checking" | "ready" | "unavailable">("idle");
  const [frameLoaded, setFrameLoaded] = useState(false);
  const [unavailable, setUnavailable] = useState(false);

  useEffect(() => {
    if (!request) return;
    const controller = new AbortController();
    async function load() {
      try {
        const response = await request!(
          `/api/v1/shell/widgets/${encodeURIComponent(widget.id)}/frame`,
          {
            cache: "no-store",
            credentials: "same-origin",
            headers: { Accept: "application/json" },
            signal: controller.signal
          }
        );
        if (!response.ok) throw new Error("widget frame unavailable");
        const body = await response.text();
        if (body.length > 4096) throw new Error("widget frame descriptor too large");
        const descriptor = parseWidgetFrameDescriptor(
          JSON.parse(body), widget.id, location.origin, allowDevelopmentLoopback, demoFrameOrigin
        );
        if (!controller.signal.aborted) setFrameUrl(descriptor.frameUrl);
      } catch {
        if (!controller.signal.aborted) setUnavailable(true);
      }
    }
    void load();
    return () => controller.abort();
  }, [request, widget.id, allowDevelopmentLoopback, demoFrameOrigin]);

  // A cross-origin widget that is down would otherwise leave the browser's
  // "refused to connect" page inside the glass. Preflight the frame first and
  // fall back to the shell's own unavailable state.
  useEffect(() => {
    if (!frameUrl) return;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 8000);
    setReachable("checking");
    setFrameLoaded(false);
    fetch(frameUrl, { mode: "no-cors", cache: "no-store", credentials: "omit", signal: controller.signal })
      .then(() => { if (!controller.signal.aborted) setReachable("ready"); })
      .catch(() => { if (!controller.signal.aborted) { setReachable("unavailable"); setUnavailable(true); } })
      .finally(() => clearTimeout(timer));
    return () => { controller.abort(); clearTimeout(timer); };
  }, [frameUrl]);

  // Once the iframe has a source, a watchdog catches a document that never
  // becomes interactive (for example after a network drop).
  useEffect(() => {
    if (!frameUrl || reachable !== "ready" || frameLoaded) return;
    const timer = setTimeout(() => setUnavailable(true), 12_000);
    return () => clearTimeout(timer);
  }, [frameUrl, reachable, frameLoaded]);

  if (unavailable || reachable === "unavailable") return <small role="status">{t("contribution.widgetUnavailable")}</small>;
  if (!frameUrl || reachable !== "ready") return <small role="status">{t("contribution.widgetLoading")}</small>;
  return <IsolatedFrame onLoad={() => setFrameLoaded(true)} onError={() => setUnavailable(true)} title={widget.title} url={frameUrl} />;
}

export function IsolatedFrame({ title, url, onError, onLoad }: {
  title: string;
  url: string;
  onError?: (() => void) | undefined;
  onLoad?: (() => void) | undefined;
}) {
  return (
    <iframe
      className="contribution-card__frame"
      loading="lazy"
      onError={onError}
      onLoad={onLoad}
      referrerPolicy="no-referrer"
      sandbox="allow-scripts"
      src={url}
      title={title}
    />
  );
}
