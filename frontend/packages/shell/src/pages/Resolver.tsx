import { useEffect } from "react";
import { useLocation, useNavigate } from "react-router";
import { UnavailablePage } from "./UnavailablePage";
import { openEmergency } from "../shell/emergency";

/**
 * Resolves rumahl `.rl` documents in the normal OS. `recovery.rl` and
 * `sensors.rl` open the Emergency & Recovery Center; any other `<surface>.rl`
 * routes to that surface. Everything else is "not found".
 */
export function Resolver() {
  const location = useLocation();
  const navigate = useNavigate();
  const match = /^\/([a-z0-9][a-z0-9-]*)\.rl$/i.exec(location.pathname);
  const document = match?.[1]?.toLowerCase();
  useEffect(() => {
    if (!document) return;
    if (document === "recovery" || document === "sensors" || document === "components") {
      openEmergency();
      navigate("/", { replace: true });
      return;
    }
    navigate(`/${document}`, { replace: true });
  }, [document, navigate]);
  if (match) return null;
  return <UnavailablePage />;
}
