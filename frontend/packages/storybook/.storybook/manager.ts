import { addons } from "storybook/manager-api";
import { create } from "storybook/theming";

/*
 * Storybook manager theme in the rumahl/website direction: the same green
 * accent, off-white surfaces and muted text as /workspace/website.
 */
const rumahl = create({
  base: "light",
  brandTitle: "rumahl UI",
  brandUrl: "./",
  brandTarget: "_self",
  colorPrimary: "#28694c",
  colorSecondary: "#3f8a68",
  appBg: "#e8ece7",
  appContentBg: "#f8faf7",
  appPreviewBg: "#f8faf7",
  appBorderColor: "#d6ddd4",
  appBorderRadius: 8,
  fontBase: '"Inter", system-ui, -apple-system, "Segoe UI", Roboto, sans-serif',
  fontCode: 'ui-monospace, "SF Mono", Menlo, monospace',
  textColor: "#18201c",
  textMutedColor: "#657068",
  textInverseColor: "#eef4f0",
  barBg: "#f8faf7",
  barTextColor: "#657068",
  barHoverColor: "#28694c",
  barSelectedColor: "#28694c",
  inputBg: "#ffffff",
  inputBorder: "#d6ddd4",
  inputTextColor: "#18201c",
  inputBorderRadius: 6
});

addons.setConfig({
  theme: rumahl,
  panelPosition: "bottom"
});
