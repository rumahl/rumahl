import { SearchIcon } from "../../icons";
import { useI18n } from "../../i18n";
import { ArrowLeftIcon, ArrowRightIcon, ArrowUpIcon, FilePlusIcon, GridViewIcon, ListViewIcon, MoveIcon, PlusIcon, UploadIcon } from "./icons";

export interface ToolbarProps {
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
  onView: (view: "grid" | "list") => void;
  onSearch: (value: string) => void;
  onNewFolder: () => void;
  onNewFile: () => void;
  onUpload: (file: File) => void;
  onPaste: () => void;
}

export function Toolbar(props: ToolbarProps) {
  const { t } = useI18n();
  return <div className="files-toolbar" role="toolbar" aria-label={t("files.actions")}>
    <div className="files-toolbar__group">
      <button className="btn files-icon-btn" type="button" disabled={!props.canBack} onClick={props.onBack} aria-label={t("files.back")} title={t("files.back")}><ArrowLeftIcon /></button>
      <button className="btn files-icon-btn" type="button" disabled={!props.canForward} onClick={props.onForward} aria-label={t("files.forward")} title={t("files.forward")}><ArrowRightIcon /></button>
      <button className="btn files-icon-btn" type="button" disabled={!props.canUp} onClick={props.onUp} aria-label={t("files.up")} title={t("files.up")}><ArrowUpIcon /></button>
    </div>
    {props.isStore
      ? <div className="files-toolbar__group">
          <button className="btn files-icon-btn" type="button" disabled={props.busy} onClick={props.onNewFolder} aria-label={t("files.newFolder")} title={t("files.newFolder")}><PlusIcon /></button>
          <button className="btn files-icon-btn" type="button" disabled={props.busy} onClick={props.onNewFile} aria-label={t("files.newFile")} title={t("files.newFile")}><FilePlusIcon /></button>
          <label className="btn files-icon-btn files-upload" title={t("files.upload")}>
            <UploadIcon />
            <input aria-label={t("files.upload")} disabled={props.busy} type="file" onChange={(event) => { const file = event.target.files?.[0]; event.target.value = ""; if (file) props.onUpload(file); }} />
          </label>
          {props.canPaste ? <button className="btn files-icon-btn" type="button" disabled={props.busy} onClick={props.onPaste} aria-label={t("files.moveHere")} title={t("files.moveHere")}><MoveIcon /></button> : null}
        </div>
      : <span className="files-readonly">{t("files.readOnly")}</span>}
    <span className="files-toolbar__spacer" />
    <label className="files-search">
      <SearchIcon className="files-search__icon" />
      <input aria-label={t("files.search")} value={props.search} onChange={(event) => props.onSearch(event.target.value)} placeholder={t("files.search")} maxLength={255} />
    </label>
    <label className="files-view-switch" title={`${t("files.grid")} / ${t("files.list")}`}>
      <input type="checkbox" role="switch" aria-label={`${t("files.grid")} / ${t("files.list")}`} checked={props.view === "list"} onChange={event => props.onView(event.target.checked ? "list" : "grid")} />
      <span className="files-view-switch__track">
        <span className="files-view-switch__thumb" aria-hidden="true" />
        <span className="files-view-switch__icon files-view-switch__icon--grid"><GridViewIcon /></span>
        <span className="files-view-switch__icon files-view-switch__icon--list"><ListViewIcon /></span>
      </span>
    </label>
  </div>;
}
