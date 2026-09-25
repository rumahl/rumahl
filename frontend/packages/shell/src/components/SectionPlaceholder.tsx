import type { ShellSection } from "../shell-state";
import { useI18n } from "../i18n";

export function SectionPlaceholder({ section }: { section: Exclude<ShellSection, "home"> }) {
  const { t } = useI18n();
  const content = {
    eyebrow: t(`section.${section}.eyebrow`),
    title: t(`section.${section}.title`),
    body: t(`section.${section}.body`)
  };
  return (
    <main className="section-placeholder">
      <p className="eyebrow">{content.eyebrow}</p>
      <h1>{content.title}</h1>
      <p>{content.body}</p>
    </main>
  );
}
