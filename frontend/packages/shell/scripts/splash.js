import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "../splash.html"), "utf8");
const style = source.match(/<style[^>]*>([\s\S]*?)<\/style>/)?.[1] ?? "";
const body = source.match(/<body[^>]*>([\s\S]*?)<\/body>/)?.[1] ?? "";
const headMarkup = `<meta name="rumahl-shell" content="1" /><style>${style}</style>`;
const bodyMarkup = body.replaceAll(' nonce="{{nonce}}"', "");

/** SSR already includes the splash and its CSP-authorized controller. */
export const injectSplash = {
  name: "inject-splash",
  /** @param {string} html @param {{ filename: string }} ctx */
  transformIndexHtml(html, ctx) {
    if (ctx.filename.endsWith("demo-widget.html") || ctx.filename.endsWith("404.html")) return html;
    if (/\bid\s*=\s*["']rumahl-splash["']/.test(html)) return html;
    return html
      .replace("</head>", `${headMarkup}</head>`)
      .replace(/<body([^>]*)>/, `<body$1>${bodyMarkup}`);
  }
};
