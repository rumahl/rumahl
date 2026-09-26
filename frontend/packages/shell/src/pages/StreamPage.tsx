import { useParams, useLocation } from "react-router";
import { StreamRenderer } from "../components/StreamRenderer";
import { useShell } from "../shell/ShellContext";
import { useI18n } from "../i18n";
import { UnavailablePage } from "./UnavailablePage";
export function StreamPage() {
  const { sessionId = "" } = useParams();
  const { live } = useShell();
  const { t } = useI18n();
  const location = useLocation();
  const title = new URLSearchParams(location.search).get("title")?.slice(0, 128) ?? t("stream.subtitle");
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(sessionId)) return <UnavailablePage />;
  return <StreamRenderer id={sessionId} request={live?.request} title={title} />;
}
