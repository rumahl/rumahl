import type { ElementType, ReactNode } from "react";
import { SettingsIcon } from "../../icons";

export interface AppTileIconProps {
  /** Stable id used to derive the monogram gradient hue. */
  id: string;
  label: string;
  /** Image source; falls back to a monogram glyph. */
  icon?: string;
  kind?: string;
  className?: string;
}

/** App icon: an image, a settings glyph, or a monogram on the theme gradient. */
export function AppTileIcon({ id, label, icon, kind, className = "" }: AppTileIconProps) {
  if (icon) return <img src={icon} alt="" />;
  if (kind === "settings") return <SettingsIcon aria-hidden="true" />;
  const initial = label.trim().slice(0, 1).toLocaleUpperCase() || "?";
  return (
    <span className={`rumahl-app-icon-glyph ${className}`} aria-hidden="true" data-app={id}>
      {initial}
    </span>
  );
}

export interface AppTileProps {
  title: string;
  /** Stable id (defaults to the title). */
  id?: string;
  /** Renders `a`/`button`/router link. */
  as?: ElementType;
  icon?: ReactNode;
  href?: string;
  onClick?: () => void;
  /** System apps have no settings link. */
  system?: boolean;
  launchable?: boolean;
  notLaunchableLabel?: string;
  settingsHref?: string;
  settingsLabel?: string;
  className?: string;
}

/**
 * Launcher/desktop app tile: icon, label and an optional settings shortcut.
 *
 * **Flat by design** — the desktop and launcher are a deliberate flat exception,
 * so app tiles carry no glass/material (unlike the rest of the kit). Keep them
 * flat when composing launcher layouts.
 */
export function AppTile({
  title,
  id = title,
  as,
  icon,
  href,
  onClick,
  system,
  launchable = true,
  notLaunchableLabel,
  settingsHref,
  settingsLabel = "App settings",
  className = ""
}: AppTileProps) {
  const Tag = as ?? (href ? "a" : "button");
  const tile = (
    <Tag
      className={`rumahl-app-tile ${className}`}
      data-app-id={id}
      {...(href ? { href } : {})}
      {...(!href && !as ? { type: "button" } : {})}
      onClick={onClick}
    >
      <span className="rumahl-app-tile__icon" aria-hidden="true">
        {icon ?? <AppTileIcon id={id} label={title} />}
      </span>
      <span>{title}</span>
      {!launchable && notLaunchableLabel ? <small>{notLaunchableLabel}</small> : null}
    </Tag>
  );
  if (system || !settingsHref) return tile;
  return (
    <div className="rumahl-app-root">
      {tile}
      <a className="rumahl-app-tile__settings" href={settingsHref} aria-label={settingsLabel}>
        <SettingsIcon aria-hidden="true" />
      </a>
    </div>
  );
}
