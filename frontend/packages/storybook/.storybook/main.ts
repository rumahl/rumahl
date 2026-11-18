import type { StorybookConfig } from "@storybook/react-vite";

/**
 * Storybook for the rumahl UI kit. It lives in its own package so it can be
 * extracted into a separate repository together with `@rumahl/ui`.
 */
const config: StorybookConfig = {
  framework: { name: "@storybook/react-vite", options: {} },
  stories: [
    // Hand-written design documentation (MDX).
    "../stories/**/*.mdx",
    "../stories/**/*.stories.@(ts|tsx)",
    // Component stories live with the components in @rumahl/ui.
    "../../ui/src/components/**/*.stories.@(ts|tsx)"
  ],
  addons: ["@storybook/addon-a11y", "@storybook/addon-docs"],
  // Serve the real OS wallpapers so translucent surfaces have the same backdrop
  // they composite over in the shell.
  staticDirs: [{ from: "../../shell/src/assets", to: "/wallpapers" }],
  // Pin the CSS minifier to esbuild. The default pipeline de-duplicates the
  // `-webkit-backdrop-filter` / `backdrop-filter` pair down to the prefixed one,
  // which silently drops the standard property (no blur in Firefox/Chrome).
  viteFinal: (config) => {
    config.build = { ...config.build, cssMinify: "esbuild" };
    return config;
  }
};

export default config;
