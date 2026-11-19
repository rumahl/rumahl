import { useId } from "react";
import { useI18n } from "../../i18n";
import { GridViewIcon, ListViewIcon } from "./icons";

/**
 * Segmented grid/list choice. Two native radios make the active view explicit
 * to assistive technology ("Grid view, selected") while the thumb keeps the
 * switch's slide ergonomics. Arrow keys move between the radios natively.
 */
export function ViewSwitch({ view, onView }: { view: "grid" | "list"; onView: (view: "grid" | "list") => void }) {
  const { t } = useI18n();
  const name = useId();
  return <div
    className="files-view-switch"
    role="radiogroup"
    aria-label={t("files.view")}
    title={`${t("files.gridView")} / ${t("files.listView")}`}>
    <input
      id={`${name}-grid`}
      className="files-view-switch__radio"
      type="radio"
      name={name}
      value="grid"
      checked={view === "grid"}
      onChange={() => onView("grid")}
      aria-label={t("files.gridView")}
    />
    <label className="files-view-switch__icon files-view-switch__icon--grid" htmlFor={`${name}-grid`} title={t("files.gridView")}>
      <GridViewIcon />
    </label>
    <input
      id={`${name}-list`}
      className="files-view-switch__radio"
      type="radio"
      name={name}
      value="list"
      checked={view === "list"}
      onChange={() => onView("list")}
      aria-label={t("files.listView")}
    />
    <label className="files-view-switch__icon files-view-switch__icon--list" htmlFor={`${name}-list`} title={t("files.listView")}>
      <ListViewIcon />
    </label>
    <span className="files-view-switch__thumb" aria-hidden="true" />
  </div>;
}
