import { useI18n } from "../../i18n";
import { ChevronRightIcon } from "./icons";
import { freshHost, freshStore, placeLabelKey, ROOT, type Nav } from "./types";

export function Breadcrumb({ nav, onNavigate }: { nav: Nav; onNavigate: (next: Nav) => void }) {
  const { t } = useI18n();
  const root = nav.place.kind === "store" ? freshStore() : freshHost(nav.place.area);
  const parts = nav.place.kind === "store"
    ? nav.trail.slice(1).map((crumb, position) => ({
        key: crumb.id,
        name: crumb.name,
        next: { place: { kind: "store" } as const, trail: nav.trail.slice(0, position + 2), segments: [] } as Nav,
      }))
    : nav.segments.map((name, position) => ({
        key: `${position}:${name}`,
        name,
        next: { place: nav.place, trail: [ROOT], segments: nav.segments.slice(0, position + 1) } as Nav,
      }));
  return <nav className="files-crumbs" aria-label={t("files.path")}>
    <button type="button" onClick={() => onNavigate(root)}>{t(placeLabelKey(nav.place))}</button>
    {parts.map((part) => <span key={part.key} className="files-crumbs__item">
      <ChevronRightIcon className="files-crumbs__sep" />
      <button type="button" onClick={() => onNavigate(part.next)}>{part.name}</button>
    </span>)}
  </nav>;
}
