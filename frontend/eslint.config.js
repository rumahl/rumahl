import eslint from "@eslint/js";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["**/dist/**", "**/dist-demo/**", "**/coverage/**", "**/glass-engine/**"] },
  eslint.configs.recommended,
  ...tseslint.configs.recommended,
  {
    // Theme code modules are plain browser modules by design.
    files: ["packages/ui/src/themes/**/code.js"],
    languageOptions: {
      globals: {
        window: "readonly",
        document: "readonly",
        AudioContext: "readonly",
        setTimeout: "readonly",
        clearTimeout: "readonly"
      }
    }
  },
  {
    files: ["**/*.{ts,tsx}"],
    rules: {
      "@typescript-eslint/consistent-type-imports": "error",
      "@typescript-eslint/no-explicit-any": "error"
    }
  },
  {
    // Browser acceptance audits use Node in the runner and browser globals
    // inside page.evaluate() callbacks.
    files: ["tests/**/*.mjs"],
    languageOptions: {
      globals: {
        process: "readonly",
        console: "readonly",
        window: "readonly",
        document: "readonly",
        getComputedStyle: "readonly",
        localStorage: "readonly",
        fetch: "readonly",
        Node: "readonly",
        HTMLElement: "readonly"
      }
    }
  }
);
