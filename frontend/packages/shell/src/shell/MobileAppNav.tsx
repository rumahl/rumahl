import { useLayoutEffect, useRef, useState } from "react";
import { MobileIslandNav, type MobileNavModel } from "@rumahl/ui/components";
import { GLASS_MATERIALS, glassMaterialWithTint } from "@rumahl/ui/glass";
import { WindowStackIcon } from "../icons";
import { useI18n } from "../i18n";
import { useGlassElementHost } from "../glass-engine/useGlassEngine";
import { useThemeTuning } from "../preferences/theme-tuning";

/**
 * One bottom bar for every mobile app: the app's registered destinations plus
 * one system action (Open windows). Home lives inside that surface as the
 * centered bottom action, so the bar has a single system entry.
 *
 * The bar normally uses the CSS Liquid Glass fallback. Phones can opt into the
 * real SVG refraction engine (the same material as the widgets); the SVG
 * backend is forced for this host only, independent of the global renderer.
 */
export function MobileAppNav({ model, onSwitcher }: {
  model: MobileNavModel | null;
  onSwitcher: () => void;
}) {
  const { t } = useI18n();
  const { tuning } = useThemeTuning();
  const hasItems = Boolean(model?.items.length);
  const svgGlass = tuning.mobileNavSvgGlass === true;
  const wrapper = useRef<HTMLDivElement>(null);
  const system = useRef<HTMLButtonElement>(null);
  const [host, setHost] = useState<HTMLElement | null>(null);
  useLayoutEffect(() => {
    setHost(hasItems ? wrapper.current : system.current);
  }, [hasItems]);
  // Keep the bar legible over light app content: the widget tint is too
  // saturated for a chrome shelf, so the CSS blends it into the panel colour.
  const material = glassMaterialWithTint(
    { ...GLASS_MATERIALS.widget, radius: 30, bevel: 16, refraction: 28, chroma: 0.14, blur: 2.2 },
    "var(--mobile-nav-glass-tint, rgba(30, 37, 66, 0.1))"
  );
  useGlassElementHost(host, material, svgGlass, "svg");
  return <div ref={wrapper} className={`mobile-app-nav${hasItems ? " mobile-app-nav--with-items" : ""}`} data-engine-glass={svgGlass ? "true" : undefined}>
    {model && hasItems
      ? <MobileIslandNav
          className="mobile-app-nav__island"
          items={model.items}
          value={model.value}
          onChange={model.onChange}
          label={model.label}
          variant="tabbar"
        />
      : <span className="mobile-app-nav__spacer" aria-hidden="true" />}
    <button ref={system} type="button" className="mobile-app-nav__system" aria-label={t("switcher.title")} title={t("switcher.title")} onClick={onSwitcher} data-engine-glass={svgGlass ? "true" : undefined}>
      <WindowStackIcon aria-hidden="true" /><span>{t("switcher.title")}</span>
    </button>
  </div>;
}
