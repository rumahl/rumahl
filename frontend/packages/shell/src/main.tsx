import { renderShell, renderStartupFailure } from "./bootstrap";
import { fetchShellSnapshot } from "./snapshot-client";

try {
  renderShell(await fetchShellSnapshot());
} catch {
  renderStartupFailure();
}
