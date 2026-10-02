import type { ShellSnapshotV1 } from "@rumahl/contracts";
import { TOKEN_IDS, tokenVariable } from "@rumahl/ui/tokens";
import { parseWorkspace } from "./preferences/Workspace";
import { applyTuning } from "./preferences/theme-tuning";
import { describeRoute } from "./routing/routes";
import { buildSnapshotTheme, resolveTokens, tuningFromSnapshot } from "./theme";

/**
 * Server-rendered appearance tokens (account theme + device customization) so
 * the first paint already uses the tuned colours, dark mode and transparency.
 */
export function appearanceCss(snapshot: ShellSnapshotV1): string {
  const tokens = resolveTokens(applyTuning(buildSnapshotTheme(snapshot).tokens, tuningFromSnapshot(snapshot)));
  return `:root{${TOKEN_IDS.map((id) => `${tokenVariable(id)}:${tokens[id]}`).join(";")}}`;
}

/**
 * Server-rendered CSS for the restored device workspace. Emitting the window
 * rectangles as a nonce-authorized `<style>` positions windows on the first
 * paint (an inline `style` attribute would be blocked by the shell CSP).
 */
export function windowPositionCss(snapshot: ShellSnapshotV1): string {
  if (!snapshot.workspace) return "";
  let windows;
  try {
    windows = parseWorkspace(JSON.parse(snapshot.workspace)).windows;
  } catch {
    return "";
  }
  const launcher = snapshot.mode === "launcher";
  const rules: string[] = [];
  const seen = new Set<string>();
  for (const saved of windows.slice(0, 32)) {
    const route = describeRoute(saved.location);
    if (route.presentation !== "window" || route.stream || seen.has(route.id)) continue;
    seen.add(route.id);
    const selector = `.shell-window[data-window-id="${route.id}"]`;
    if (launcher) rules.push(`${selector}{left:0;top:0;width:100%;height:100%}`);
    else if (saved.placement === "left") rules.push(`${selector}{left:0;top:0;width:50%;height:100%}`);
    else if (saved.placement === "right") rules.push(`${selector}{left:50%;top:0;width:50%;height:100%}`);
    else if (saved.placement === "maximized") rules.push(`${selector}{left:0;top:0;width:100%;height:100%}`);
    else rules.push(`${selector}{left:${saved.rect.x}px;top:${saved.rect.y}px;width:${saved.rect.width}px;height:${saved.rect.height}px}`);
  }
  return rules.join("");
}
