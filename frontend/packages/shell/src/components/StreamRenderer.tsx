import { useEffect, useRef, useState } from "react";
import { useTheme } from "@rumahl/ui";
import type { ShellRequest } from "../snapshot-client";
import { grantStreamSession } from "../stream-client";
import { useI18n } from "../i18n";
import { readShellTheme } from "../apps/shell-theme";
import { applyStreamTheme } from "./stream-theme";

export function StreamRenderer({ id, title, request }: {
  id: string;
  title: string;
  request: ShellRequest | undefined;
}) {
  const { t } = useI18n();
  const { tokens } = useTheme();
  const frameRef = useRef<HTMLIFrameElement>(null);
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

  // Project the OS theme into the streamed app and keep it in sync. The stream
  // is served same-origin, so the shell can reach its document.
  useEffect(() => {
    if (frameUrl) applyStreamTheme(frameRef.current, readShellTheme(tokens as Record<string, string>));
  }, [frameUrl, tokens]);

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
      ref={frameRef}
      sandbox="allow-scripts allow-same-origin allow-downloads allow-pointer-lock"
      src={frameUrl}
      title={title}
      onLoad={() => applyStreamTheme(frameRef.current, readShellTheme(tokens as Record<string, string>))}
    />
  );
}
