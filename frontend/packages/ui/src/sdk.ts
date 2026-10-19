import { defaultTokens, type Tokens } from "./tokens";
import { resolveVariants, type VariantSelection } from "./variants";
import {
  themes,
  type AppearanceMode,
  type Theme,
  type ThemeParameter,
} from "./themes";
import { sanitizeSlots, type SlotName, type SlotTemplate } from "./slots";

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
  /** Optional theme stylesheet (applied CSP-safely). */
  css?: string | undefined;
  material?: "solid" | undefined;
  /** Base look: the native `rumahl` UI (default) or a stripped `clean` slate. */
  base?: "rumahl" | "clean" | undefined;
  /**
   * Declared capabilities for code themes. The shell enforces them; using an
   * undeclared capability is rejected by the safety engine.
   */
  capabilities?: readonly string[] | undefined;
  /** Custom code (module source). Only trusted/signed themes are executed. */
  code?: string | undefined;
}

/** Merges partial tokens/variants over the rumahl defaults into a full theme. */
export function defineTheme(input: ThemeInput): Theme {
  if (!input.id.trim() || !input.name.trim())
    throw new Error("theme id and name are required");
  return {
    id: input.id,
    name: input.name,
    tokens: { ...defaultTokens, ...input.tokens },
    variants: resolveVariants(input.variants),
    parameters: input.parameters ?? [],
    modes: input.modes ?? ["light"],
    ...(input.slots ? { slots: sanitizeSlots(input.slots) } : {}),
    ...(input.material ? { material: input.material } : {}),
    ...(input.base ? { base: input.base } : {}),
    ...(input.capabilities?.length ? { capabilities: input.capabilities.slice(0, 32) } : {}),
    ...(input.code ? { code: input.code.slice(0, 500_000) } : {}),
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

export function getTheme(id: string): Theme | undefined {
  return registry.get(id);
}

export function listThemes(): readonly Theme[] {
  return [...registry.values()];
}
