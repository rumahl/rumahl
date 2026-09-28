import { createContext, useContext, useEffect, useMemo, type PropsWithChildren } from "react";
import { applyTokens, clearTokens, type Tokens } from "./tokens";
import { defaultTheme, type Theme } from "./themes";
import type { VariantSelection } from "./variants";

export interface ThemeContextValue {
  theme: Theme;
  tokens: Tokens;
  variants: VariantSelection;
}

const ThemeContext = createContext<ThemeContextValue>({
  theme: defaultTheme,
  tokens: defaultTheme.tokens,
  variants: defaultTheme.variants
});

/**
 * Applies a theme to the document root. Token projection happens after mount
 * (CSSOM, CSP-safe) so SSR markup stays deterministic and hydration matches.
 */
export function ThemeProvider({ theme = defaultTheme, children }: PropsWithChildren<{ theme?: Theme | undefined }>) {
  useEffect(() => {
    const root = document.documentElement;
    applyTokens(root, theme.tokens);
    return () => clearTokens(root);
  }, [theme]);
  const value = useMemo<ThemeContextValue>(() => ({ theme, tokens: theme.tokens, variants: theme.variants }), [theme]);
  return <ThemeContext value={value}>{children}</ThemeContext>;
}

export function useTheme(): ThemeContextValue {
  return useContext(ThemeContext);
}
