import { defaultTokens, type Tokens } from "./tokens";
import { resolveVariants, type VariantSelection } from "./variants";
import { rumahlTheme, type Theme } from "./themes";

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
}

/** Merges partial tokens/variants over the rumahl defaults into a full theme. */
export function defineTheme(input: ThemeInput): Theme {
  if (!input.id.trim() || !input.name.trim()) throw new Error("theme id and name are required");
  return {
    id: input.id,
    name: input.name,
    tokens: { ...defaultTokens, ...input.tokens },
    variants: resolveVariants(input.variants)
  };
}

const registry = new Map<string, Theme>([[rumahlTheme.id, rumahlTheme]]);

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
