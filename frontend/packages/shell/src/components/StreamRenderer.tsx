import { useEffect, useState } from "react";
import type { ShellRequest } from "../snapshot-client";
import { grantStreamSession } from "../stream-client";
import { useI18n } from "../i18n";

export function StreamRenderer({ id, title, request }: {
  id: string;
  title: string;
  request: ShellRequest | undefined;
}) {
  const { t } = useI18n();
  const [frameUrl, setFrameUrl] = useState<string | null>(null);
  const [unavailable, setUnavailable] = useState(false);

  useEffect(() => {
    let stopped = false;
    if (!request) {
      setUnavailable(true);
      return;
    }
    async function renew() {
      if (!request) return;
      try {
        const path = await grantStreamSession(request, id);
        if (!stopped) {
          setFrameUrl(path);
          setUnavailable(false);
        }
      } catch {
        if (!stopped) setUnavailable(true);
      }
    }
    void renew();
    const timer = setInterval(() => { void renew(); }, 30_000);
    return () => {
      stopped = true;
      clearInterval(timer);
    };
  }, [id, request]);

  if (unavailable) {
    return <div className="stream-status" role="status">{t("stream.unavailable")}</div>;
  }
  if (!frameUrl) {
    return <div className="stream-status" role="status">{t("stream.connecting")}</div>;
  }
  return (
    <iframe
      allow="autoplay"
      className="stream-frame"
      referrerPolicy="no-referrer"
      sandbox="allow-scripts allow-same-origin allow-downloads allow-pointer-lock"
      src={frameUrl}
      title={title}
    />
  );
}
