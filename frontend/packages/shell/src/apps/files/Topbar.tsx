import { SearchIcon } from "../../icons";
import { useI18n } from "../../i18n";
import { Breadcrumb } from "./Breadcrumb";
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  ArrowUpIcon,
  FilePlusIcon,
  FolderIcon,
  GridViewIcon,
  ListViewIcon,
  MoveIcon,
  PlusIcon,
  UploadIcon,
} from "./icons";
import type { Nav } from "./types";

export interface TopbarProps {
  nav: Nav;
  compact: boolean;
  isStore: boolean;
  canBack: boolean;
  canForward: boolean;
  canUp: boolean;
  busy: boolean;
  canPaste: boolean;
  view: "grid" | "list";
  search: string;
  onNavigate: (next: Nav) => void;
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

export function Topbar(props: TopbarProps) {
  const { t } = useI18n();
  return <div className={`files-topbar${props.compact ? " files-topbar--compact" : ""}`}>
    <span className="files-topbar__nav-surface" aria-hidden="true" />
    <div className="files-topbar__brand">
      <span className="files-topbar__mark" aria-hidden="true"><FolderIcon /></span>
      <strong>{t("files.title")}</strong>
    </div>
    {props.compact ? null : <div className="files-topbar__command">
      <div className="files-topbar__group files-topbar__group--nav" role="group" aria-label={t("files.path")}>
        <button className="files-icon-btn" type="button" disabled={!props.canBack} onClick={props.onBack} aria-label={t("files.back")} title={t("files.back")}><ArrowLeftIcon /></button>
        <button className="files-icon-btn" type="button" disabled={!props.canForward} onClick={props.onForward} aria-label={t("files.forward")} title={t("files.forward")}><ArrowRightIcon /></button>
        <button className="files-icon-btn" type="button" disabled={!props.canUp} onClick={props.onUp} aria-label={t("files.up")} title={t("files.up")}><ArrowUpIcon /></button>
      </div>
      <div className="files-topbar__path">
        <FolderIcon className="files-topbar__path-icon" />
        <Breadcrumb nav={props.nav} onNavigate={props.onNavigate} />
      </div>
      <span className="files-topbar__spacer" />
      {props.isStore
        ? <div className="files-topbar__group files-topbar__group--actions">
            <button className="files-icon-btn files-topbar__action" type="button" disabled={props.busy} onClick={props.onNewFolder} aria-label={t("files.newFolder")} title={t("files.newFolder")}><PlusIcon /><span className="files-action-label">{t("files.newFolder")}</span></button>
            <button className="files-icon-btn files-topbar__action" type="button" disabled={props.busy} onClick={props.onNewFile} aria-label={t("files.newFile")} title={t("files.newFile")}><FilePlusIcon /><span className="files-action-label">{t("files.newFile")}</span></button>
            <label className="files-icon-btn files-topbar__action files-upload" title={t("files.upload")}>
              <UploadIcon /><span className="files-action-label">{t("files.upload")}</span>
              <input aria-label={t("files.upload")} disabled={props.busy} type="file" onChange={(event) => { const file = event.target.files?.[0]; event.target.value = ""; if (file) props.onUpload(file); }} />
            </label>
            {props.canPaste ? <button className="files-icon-btn files-topbar__action" type="button" disabled={props.busy} onClick={props.onPaste} aria-label={t("files.moveHere")} title={t("files.moveHere")}><MoveIcon /><span className="files-action-label">{t("files.moveHere")}</span></button> : null}
          </div>
        : <span className="files-readonly files-topbar__readonly">{t("files.readOnly")}</span>}
      <label className="files-topbar__search">
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
    </div>}
  </div>;
}
