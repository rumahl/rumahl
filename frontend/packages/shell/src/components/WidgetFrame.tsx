import { useEffect, useState } from "react";
import type { ExtensionContribution } from "@rumahl/contracts";
import { parseWidgetFrameDescriptor } from "@rumahl/contracts/widget";
import type { ShellRequest } from "../snapshot-client";
import { useI18n } from "../i18n";

type Widget = Extract<ExtensionContribution, { kind: "widget" }>;

export function WidgetFrame({ widget, request, allowDevelopmentLoopback = false }: {
  widget: Widget;
  request?: ShellRequest | undefined;
  allowDevelopmentLoopback?: boolean | undefined;
}) {
  const { t } = useI18n();
  const [frameUrl, setFrameUrl] = useState<string | null>(null);
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
          JSON.parse(body), widget.id, location.origin, allowDevelopmentLoopback
        );
        if (!controller.signal.aborted) setFrameUrl(descriptor.frameUrl);
      } catch {
        if (!controller.signal.aborted) setUnavailable(true);
      }
    }
    void load();
    return () => controller.abort();
  }, [request, widget.id, allowDevelopmentLoopback]);

  if (unavailable) return <small role="status">{t("contribution.widgetUnavailable")}</small>;
  if (!frameUrl) return <small>{t("contribution.widgetLoading")}</small>;
  return <IsolatedFrame onError={() => setUnavailable(true)} title={widget.title} url={frameUrl} />;
}

export function IsolatedFrame({ title, url, onError }: {
  title: string;
  url: string;
  onError?: (() => void) | undefined;
}) {
  return (
    <iframe
      className="contribution-card__frame"
      loading="lazy"
      onError={onError}
      referrerPolicy="no-referrer"
      sandbox="allow-scripts"
      src={url}
      title={title}
    />
  );
}
