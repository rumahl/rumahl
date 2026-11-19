import { useI18n } from "../../i18n";
import type { MessageKey } from "../../i18n/locales/en";
import type { PetLife } from "./pet-life";

/** Shared, read-only needs display for the overlay and the zoo studio. */
export function PetVitals({ life, compact = false }: { life: PetLife | null; compact?: boolean }) {
  const { t } = useI18n();
  if (!life) return null;
  const stats = [
    { id: "energy", value: life.energy, label: t("pets.life.energy") },
    { id: "hunger", value: life.hunger, label: t("pets.life.hunger") },
    { id: "bond", value: life.bond, label: t("pets.life.bond") }
  ] as const;
  return (
    <div className={`pet-vitals${compact ? " pet-vitals--compact" : ""}`} aria-label={t("pets.life.vitals")}>
      <span className="pet-vitals__mood">{t(`pets.life.mood.${life.mood}` as MessageKey)}</span>
      <span className="pet-vitals__bars">
        {stats.map((stat) => (
          <span
            key={stat.id}
            className={`pet-vitals__stat pet-vitals__stat--${stat.id}`}
            title={`${stat.label}: ${Math.round(stat.value)}%`}
            aria-label={`${stat.label}: ${Math.round(stat.value)}%`}
          >
            <span className="pet-vitals__track">
              <span className="pet-vitals__fill" style={{ width: `${stat.value}%` }} />
            </span>
          </span>
        ))}
      </span>
    </div>
  );
}
