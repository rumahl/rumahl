import { SearchIcon } from "../../icons";
import { useI18n } from "../../i18n";
import { Breadcrumb } from "./Breadcrumb";
import { ArrowLeftIcon, ArrowRightIcon, ArrowUpIcon, GridViewIcon, ListViewIcon, MoveIcon, PlusIcon, UploadIcon } from "./icons";
import type { Nav } from "./types";

export interface ToolbarProps {
  nav: Nav;
  isStore: boolean;
  canBack: boolean;
  canForward: boolean;
  canUp: boolean;
  busy: boolean;
  canPaste: boolean;
  view: "grid" | "list";
  search: string;
  onBack: () => void;
  onForward: () => void;
  onUp: () => void;
  onNavigate: (next: Nav) => void;
  onView: (view: "grid" | "list") => void;
  onSearch: (value: string) => void;
  onNewFolder: () => void;
  onUpload: (file: File) => void;
  onPaste: () => void;
}

export function Toolbar(props: ToolbarProps) {
  const { t } = useI18n();
  const icon = (active: boolean) => `files-icon-btn${active ? " is-active" : ""}`;
  return <div className="files-toolbar" role="toolbar" aria-label={t("files.actions")}>
    <div className="files-toolbar__group">
      <button className="files-icon-btn" type="button" disabled={!props.canBack} onClick={props.onBack} aria-label={t("files.back")} title={t("files.back")}><ArrowLeftIcon /></button>
      <button className="files-icon-btn" type="button" disabled={!props.canForward} onClick={props.onForward} aria-label={t("files.forward")} title={t("files.forward")}><ArrowRightIcon /></button>
      <button className="files-icon-btn" type="button" disabled={!props.canUp} onClick={props.onUp} aria-label={t("files.up")} title={t("files.up")}><ArrowUpIcon /></button>
    </div>
    <Breadcrumb nav={props.nav} onNavigate={props.onNavigate} />
    <label className="files-search">
      <SearchIcon className="files-search__icon" />
      <input value={props.search} onChange={(event) => props.onSearch(event.target.value)} placeholder={t("files.search")} maxLength={255} />
    </label>
    <div className="files-toolbar__group">
      <button className={icon(props.view === "grid")} type="button" aria-pressed={props.view === "grid"} onClick={() => props.onView("grid")} aria-label={t("files.grid")} title={t("files.grid")}><GridViewIcon /></button>
      <button className={icon(props.view === "list")} type="button" aria-pressed={props.view === "list"} onClick={() => props.onView("list")} aria-label={t("files.list")} title={t("files.list")}><ListViewIcon /></button>
    </div>
    {props.isStore
      ? <div className="files-toolbar__group">
          <button className="files-icon-btn" type="button" disabled={props.busy} onClick={props.onNewFolder} aria-label={t("files.newFolder")} title={t("files.newFolder")}><PlusIcon /></button>
          <label className="files-icon-btn files-upload" title={t("files.upload")}>
            <UploadIcon />
            <input type="file" onChange={(event) => { const file = event.target.files?.[0]; event.target.value = ""; if (file) props.onUpload(file); }} />
          </label>
          {props.canPaste ? <button className="files-icon-btn" type="button" disabled={props.busy} onClick={props.onPaste} aria-label={t("files.moveHere")} title={t("files.moveHere")}><MoveIcon /></button> : null}
        </div>
      : <span className="files-readonly">{t("files.readOnly")}</span>}
  </div>;
}
