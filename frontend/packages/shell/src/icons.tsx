import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;

function IconBase({ children, ...props }: IconProps) {
  return (
    <svg
      aria-hidden="true"
      fill="none"
      height="20"
      viewBox="0 0 24 24"
      width="20"
      {...props}
    >
      {children}
    </svg>
  );
}

export function HomeIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M3.5 10.5 12 3l8.5 7.5v9a1.5 1.5 0 0 1-1.5 1.5H5a1.5 1.5 0 0 1-1.5-1.5z" />
      <path d="M9 21v-7h6v7" />
    </IconBase>
  );
}

export function GridIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <rect height="7" rx="2" width="7" x="3" y="3" />
      <rect height="7" rx="2" width="7" x="14" y="3" />
      <rect height="7" rx="2" width="7" x="3" y="14" />
      <rect height="7" rx="2" width="7" x="14" y="14" />
    </IconBase>
  );
}

export function PulseIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M3 12h4l2.2-6 4.3 12 2.2-6H21" />
    </IconBase>
  );
}

export function SettingsIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1A1.7 1.7 0 0 0 9 4.6 1.7 1.7 0 0 0 10 3v-.2h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z" />
    </IconBase>
  );
}

export function SearchIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <circle cx="11" cy="11" r="6.5" />
      <path d="m16 16 4.5 4.5" />
    </IconBase>
  );
}

export function ArrowIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M5 12h14M14 7l5 5-5 5" />
    </IconBase>
  );
}

export function ArrowLeftIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M19 12H5M10 7l-5 5 5 5" strokeLinecap="round" strokeLinejoin="round" />
    </IconBase>
  );
}

export function ShieldIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M12 3 5 6v5c0 4.6 2.8 8.2 7 10 4.2-1.8 7-5.4 7-10V6z" />
      <path d="m9 12 2 2 4-4" />
    </IconBase>
  );
}

export function DesktopIcon(props: IconProps) {
  return <IconBase {...props}><rect x="3" y="4" width="18" height="13" rx="2" /><path d="M12 17v4M8 21h8" /></IconBase>;
}

export function InfoIcon(props: IconProps) {
  return <IconBase {...props}><circle cx="12" cy="12" r="9" /><path d="M12 11v5M12 7.6v.2" strokeLinecap="round" /></IconBase>;
}

export function DocumentIcon(props: IconProps) {
  return <IconBase {...props}><path d="M6 3h7l5 5v13H6z" /><path d="M13 3v5h5M9 13h6M9 17h6" strokeLinecap="round" /></IconBase>;
}

export function ExternalLinkIcon(props: IconProps) {
  return <IconBase {...props}><path d="M14 4h6v6" /><path d="M20 4 11 13" /><path d="M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4" strokeLinecap="round" strokeLinejoin="round" /></IconBase>;
}

export function TerminalIcon(props: IconProps) {
  return <IconBase {...props}><rect x="3" y="4" width="18" height="16" rx="3" /><path d="m7 9 3 3-3 3M13 15h4" strokeLinecap="round" strokeLinejoin="round" /></IconBase>;
}

/** First-party window controls (rumahl, not the platform's). */
export function WindowMinimizeIcon(props: IconProps) {
  return <IconBase stroke="currentColor" strokeWidth={2} strokeLinecap="round" {...props}><path d="M6 12h12" /></IconBase>;
}

export function WindowMaximizeIcon(props: IconProps) {
  return <IconBase stroke="currentColor" strokeWidth={2} strokeLinejoin="round" {...props}><rect x="6" y="6" width="12" height="12" rx="3" /></IconBase>;
}

export function WindowRestoreIcon(props: IconProps) {
  return (
    <IconBase stroke="currentColor" strokeWidth={2} strokeLinejoin="round" {...props}>
      <rect x="9" y="9" width="9" height="9" rx="2.5" />
      <path d="M6 15V8.5A2.5 2.5 0 0 1 8.5 6H15" strokeLinecap="round" />
    </IconBase>
  );
}

export function WindowCloseIcon(props: IconProps) {
  return <IconBase stroke="currentColor" strokeWidth={2} strokeLinecap="round" {...props}><path d="M7 7l10 10M17 7 7 17" /></IconBase>;
}
