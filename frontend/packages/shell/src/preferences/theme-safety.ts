import { contrastRatio } from "@rumahl/ui/color";
import { composite } from "@rumahl/ui/palette";
import { defineTheme, type ThemeInput } from "@rumahl/ui/sdk";
import { normalizeThemeRuntime } from "@rumahl/ui/themes";
import { readIconPack } from "@rumahl/ui/icon-pack";
import { TOKEN_IDS, type Tokens } from "@rumahl/ui/tokens";

/**
 * Theme safety engine.
 *
 * A theme may change **anything** — but it must never make the OS unusable.
 * Before installing, the theme is validated for the things operability depends
 * on: required colour roles, readable text on every surface, and CSS that would
 * hide or disable a critical shell region. Errors block the install; warnings
 * are surfaced but allowed.
 */
export type IssueLevel = "error" | "warning";

export interface ThemeIssue {
  level: IssueLevel;
  code: string;
  message: string;
}

export interface SafetyReport {
  ok: boolean;
  issues: ThemeIssue[];
}

export class ThemeSafetyError extends Error {
  report: SafetyReport;
  constructor(report: SafetyReport) {
    const firstError = report.issues.find((item) => item.level === "error");
    super(firstError?.message ?? report.issues[0]?.message ?? "Theme failed the safety checks");
    this.name = "ThemeSafetyError";
    this.report = report;
  }
}

const HEX = /^#(?:[0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i;
const isHex = (value: unknown): value is string =>
  typeof value === "string" && HEX.test(value);

/** Selectors that control whether the OS can be used at all. */
const CRITICAL_SELECTOR =
  /(?:^|[\s,>+~(])(?::root|html|body|\.shell|\.shell-window|\.window|\.menubar|\.topbar|\.topbar__island|\.os-dock|\.taskbar)\b/;
const DISABLES = /(display\s*:\s*none|visibility\s*:\s*hidden|pointer-events\s*:\s*none|opacity\s*:\s*0(?:[;\s}]|$))/i;

/** Capabilities a code theme may declare; anything else is rejected at review. */
const KNOWN_CAPABILITIES = new Set([
  "network",
  "storage",
  "clipboard",
  "notifications",
  "navigation"
]);

/** APIs a theme's code must never use, regardless of capabilities. */
const FORBIDDEN_CODE: { pattern: RegExp; label: string }[] = [
  { pattern: /\beval\s*\(/, label: "eval()" },
  { pattern: /\bnew\s+Function\b/, label: "new Function()" },
  { pattern: /document\.write\s*\(/, label: "document.write()" },
  { pattern: /document\.cookie/, label: "document.cookie" },
  { pattern: /createElement\s*\(\s*["']script["']\s*\)/, label: "dynamic <script> injection" },
  { pattern: /\.srcdoc\s*=/, label: "iframe srcdoc injection" }
];

/**
 * Absolute external resources are blocked by the OS CSP (`connect-src 'self'`,
 * `img-src 'self'`, `default-src 'none'`). Themes must bundle assets (data: URI
 * or same-origin), never load them from another origin.
 */
const EXTERNAL_RESOURCE =
  /(?:url\(\s*["']?https?:|@import[^;{}]*https?:|\bsrc\s*=\s*["']https?:|\bfetch\s*\(\s*["'`]https?:|\bWebSocket\s*\(\s*["'`]wss?:)/i;

/** APIs that require an explicitly declared capability. */
const CAPABILITY_CODE: { pattern: RegExp; capability: string; label: string }[] = [
  { pattern: /\bfetch\s*\(|\bXMLHttpRequest\b|\bWebSocket\b|navigator\.sendBeacon/, capability: "network", label: "network access" },
  { pattern: /localStorage|sessionStorage|indexedDB/, capability: "storage", label: "storage access" },
  { pattern: /navigator\.clipboard|execCommand\s*\(\s*["']copy/, capability: "clipboard", label: "clipboard access" },
  { pattern: /\bnew\s+Notification\b|Notification\.requestPermission\b/, capability: "notifications", label: "notifications" },
  { pattern: /window\.open\s*\(|location\.(?:assign|replace)\s*\(|location\.href\s*=/, capability: "navigation", label: "navigation" }
];

function issue(level: IssueLevel, code: string, message: string): ThemeIssue {
  return { level, code, message };
}

function contrastOf(ink: unknown, background: unknown): number | null {
  if (!isHex(ink) || !isHex(background)) return null;
  return contrastRatio(ink, background).ratio;
}

/** Composites an 8-digit hex surface over an opaque canvas for contrast checks. */
function effectiveSurface(surface: unknown, canvas: unknown): string | null {
  if (typeof surface !== "string" || typeof canvas !== "string") return null;
  const alphaHex = /^#(?:[0-9a-f]{6})([0-9a-f]{2})$/i.exec(surface);
  if (!alphaHex) return isHex(canvas) ? surface : null;
  if (!isHex(canvas)) return null;
  const alpha = Number.parseInt(alphaHex[1]!, 16) / 255;
  return composite(canvas, surface.slice(0, 7), alpha);
}

function checkContrast(
  issues: ThemeIssue[],
  ink: unknown,
  background: unknown,
  min: number,
  label: string,
  level: IssueLevel = "error",
  code = "contrast"
): void {
  const ratio = contrastOf(ink, background);
  if (ratio === null || ratio >= min) return;
  issues.push(
    issue(
      level,
      code,
      `${label}: contrast ${ratio.toFixed(2)} is below ${min}:1.`
    )
  );
}

/** Validates untrusted theme data without throwing. */
export function validateTheme(raw: unknown): SafetyReport {
  const issues: ThemeIssue[] = [];
  let tokens: Tokens;
  try {
    tokens = defineTheme(raw as ThemeInput).tokens;
  } catch {
    return { ok: false, issues: [issue("error", "invalid", "Theme data is invalid.")] };
  }

  const required: (keyof Tokens)[] = [
    "color.accent",
    "color.on.accent",
    "color.text.primary",
    "color.text.muted",
    "color.surface.strong",
    "color.panel.background",
    "color.canvas.background"
  ];
  for (const key of required) {
    if (!tokens[key]) {
      issues.push(issue("error", "missing-token", `Missing colour role ${key}.`));
    }
  }

  const suppliedTokens = (raw as ThemeInput).tokens;
  if (suppliedTokens) {
    const known = new Set<string>(TOKEN_IDS);
    const custom = Object.keys(suppliedTokens).filter((id) => !known.has(id));
    if (custom.length) {
      issues.push(
        issue(
          "warning",
          "custom-tokens",
          `Theme defines ${custom.length} custom token${custom.length === 1 ? "" : "s"}: ${custom.slice(0, 6).join(", ")}${custom.length > 6 ? "…" : ""}. Components only consume them where documented.`
        )
      );
    }
  }

  const iconReport = readIconPack((raw as { icons?: unknown }).icons);
  for (const message of iconReport.errors) issues.push(issue("error", "unsafe-icons", message));
  for (const message of iconReport.warnings) issues.push(issue("warning", "icons", message));

  const schemaVersion = (raw as { schemaVersion?: unknown }).schemaVersion;
  if (schemaVersion !== undefined && schemaVersion !== 1) {
    issues.push(
      issue(
        "warning",
        "schema-version",
        `Theme schema version ${String(schemaVersion)} is not v1; it was validated with the v1 rules.`
      )
    );
  }

  const declaredBase = (raw as ThemeInput).base;
  if (declaredBase === "rumahl" || declaredBase === "clean") {
    issues.push(
      issue(
        "warning",
        "legacy-base",
        `Legacy base "${declaredBase}" is normalized to "${declaredBase === "clean" ? "ground" : "iris"}"; new packages should declare base explicitly.`
      )
    );
  }

  const text = tokens["color.text.primary"];
  const muted = tokens["color.text.muted"];
  const accent = tokens["color.accent"];
  for (const key of ["color.surface.strong", "color.panel.background", "color.canvas.background"] as const) {
    checkContrast(issues, text, tokens[key], 4.5, `Text on ${key}`);
    if (muted) {
      const ratio = contrastOf(muted, tokens[key]);
      if (ratio !== null && ratio < 3) {
        issues.push(issue("error", "contrast-muted", `Muted text on ${key}: contrast ${ratio.toFixed(2)} is below 3:1.`));
      } else if (ratio !== null && ratio < 4.5) {
        issues.push(issue("warning", "contrast-muted", `Muted text on ${key}: contrast ${ratio.toFixed(2)} does not reach AA 4.5:1.`));
      }
    }
    // Focus rings use the accent; 3:1 is the non-text UI floor.
    if (accent) {
      checkContrast(issues, accent, tokens[key], 3, `Accent/focus on ${key}`, "error", "contrast-ui");
    }
  }

  // Alpha surfaces are common for glass. Check text against the surface as it
  // actually renders over the canvas, not against the uncomposited value.
  const compositedSurface = effectiveSurface(tokens["color.surface"], tokens["color.canvas.background"]);
  if (compositedSurface) {
    checkContrast(issues, text, compositedSurface, 4.5, "Text on the composited surface");
    const mutedRatio = contrastOf(muted, compositedSurface);
    if (mutedRatio !== null && mutedRatio < 3) {
      issues.push(issue("error", "contrast-muted", `Muted text on the composited surface: contrast ${mutedRatio.toFixed(2)} is below 3:1.`));
    } else if (mutedRatio !== null && mutedRatio < 4.5) {
      issues.push(issue("warning", "contrast-muted", `Muted text on the composited surface: contrast ${mutedRatio.toFixed(2)} does not reach AA 4.5:1.`));
    }
  }

  // A theme may keep a lower-contrast brand ink, but its accent button text
  // still has to remain legible. 3:1 is the hard floor; 4.5:1 is AA advice.
  const onAccent = contrastOf(tokens["color.on.accent"], accent);
  if (onAccent !== null && onAccent < 3) {
    issues.push(issue("error", "contrast-on-accent", `Text on the accent: contrast ${onAccent.toFixed(2)} is below 3:1.`));
  } else if (onAccent !== null && onAccent < 4.5) {
    issues.push(issue("warning", "contrast-on-accent", `Text on the accent: contrast ${onAccent.toFixed(2)} does not reach AA 4.5:1.`));
  }

  const css = typeof (raw as ThemeInput).css === "string" ? (raw as ThemeInput).css : "";
  if (css) {
    for (const match of css.matchAll(/([^{}]+)\{([^}]*)\}/g)) {
      const [, selector, body] = match;
      if (!selector || !body) continue;
      // Pseudo-element layers (scrims, overlays, HUD labels) can never disable
      // the chrome they are drawn on, so they may use pointer-events/opacity.
      const disablesChrome = selector
        .split(",")
        .map((part) => part.trim())
        .some((part) => CRITICAL_SELECTOR.test(part) && !/::(?:before|after)/i.test(part));
      if (DISABLES.test(body) && disablesChrome) {
        issues.push(
          issue(
            "error",
            "unsafe-css",
            `CSS hides or disables a critical region (${selector.trim().slice(0, 60)}).`
          )
        );
      }
    }
    if (/position\s*:\s*fixed/i.test(css) && /inset\s*:\s*0/i.test(css)) {
      issues.push(
        issue("warning", "overlay", "CSS paints a full-screen fixed layer; check it does not block input.")
      );
    }
  }

  if (css && EXTERNAL_RESOURCE.test(css)) {
    issues.push(
      issue(
        "error",
        "external-resource",
        "External resources are blocked; bundle the asset (data: URI or same-origin)."
      )
    );
  }

  const input = raw as ThemeInput;
  const capabilities = new Set(
    Array.isArray(input.capabilities)
      ? input.capabilities.filter((item): item is string => typeof item === "string")
      : []
  );
  for (const capability of capabilities) {
    if (!KNOWN_CAPABILITIES.has(capability)) {
      issues.push(issue("warning", "unknown-capability", `Unknown capability "${capability}".`));
    }
  }

  const code = typeof input.code === "string" ? input.code : "";
  if (code) {
    const runtime = normalizeThemeRuntime(input.runtime);
    issues.push(
      issue(
        "warning",
        "code-theme",
        runtime === "shell"
          ? "This theme contains executable code that requires full shell access; only install it from authors you trust."
          : "This theme contains executable code that runs in an isolated sandbox layer."
      )
    );
    for (const { pattern, label } of FORBIDDEN_CODE) {
      if (pattern.test(code)) {
        issues.push(issue("error", "unsafe-code", `Theme code uses ${label}, which is not allowed.`));
      }
    }
    for (const { pattern, capability, label } of CAPABILITY_CODE) {
      if (pattern.test(code) && !capabilities.has(capability)) {
        issues.push(
          issue(
            "error",
            "capability",
            `Theme code uses ${label} but does not declare the "${capability}" capability.`
          )
        );
      }
    }
  }

  if (code && EXTERNAL_RESOURCE.test(code)) {
    issues.push(
      issue(
        "error",
        "external-resource",
        "External resources are blocked; bundle the asset (data: URI or same-origin)."
      )
    );
  }

  return { ok: !issues.some((item) => item.level === "error"), issues };
}
