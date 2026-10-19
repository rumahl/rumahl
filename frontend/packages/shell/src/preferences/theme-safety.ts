import { contrastRatio } from "@rumahl/ui/color";
import { defineTheme, type ThemeInput } from "@rumahl/ui/sdk";
import type { Tokens } from "@rumahl/ui/tokens";

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
    super(report.issues[0]?.message ?? "Theme failed the safety checks");
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

function checkContrast(
  issues: ThemeIssue[],
  ink: unknown,
  background: unknown,
  min: number,
  label: string
): void {
  if (!isHex(ink) || !isHex(background)) return; // not verifiable (e.g. color-mix) — skip
  const { ratio } = contrastRatio(ink, background);
  if (ratio < min) {
    issues.push(
      issue(
        "error",
        "contrast",
        `${label}: contrast ${ratio.toFixed(2)} is below ${min}:1.`
      )
    );
  }
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

  const text = tokens["color.text.primary"];
  const muted = tokens["color.text.muted"];
  for (const key of ["color.surface.strong", "color.panel.background", "color.canvas.background"] as const) {
    checkContrast(issues, text, tokens[key], 4.5, `Text on ${key}`);
    if (muted) checkContrast(issues, muted, tokens[key], 3, `Muted text on ${key}`);
  }
  checkContrast(issues, tokens["color.on.accent"], tokens["color.accent"], 3, "Text on the accent");

  const css = typeof (raw as ThemeInput).css === "string" ? (raw as ThemeInput).css : "";
  if (css) {
    for (const match of css.matchAll(/([^{}]+)\{([^}]*)\}/g)) {
      const [, selector, body] = match;
      if (!selector || !body) continue;
      if (DISABLES.test(body) && CRITICAL_SELECTOR.test(selector)) {
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
    issues.push(
      issue(
        "warning",
        "code-theme",
        "This theme contains executable code; only install code themes from authors you trust."
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
