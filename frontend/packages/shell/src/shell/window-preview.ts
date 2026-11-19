/** A non-interactive copy of already rendered first-party content. Never mount
 * another app instance or inspect a sandboxed frame to build an overview. */
export function cloneWindowPreview(source: HTMLElement): HTMLElement | null {
  if (source.querySelector("iframe, object, embed")) return null;
  const copy = source.cloneNode(true) as HTMLElement;
  copy.querySelectorAll("script, style, link, iframe, object, embed, video, audio, canvas, [hidden]").forEach(node => node.remove());
  for (const node of [copy, ...copy.querySelectorAll("*")]) {
    for (const attribute of [...node.attributes]) {
      if (/^on/i.test(attribute.name) || ["id", "name", "for", "autofocus", "href", "srcdoc", "form", "formaction"].includes(attribute.name)) node.removeAttribute(attribute.name);
    }
    if (node instanceof HTMLInputElement) { node.value = ""; node.removeAttribute("value"); node.disabled = true; }
    if (node instanceof HTMLTextAreaElement) { node.value = ""; node.textContent = ""; node.disabled = true; }
    if (node instanceof HTMLSelectElement) node.disabled = true;
    if (node.hasAttribute("contenteditable")) { node.textContent = ""; node.removeAttribute("contenteditable"); }
  }
  copy.inert = true;
  copy.setAttribute("aria-hidden", "true");
  return copy;
}
