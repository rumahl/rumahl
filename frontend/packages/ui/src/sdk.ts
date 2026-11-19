import { mergeTokens, tokensForBase, type Tokens } from "./tokens";
import { resolveVariants, type VariantSelection } from "./variants";
import {
  normalizeParameter,
  normalizeThemeBase,
  themes,
  type AppearanceMode,
  type Theme,
  type ThemeParameter,
} from "./themes";
import { sanitizeSlots, type SlotName, type SlotTemplate } from "./slots";
import { sanitizeIconPack, type IconPackInput } from "./icon-pack";

/**
 * Developer SDK. Third-party code authors themes as data; the shell keeps
 * ownership of the variant implementations, so a theme can restyle the whole
 * OS without injecting code.
 */
export interface ThemeInput {
  id: string;
  name: string;
  tokens?: Partial<Tokens> | undefined;
  variants?: Partial<VariantSelection> | undefined;
  parameters?: readonly ThemeParameter[] | undefined;
  modes?: readonly AppearanceMode[] | undefined;
  /** Optional per-region markup (validated safe templates). */
  slots?: Partial<Record<SlotName, SlotTemplate>> | undefined;
  /** Optional artwork for the shell's system icons (validated safe shapes). */
  icons?: IconPackInput | undefined;
  /** Optional theme stylesheet (applied CSP-safely). */
  css?: string | undefined;
  material?: "solid" | undefined;
  /** Fixed art direction: device colour/material tuning is ignored. */
  locked?: boolean | undefined;
  /** Disable generated light/dark palettes; the theme owns every scheme token. */
  paletteTuning?: boolean | undefined;
  /** Portable package schema version. Omitted means v1. */
  schemaVersion?: number | undefined;
  /** Base look: Iris (native default) or Ground (open foundation). Legacy
   * `rumahl` and `clean` values are accepted and normalized. */
  base?: "iris" | "ground" | "rumahl" | "clean" | undefined;
  /**
   * Declared capabilities for code themes. The shell enforces them; using an
   * undeclared capability is rejected by the safety engine.
   */
  capabilities?: readonly string[] | undefined;
  /** Custom code (module source). Only trusted/signed themes are executed. */
  code?: string | undefined;
  /** Optional real preview image (data URI or bundled asset). */
  preview?: string | undefined;
}

/** Merges partial tokens/variants over the rumahl defaults into a full theme. */
export function defineTheme(input: ThemeInput): Theme {
  if (!input.id.trim() || !input.name.trim())
    throw new Error("theme id and name are required");
  const base = normalizeThemeBase(input.base) ?? "iris";
  return {
    id: input.id,
    name: input.name,
    tokens: mergeTokens(tokensForBase(base), input.tokens),
    variants: resolveVariants(input.variants),
    parameters: (input.parameters ?? []).map(normalizeParameter),
    modes: input.modes ?? ["light"],
    ...(input.slots ? { slots: sanitizeSlots(input.slots) } : {}),
    ...(input.icons ? { icons: sanitizeIconPack(input.icons) } : {}),
    ...(input.material ? { material: input.material } : {}),
    ...(input.locked ? { locked: true } : {}),
    ...(typeof input.paletteTuning === "boolean" ? { paletteTuning: input.paletteTuning } : {}),
    ...(typeof input.schemaVersion === "number" ? { schemaVersion: input.schemaVersion } : {}),
    ...(input.base ? { base } : {}),
    ...(input.capabilities?.length ? { capabilities: input.capabilities.slice(0, 32) } : {}),
    ...(input.code ? { code: input.code.slice(0, 500_000) } : {}),
    ...(typeof input.preview === "string" && input.preview.length < 4_000_000 ? { preview: input.preview } : {}),
    ...(input.css ? { css: input.css.slice(0, 200_000) } : {}),
  };
}

const registry = new Map<string, Theme>(
  themes.map((theme) => [theme.id, theme]),
);

export function registerTheme(theme: Theme): Theme {
  registry.set(theme.id, theme);
  return theme;
}

/** Drops every registered theme and restores the built-ins (uninstall). */
export function resetThemes(): void {
  registry.clear();
  for (const theme of themes) registry.set(theme.id, theme);
}

export function getTheme(id: string): Theme | undefined {
  return registry.get(id);
}

export function listThemes(): readonly Theme[] {
  return [...registry.values()];
}
