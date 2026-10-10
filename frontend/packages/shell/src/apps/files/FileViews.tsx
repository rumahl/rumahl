import { useI18n } from "../../i18n";
import { FileIcon, FolderIcon } from "./icons";
import type { Item } from "./types";
import type { SelectionEvent } from "./useSelection";

export interface ViewProps {
  items: readonly Item[];
  selected: ReadonlySet<string>;
  renameKey: string | null;
  draggingKey: string | null;
  dragOverKey: string | null;
  onSelect: (item: Item, event: SelectionEvent) => void;
  onOpen: (item: Item) => void;
  onRenameCommit: (item: Item, name: string) => void;
  onRenameCancel: () => void;
  onMenu: (item: Item, x: number, y: number) => void;
  onDragStart: (item: Item) => void;
  onDragEnd: () => void;
  onDragOver: (key: string | null) => void;
  onDropOn: (folder: Item) => void;
}

function RenameInput({ item, onCommit, onCancel }: { item: Item; onCommit: (item: Item, name: string) => void; onCancel: () => void }) {
  return <input className="rumahl-input files-rename" autoFocus defaultValue={item.name}
    onClick={(event) => event.stopPropagation()}
    onBlur={(event) => onCommit(item, event.target.value)}
    onKeyDown={(event) => { if (event.key === "Enter") (event.target as HTMLInputElement).blur(); if (event.key === "Escape") onCancel(); }} />;
}

function rowHandlers(props: ViewProps, item: Item) {
  const canDrop = item.directory && !item.host && props.draggingKey !== null && props.draggingKey !== item.key;
  return {
    draggable: !item.host,
    onClick: (event: { ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }) => props.onSelect(item, event),
    onDoubleClick: () => props.onOpen(item),
    onContextMenu: (event: { preventDefault: () => void; clientX: number; clientY: number }) => { event.preventDefault(); props.onMenu(item, event.clientX, event.clientY); },
    onDragStart: (event: { dataTransfer: DataTransfer | null }) => { if (item.host) return; if (event.dataTransfer) { event.dataTransfer.effectAllowed = "move"; event.dataTransfer.setData("text/plain", item.key); } props.onDragStart(item); },
    onDragEnd: () => props.onDragEnd(),
    onDragOver: (event: { preventDefault: () => void }) => { if (canDrop) { event.preventDefault(); props.onDragOver(item.key); } },
    onDragLeave: () => { if (props.dragOverKey === item.key) props.onDragOver(null); },
    onDrop: (event: { preventDefault: () => void }) => { if (canDrop) { event.preventDefault(); props.onDropOn(item); } },
  };
}

function classes(props: ViewProps, item: Item): string {
  return [
    props.selected.has(item.key) ? "is-selected" : "",
    props.draggingKey === item.key ? "is-dragging" : "",
    props.dragOverKey === item.key ? "is-drop-target" : "",
  ].filter(Boolean).join(" ");
}

export function FileGrid(props: ViewProps) {
  return <ul className="files-grid">{props.items.map((item) => <li key={item.key}
    data-file-key={item.key}
    className={`files-tile ${classes(props, item)}`}
    {...rowHandlers(props, item)}>
    <span className="files-tile__icon" aria-hidden="true">{item.directory ? <FolderIcon className="files-icon-folder" /> : <FileIcon className="files-icon-file" />}</span>
    {props.renameKey === item.key
      ? <RenameInput item={item} onCommit={props.onRenameCommit} onCancel={props.onRenameCancel} />
      : <span className="files-tile__name" title={item.name}>{item.name}</span>}
  </li>)}</ul>;
}

export function FileList(props: ViewProps) {
  const { t } = useI18n();
  return <div className="files-list">
    <div className="files-list__head"><span>{t("files.name")}</span><span>{t("files.size")}</span><span>{t("files.modified")}</span></div>
    <ul>{props.items.map((item) => <li key={item.key}
      data-file-key={item.key}
      className={`files-row ${classes(props, item)}`}
      {...rowHandlers(props, item)}>
      <span className="files-row__name">
        {item.directory ? <FolderIcon className="files-icon-folder" /> : <FileIcon className="files-icon-file" />}
        {props.renameKey === item.key
          ? <RenameInput item={item} onCommit={props.onRenameCommit} onCancel={props.onRenameCancel} />
          : <span title={item.name}>{item.name}</span>}
      </span>
      <span className="files-row__meta">{item.directory ? "—" : `${item.size.toLocaleString()} B`}</span>
      <span className="files-row__meta">{item.host && item.modified ? new Date(item.modified * 1000).toLocaleDateString() : ""}</span>
    </li>)}</ul>
  </div>;
}
