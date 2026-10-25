import { useEffect, useRef } from "react";
import { GLASS_MATERIALS, glassColorTint, glassMaterialWithTint } from "@rumahl/ui/glass";
import { useTheme } from "@rumahl/ui";
import { useThemeTuning } from "../preferences/theme-tuning";
import { mountGlassHostHandle, type MountedGlassHost } from "./useGlassEngine";

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
  const { tokens } = useTheme();
  const { tuning } = useThemeTuning();
  const tint = glassColorTint(tuning.glassColor, tuning.glassTint, tokens["color.accent"] ?? "#beff2d");
  const tintRef = useRef(tint);
  tintRef.current = tint;
  const mounted = useRef(new Map<Element, { name: string; handle: MountedGlassHost }>()).current;
  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function" || typeof MutationObserver === "undefined") return;

    const upgrade = (element: Element) => {
      if (!(element instanceof HTMLElement) || mounted.has(element)) return;
      // Drop the CSS fallback so the engine owns the surface (no double layer).
      element.querySelectorAll(".rumahl-glass-frame[data-glass-fallback]").forEach((node) => node.remove());
      const name = element.dataset.glassMaterial ?? "bar";
      const base = (GLASS_MATERIALS as Record<string, object>)[name] ?? GLASS_MATERIALS.bar;
      const material = name === "widget" ? glassMaterialWithTint(base, tintRef.current) : base;
      mounted.set(element, { name, handle: mountGlassHostHandle(element, { ...material }) });
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
          const record = mounted.get(node);
          if (record) {
            record.handle.destroy();
            mounted.delete(node);
          }
          node.querySelectorAll?.(HOST_SELECTOR).forEach((child) => {
            const childRecord = mounted.get(child);
            if (childRecord) {
              childRecord.handle.destroy();
              mounted.delete(child);
            }
          });
        });
      }
    });
    observer.observe(document.body, { childList: true, subtree: true });

    return () => {
      observer.disconnect();
      for (const record of mounted.values()) record.handle.destroy();
      mounted.clear();
    };
  }, [mounted]);
  useEffect(() => {
    for (const record of mounted.values()) {
      if (record.name !== "widget") continue;
      record.handle.update(glassMaterialWithTint(GLASS_MATERIALS.widget, tint));
    }
  }, [mounted, tint]);
}
