import { ShellLink } from "../routing/ShellLink";
import type { ComponentType, SVGProps } from "react";
import { GridIcon, HomeIcon, PulseIcon, SettingsIcon } from "../icons";
import { RumahlMark } from "./RumahlMark";
import { useI18n } from "../i18n";
import type { ShellSection } from "../shell-state";

interface NavigationProps {
  active: ShellSection;
}

export function Navigation({ active }: NavigationProps) {
  const { t } = useI18n();
  const items: readonly {
    icon: ComponentType<SVGProps<SVGSVGElement>>;
    id: ShellSection;
    label: string;
  }[] = [
    { id: "home", label: t("nav.home"), icon: HomeIcon },
    { id: "apps", label: t("nav.apps"), icon: GridIcon },
    { id: "activity", label: t("nav.activity"), icon: PulseIcon },
    { id: "settings", label: t("nav.settings"), icon: SettingsIcon }
  ];

  return (
    <nav aria-label={t("nav.main")} className="navigation">
      <div className="navigation__brand" aria-label="rumahl OS">
        <span aria-hidden="true" className="navigation__mark">
          <RumahlMark />
        </span>
        <span>rumahl</span>
      </div>
      <div className="navigation__items">
        {items.map(({ icon: Icon, id, label }) => (
          <ShellLink
            aria-current={active === id ? "page" : undefined}
            className={active === id ? "is-active" : undefined}
            key={id}
            to={id === "home" ? "/" : `/${id}`}
          >
            <Icon />
            <span>{label}</span>
          </ShellLink>
        ))}
      </div>
      <div className="navigation__footer">
        <span className="navigation__status" />
        <span>{t("nav.status")}</span>
      </div>
    </nav>
  );
}
