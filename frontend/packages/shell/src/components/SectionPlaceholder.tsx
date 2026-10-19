import { RumahlEmptyState } from "@rumahl/ui/components";
import type { ShellSection } from "../shell-state";
import { useI18n } from "../i18n";

export function SectionPlaceholder({ section }: { section: Exclude<ShellSection, "home"> }) {
  const { t } = useI18n();
  const content = {
    eyebrow: t(`section.${section}.eyebrow`),
    title: t(`section.${section}.title`),
    body: t(`section.${section}.body`)
  };
  return <RumahlEmptyState className="section-placeholder" eyebrow={content.eyebrow} title={content.title}>
    {content.body}
  </RumahlEmptyState>;
}
