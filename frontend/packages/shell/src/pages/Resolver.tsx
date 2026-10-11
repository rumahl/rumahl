import { useEffect } from "react";
import { useLocation, useNavigate } from "react-router";
import { UnavailablePage } from "./UnavailablePage";

/**
 * Resolves rumahl `.rl` documents in the normal OS: opening `activity.rl` (or
 * any `<surface>.rl`) routes to that surface. Everything else is "not found".
 */
export function Resolver() {
  const location = useLocation();
  const navigate = useNavigate();
  const match = /^\/([a-z0-9][a-z0-9-]*)\.rl$/i.exec(location.pathname);
  useEffect(() => {
    if (match) navigate(`/${match[1]}`, { replace: true });
  }, [match, navigate]);
  if (match) return null;
  return <UnavailablePage />;
}
