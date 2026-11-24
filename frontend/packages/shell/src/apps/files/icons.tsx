import type { PropsWithChildren } from "react";
import folderEmpty from "./folder-icons/folder.svg?raw";
import folderSingle from "./folder-icons/folder-single-file.svg?raw";
import folderMultiple from "./folder-icons/folder-multiple-file.svg?raw";
import folderMore from "./folder-icons/folder-more-file.svg?raw";
import desktopArtwork from "./folder-icons/desktop.svg";
import fileArtwork from "./folder-icons/file.svg";
import textFileArtwork from "./folder-icons/text-file.svg";
import { defaultAssociations, extensionOf } from "./fileTypes";

function Icon({ className, children }: PropsWithChildren<{ className?: string | undefined }>) {
  return <svg className={className} viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{children}</svg>;
}

export function FolderIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /></Icon>;
}
/** Folder artwork reflects how many items are inside without exposing file names. */
export function FolderArtwork({ className, count }: { className?: string | undefined; count?: number | null | undefined }) {
  const source = count == null || count === 0 ? folderEmpty : count === 1 ? folderSingle : count <= 3 ? folderMultiple : folderMore;
  return <span className={`folder-artwork${className ? ` ${className}` : ""}`} aria-hidden="true" dangerouslySetInnerHTML={{ __html: source }} />;
}
export function DesktopArtwork({ className }: { className?: string | undefined }) {
  return <img className={className} src={desktopArtwork} alt="" aria-hidden="true" draggable={false} />;
}

export function FileArtwork({ name, className }: { name: string; className?: string | undefined }) {
  const text = defaultAssociations[extensionOf(name)] === "text";
  return <img className={className} src={text ? textFileArtwork : fileArtwork} alt="" aria-hidden="true" draggable={false} />;
}

export function FileIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M6 3h8l4 4v14H6z" /><path d="M14 3v4h4" /></Icon>;
}
export function ChevronRightIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="m9 6 6 6-6 6" /></Icon>;
}
export function ArrowLeftIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="m15 6-6 6 6 6" /></Icon>;
}
export function ArrowRightIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="m9 6 6 6-6 6" /></Icon>;
}
export function ArrowUpIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M12 19V5" /><path d="m6 11 6-6 6 6" /></Icon>;
}
export function UploadIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M12 16V4" /><path d="m7 9 5-5 5 5" /><path d="M5 20h14" /></Icon>;
}
export function PlusIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M12 5v14M5 12h14" /></Icon>;
}
export function FilePlusIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M6 3h8l4 4v14H6z" /><path d="M14 3v4h4" /><path d="M12 11v6M9 14h6" /></Icon>;
}
export function GridViewIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><rect x="4" y="4" width="7" height="7" rx="1" /><rect x="13" y="4" width="7" height="7" rx="1" /><rect x="4" y="13" width="7" height="7" rx="1" /><rect x="13" y="13" width="7" height="7" rx="1" /></Icon>;
}
export function ListViewIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M8 6h12M8 12h12M8 18h12" /><path d="M4 6h.01M4 12h.01M4 18h.01" /></Icon>;
}
export function PencilIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M4 20h4L19 9l-4-4L4 16z" /><path d="m14 5 4 4" /></Icon>;
}
export function TrashIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M4 7h16" /><path d="M9 7V5h6v2" /><path d="M6 7l1 13h10l1-13" /></Icon>;
}
export function MoveIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /><path d="M12 11v4M9 13l3 3 3-3" /></Icon>;
}
export function OpenIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M14 4h6v6" /><path d="M20 4 10 14" /><path d="M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4" /></Icon>;
}
export function DownloadIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M12 4v12" /><path d="m7 11 5 5 5-5" /><path d="M5 20h14" /></Icon>;
}
/** Overflow trigger for the per-item touch/keyboard action menu. */
export function MoreIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M5 12h.01" /><path d="M12 12h.01" /><path d="M19 12h.01" /></Icon>;
}

export function AlertIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M12 4 3 20h18z" /><path d="M12 10v4" /><path d="M12 17h.01" /></Icon>;
}
