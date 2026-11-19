import { useEffect, useRef } from "react";
import { useLocation } from "react-router";
import { RumahlPageHeading } from "@rumahl/ui/components";
import { ArrowLeftIcon } from "../icons";
import { useI18n } from "../i18n";
import type { MessageKey } from "../i18n/locales/en";
import { ShellLink } from "../routing/ShellLink";
import { MobileLargeTitle, MobileNavBar } from "./MobileNavBar";
import { useOptionalShell } from "../shell/ShellContext";

/** Back affordance shown before the title on settings sub-pages. */
export function BackLink({ to }: { to: string }) {
  const { t } = useI18n();
  return <ShellLink className="settings-back" to={to} aria-label={t("settings.back")} title={t("settings.back")}>
    <ArrowLeftIcon />
  </ShellLink>;
}

function parentCrumbs(pathname: string, t: (key: MessageKey) => string): { label: string; to: string }[] {
  if (pathname === "/settings") return [];
  const root = { label: t("settings.start"), to: "/settings" };
  if (pathname.startsWith("/settings/display/")) {
    return [root, { label: t("settings.personalization"), to: "/settings/display" }];
  }
  if (pathname.startsWith("/settings/system/")) {
    return [root, { label: t("settings.system"), to: "/settings/system" }];
  }
  if (pathname.startsWith("/settings/apps/")) {
    return [root, { label: t("settings.apps"), to: "/settings/apps" }];
  }
  if (pathname.startsWith("/settings/admin/users/")) {
    return [root, { label: t("settings.admin.users"), to: "/settings/admin/users" }];
  }
  return [root];
}

/** Settings page header with an optional back button before the title. */
export function SettingsHeading({ title, help, back }: { title: string; help?: string; back?: string }) {
  const { t } = useI18n();
  const { pathname } = useLocation();
  const mobile = useOptionalShell()?.mode === "mobile";
  const crumbs: { label: string; to?: string }[] = [...parentCrumbs(pathname, t), { label: title }];
  const wrapper = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const header = wrapper.current?.querySelector<HTMLElement>(".settings-page-heading");
    const scroller = wrapper.current?.closest<HTMLElement>(".settings-content");
    if (!header) return;
    const update = () => header.toggleAttribute("data-stuck", (scroller?.scrollTop ?? 0) > 1);
    update();
    scroller?.addEventListener("scroll", update, { passive: true });
    return () => scroller?.removeEventListener("scroll", update);
  }, []);
  const root = !back && pathname === "/settings";
  return <div ref={wrapper} className="settings-heading-wrap">
    {mobile ? <MobileNavBar
      title={root ? t("nav.settings") : title}
      back={back ? <BackLink to={back} /> : undefined}
      heading={Boolean(back)}
    /> : null}
    {mobile && !back ? <MobileLargeTitle title={title} subtitle={help} /> : null}
    <RumahlPageHeading
      className="settings-page-heading"
      title={title}
      help={help}
      back={back ? <BackLink to={back} /> : null}
      breadcrumb={crumbs.length > 1 ? <nav className="settings-breadcrumb" aria-label={t("settings.breadcrumb")}>
        {crumbs.map((crumb, index) => <span className="settings-breadcrumb__item" key={`${crumb.label}-${index}`}>
          {index > 0 ? <span className="settings-breadcrumb__separator" aria-hidden="true">›</span> : null}
          {crumb.to ? <ShellLink to={crumb.to}>{crumb.label}</ShellLink> : <span aria-current="page">{crumb.label}</span>}
        </span>)}
      </nav> : null}
    />
  </div>;
}
