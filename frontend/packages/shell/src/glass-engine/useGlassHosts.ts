import { useEffect } from "react";
import { GLASS_MATERIALS } from "@rumahl/ui/glass";
import { mountGlassHost } from "./useGlassEngine";

const HOST_SELECTOR = ".rumahl-glass-host[data-glass-material]";

/**
 * Bridge that upgrades `@rumahl/ui` `GlassSurface` hosts to real refraction
 * glass. It scans for `[data-glass-material]` hosts, removes their CSS fallback
 * frame and mounts the shared engine on them.
 *
 * Real glass only exists in rumahl OS: outside the shell (apps, Storybook, the
 * extracted UI) there is no engine, so `@rumahl/ui/glass.css` renders the same
 * look without refraction.
 */
export function useGlassHosts(): void {
  useEffect(() => {
    if (typeof window === "undefined" || typeof MutationObserver === "undefined") return;
    const mounted = new Map<Element, () => void>();

    const upgrade = (element: Element) => {
      if (!(element instanceof HTMLElement) || mounted.has(element)) return;
      // Drop the CSS fallback so the engine owns the surface (no double layer).
      element.querySelectorAll(".rumahl-glass-frame[data-glass-fallback]").forEach((node) => node.remove());
      const name = element.dataset.glassMaterial ?? "bar";
      const material = (GLASS_MATERIALS as Record<string, object>)[name] ?? GLASS_MATERIALS.bar;
      mounted.set(element, mountGlassHost(element, { ...material }));
    };

    const scan = (root: ParentNode) => {
      if (root instanceof HTMLElement && root.matches(HOST_SELECTOR)) upgrade(root);
      root.querySelectorAll?.(HOST_SELECTOR).forEach(upgrade);
    };

    scan(document.body);
    const observer = new MutationObserver((records) => {
      for (const record of records) {
        record.addedNodes.forEach((node) => {
          if (node instanceof HTMLElement) scan(node);
        });
        record.removedNodes.forEach((node) => {
          if (!(node instanceof HTMLElement)) return;
          const dispose = mounted.get(node);
          if (dispose) {
            dispose();
            mounted.delete(node);
          }
          node.querySelectorAll?.(HOST_SELECTOR).forEach((child) => {
            const childDispose = mounted.get(child);
            if (childDispose) {
              childDispose();
              mounted.delete(child);
            }
          });
        });
      }
    });
    observer.observe(document.body, { childList: true, subtree: true });

    return () => {
      observer.disconnect();
      for (const dispose of mounted.values()) dispose();
      mounted.clear();
    };
  }, []);
}
