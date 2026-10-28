import type { PropsWithChildren } from "react";

function Icon({ className, children }: PropsWithChildren<{ className?: string | undefined }>) {
  return <svg className={className} viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{children}</svg>;
}

export function FolderIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /></Icon>;
}
export function FolderArtwork({ className }: { className?: string | undefined }) {
  return <svg className={className} viewBox="0 0 270.93333 270.93333" fill="none" aria-hidden="true">
    <path d="m 57.325183,31.871957 c -21.191354,0 -38.251412,17.060573 -38.251412,38.25193 v 92.252253 c 0,21.19135 17.060058,38.25193 38.251412,38.25193 H 215.82248 c 21.19136,0 38.25141,-17.06058 38.25141,-38.25193 V 110.34831 C 253.97499,89.295927 237.20871,72.367947 216.36612,72.284477 h -56.15988 c -12.26852,0 -22.22464,-9.54921 -22.87147,-21.650875 v 0.296623 c 0,-0.12013 -0.001,-0.240079 -0.004,-0.359668 -0.0117,-0.229728 -0.0199,-0.460439 -0.0248,-0.691947 -0.51331,-9.563101 -7.9883,-17.216316 -17.47283,-18.006653 z" fill="currentColor" fillOpacity=".76" />
    <rect x="19.186193" y="72.25396" width="234.99998" height="168.7558" ry="38.136852" fill="currentColor" fillOpacity=".96" />
  </svg>;
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

export function AlertIcon({ className }: { className?: string | undefined }) {
  return <Icon className={className}><path d="M12 4 3 20h18z" /><path d="M12 10v4" /><path d="M12 17h.01" /></Icon>;
}
