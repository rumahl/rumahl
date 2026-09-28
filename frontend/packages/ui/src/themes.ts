import { defaultTokens, type Tokens } from "./tokens";
import { defaultVariants, type VariantSelection } from "./variants";

/**
 * A resolved theme is plain data: token values plus the variant selection.
 * It can safely cross the wire (validated by the platform) or be authored by
 * developers with `defineTheme`.
 */
export interface Theme {
  id: string;
  name: string;
  tokens: Tokens;
  variants: VariantSelection;
}

/** The rumahl brand default: green accent, glass materials, dock + springboard. */
export const rumahlTheme: Theme = {
  id: "com.rumahl.default",
  name: "rumahl",
  tokens: defaultTokens,
  variants: defaultVariants
};

export const defaultTheme = rumahlTheme;
