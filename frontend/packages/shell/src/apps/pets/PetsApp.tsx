import { useEffect, useState } from "react";
import { Button, RumahlInput, RumahlSelect, RumahlSwitch } from "@rumahl/ui/components";
import { MobileNavBar } from "../../components/MobileNavBar";
import { useI18n } from "../../i18n";
import type { MessageKey } from "../../i18n/locales/en";
import { useOptionalShell } from "../../shell/ShellContext";
import { useShellPreferences } from "../../preferences/ShellPreferences";
import { showToast } from "../../shell/toasts";
import {
  DEFAULT_PET,
  PET_ACCESSORIES,
  PET_ACCENT_COLORS,
  PET_BODIES,
  PET_BODY_COLORS,
  PET_3D_DEPTHS,
  PET_ENERGIES,
  PET_EYES,
  PET_FUR_COLORS,
  PET_FUR_LENGTHS,
  PET_HAIRS,
  PET_MATERIALS,
  PET_MOTIONS,
  PET_PLACEMENTS,
  PET_RENDERS,
  PET_SIZES,
  PET_TAILS,
  randomPet,
  sanitizePetName,
  type PetConfig
} from "./pet-model";
import { MAX_PETS, createPet } from "./pet-zoo";
import {
  addPet,
  duplicatePet,
  removePet,
  savePet,
  setActivePet,
  updatePet,
  useZoo
} from "./pet-store";
import { PetAvatar } from "./PetAvatar";
import { Pet3D } from "./Pet3D";
import { PetVitals } from "./PetVitals";
import { deleteLife, usePetLife } from "./pet-life";
import "./pets.css";

type OptionLabel = (key: string) => string;

function options(prefix: string, values: readonly string[], label: OptionLabel) {
  return values.map((value) => ({ value, label: label(`${prefix}.${value}`) }));
}

function timeValue(minutes: number): string {
  return `${String(Math.floor(minutes / 60)).padStart(2, "0")}:${String(minutes % 60).padStart(2, "0")}`;
}

function parseTime(value: string): number | null {
  const match = /^(\d{2}):(\d{2})$/.exec(value);
  if (!match) return null;
  const hours = Number(match[1]);
  const minutes = Number(match[2]);
  if (!Number.isInteger(hours) || !Number.isInteger(minutes) || hours > 23 || minutes > 59) return null;
  return hours * 60 + minutes;
}

export function PetsApp() {
  const { t } = useI18n();
  const mobile = useOptionalShell()?.mode === "mobile";
  const { preferences } = useShellPreferences();
  const owner = preferences?.ownerId ?? null;
  const zoo = useZoo(owner);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [dirtyDraft, setDirtyDraft] = useState<PetConfig | null>(null);

  const selected = zoo?.pets.find((pet) => pet.id === selectedId) ?? null;
  const draft = dirtyDraft ?? selected?.config ?? DEFAULT_PET;
  const dirty = dirtyDraft !== null;
  const active = zoo?.activeId === selected?.id;
  const { life } = usePetLife(owner, selected?.id ?? null, selected?.config ?? null);

  useEffect(() => {
    if (!zoo) {
      setSelectedId(null);
      return;
    }
    if (!selectedId || !zoo.pets.some((pet) => pet.id === selectedId)) {
      setSelectedId(zoo.activeId);
      setDirtyDraft(null);
    }
  }, [zoo, selectedId]);

  const label = (key: string) => t(key as MessageKey);
  const update = (patch: Partial<PetConfig>) => setDirtyDraft({ ...draft, ...patch });
  const canSave = owner !== null && sanitizePetName(draft.name).length > 0;

  const fail = () => showToast({ title: t("pets.saveFailed"), message: t("pets.saveFailedHelp"), variant: "error" });

  const save = () => {
    if (!owner) return;
    const config = { ...draft, name: sanitizePetName(draft.name) };
    if (selected) {
      if (!updatePet(owner, selected.id, config)) return fail();
    } else if (!savePet(owner, config)) {
      return fail();
    }
    setDirtyDraft(null);
    showToast({ title: t("pets.saved"), message: config.name });
  };

  const add = () => {
    if (!owner) return;
    if (zoo && zoo.pets.length >= MAX_PETS) {
      showToast({ title: t("pets.maxPets"), message: t("pets.zooCount", { count: zoo.pets.length, max: MAX_PETS }), variant: "warning" });
      return;
    }
    const pet = createPet({ takenNames: zoo?.pets.map((entry) => entry.config.name) ?? [] });
    const next = addPet(owner, pet);
    if (!next) return fail();
    setSelectedId(next.activeId);
    setDirtyDraft(null);
    showToast({ title: t("pets.petAdded"), message: next.pets.find((entry) => entry.id === next.activeId)?.config.name ?? pet.config.name });
  };

  const duplicate = () => {
    if (!owner || !selected) return;
    const next = duplicatePet(owner, selected.id);
    if (!next) {
      showToast({ title: t("pets.maxPets"), message: t("pets.zooCount", { count: zoo?.pets.length ?? MAX_PETS, max: MAX_PETS }), variant: "warning" });
      return;
    }
    setSelectedId(next.activeId);
    setDirtyDraft(null);
    showToast({ title: t("pets.petAdded"), message: next.pets.find((entry) => entry.id === next.activeId)?.config.name ?? selected.config.name });
  };

  const remove = () => {
    if (!owner || !selected) return;
    const next = removePet(owner, selected.id);
    deleteLife(owner, selected.id);
    setSelectedId(next?.activeId ?? null);
    setDirtyDraft(null);
    showToast({ title: t("pets.petDeleted"), message: t("pets.petDeletedHelp") });
  };

  const activate = () => {
    if (!owner || !selected) return;
    if (!setActivePet(owner, selected.id)) return fail();
    showToast({ title: t("pets.activated"), message: selected.config.name });
  };

  const shuffle = () => update(randomPet());
  const select = (id: string) => {
    setSelectedId(id);
    setDirtyDraft(null);
  };

  return (
    <section className="pets-app" aria-labelledby="pets-heading">
      {mobile ? <MobileNavBar title={t("pets.title")} /> : null}
      <header className="pets-head">
        <div className="pets-copy">
          <p className="pets-kicker">{t("pets.kicker")}</p>
          <h1 id="pets-heading">{t("pets.heading")}</h1>
          <p className="pets-lead">{t("pets.help")}</p>
        </div>
        <div className="pets-actions">
          <Button size="sm" onClick={add} disabled={!owner || (zoo !== null && zoo.pets.length >= MAX_PETS)}>{t("pets.addPet")}</Button>
          {selected ? <Button size="sm" variant="ghost" onClick={duplicate} disabled={zoo !== null && zoo.pets.length >= MAX_PETS}>{t("pets.duplicatePet")}</Button> : null}
          {selected ? <Button size="sm" variant="ghost" onClick={remove}>{t("pets.deletePet")}</Button> : null}
          <Button size="sm" onClick={shuffle}>{t("pets.shuffle")}</Button>
          <Button variant="primary" onClick={save} disabled={!canSave}>
            {selected ? t("pets.save") : t("pets.create")}
          </Button>
        </div>
      </header>

      <section className="pets-zoo" aria-label={t("pets.zoo")}>
        <div className="pets-zoo__head">
          <h2>{t("pets.zooHeading")}</h2>
          <span>{t("pets.zooCount", { count: zoo?.pets.length ?? 0, max: MAX_PETS })}</span>
        </div>
        <div className="pets-zoo__grid">
          {zoo?.pets.map((pet) => (
            <button
              key={pet.id}
              type="button"
              className={`pets-zoo-card${pet.id === selected?.id ? " is-selected" : ""}${pet.id === zoo.activeId ? " is-active" : ""}`}
              aria-label={t("pets.switchPet", { name: pet.config.name })}
              aria-pressed={pet.id === selected?.id}
              onClick={() => select(pet.id)}
            >
              <PetAvatar pet={pet.config} size="xs" />
              <span className="pets-zoo-card__copy">
                <strong>{pet.config.name}</strong>
                <small>{label(`pets.render.${pet.config.render}`)} · {label(`pets.energy.${pet.config.energy}`)}</small>
              </span>
              {pet.id === zoo.activeId ? <span className="pets-zoo-card__badge">{t("pets.activePet")}</span> : null}
            </button>
          ))}
          {(!zoo || zoo.pets.length < MAX_PETS) && owner ? (
            <button type="button" className="pets-zoo-add" onClick={add}>
              <span aria-hidden="true">＋</span>
              <span>{t("pets.addPet")}</span>
            </button>
          ) : null}
        </div>
      </section>

      <div className="pets-layout">
        <section className="pets-preview" aria-label={t("pets.preview")}>
          <div
            className={`pets-stage pet-body-color-${draft.bodyColor} pet-accent-color-${draft.accentColor} pet-material-${draft.material}`}
          >
            {draft.render === "3d"
              ? <Pet3D pet={draft} size="preview" />
              : <PetAvatar pet={draft} size="preview" />}
          </div>
          <PetVitals life={life} />
          <div className="pets-preview__caption">
            <strong>{draft.name || t("pets.namePlaceholder")}</strong>
            <span>
              {label(`pets.body.${draft.body}`)} · {label(`pets.motion.${draft.motion}`)} · {label(`pets.render.${draft.render}`)}
            </span>
          </div>
          <div className="pets-preview__status">
            <p className="pets-status" role="status" aria-live="polite">
              {!owner
                ? t("pets.noOwner")
                : dirty
                  ? t("pets.unsaved")
                  : selected
                    ? active ? t("pets.active") : t("pets.inZoo")
                    : t("pets.none")}
            </p>
            {selected && !active ? <Button size="sm" onClick={activate}>{t("pets.makeActive")}</Button> : null}
          </div>
        </section>

        <section className="pets-controls" aria-label={t("pets.customise")}>
          <label className="pets-field pets-field--wide">
            <span>{t("pets.name")}</span>
            <RumahlInput
              value={draft.name}
              maxLength={24}
              placeholder={t("pets.namePlaceholder")}
              aria-label={t("pets.name")}
              onChange={(event) => update({ name: event.target.value })}
            />
          </label>

          <div className="pets-field">
            <span>{t("pets.body")}</span>
            <RumahlSelect
              label={t("pets.body")}
              value={draft.body}
              options={options("pets.body", PET_BODIES, label)}
              onChange={(value) => update({ body: value as PetConfig["body"] })}
            />
          </div>
          <div className="pets-field">
            <span>{t("pets.eyes")}</span>
            <RumahlSelect
              label={t("pets.eyes")}
              value={draft.eyes}
              options={options("pets.eyes", PET_EYES, label)}
              onChange={(value) => update({ eyes: value as PetConfig["eyes"] })}
            />
          </div>
          <div className="pets-field">
            <span>{t("pets.accessory")}</span>
            <RumahlSelect
              label={t("pets.accessory")}
              value={draft.accessory}
              options={options("pets.accessory", PET_ACCESSORIES, label)}
              onChange={(value) => update({ accessory: value as PetConfig["accessory"] })}
            />
          </div>
          <div className="pets-field">
            <span>{t("pets.hair")}</span>
            <RumahlSelect
              label={t("pets.hair")}
              value={draft.hair}
              options={options("pets.hair", PET_HAIRS, label)}
              onChange={(value) => update({ hair: value as PetConfig["hair"] })}
            />
          </div>
          <div className="pets-field">
            <span>{t("pets.tail")}</span>
            <RumahlSelect
              label={t("pets.tail")}
              value={draft.tail}
              options={options("pets.tail", PET_TAILS, label)}
              onChange={(value) => update({ tail: value as PetConfig["tail"] })}
            />
          </div>
          <div className="pets-field">
            <span>{t("pets.motion")}</span>
            <RumahlSelect
              label={t("pets.motion")}
              value={draft.motion}
              options={options("pets.motion", PET_MOTIONS, label)}
              onChange={(value) => update({ motion: value as PetConfig["motion"] })}
            />
          </div>
          <div className="pets-field">
            <span>{t("pets.placement")}</span>
            <RumahlSelect
              label={t("pets.placement")}
              value={draft.placement}
              options={options("pets.placement", PET_PLACEMENTS, label)}
              onChange={(value) => update({ placement: value as PetConfig["placement"] })}
            />
          </div>
          <div className="pets-field">
            <span>{t("pets.size")}</span>
            <RumahlSelect
              label={t("pets.size")}
              value={draft.size}
              options={options("pets.size", PET_SIZES, label)}
              onChange={(value) => update({ size: value as PetConfig["size"] })}
            />
          </div>
          <div className="pets-field">
            <span>{t("pets.material")}</span>
            <RumahlSelect
              label={t("pets.material")}
              value={draft.material}
              options={options("pets.material", PET_MATERIALS, label)}
              onChange={(value) => update({ material: value as PetConfig["material"] })}
            />
          </div>
          <div className="pets-field">
            <span>{t("pets.render")}</span>
            <RumahlSelect
              label={t("pets.render")}
              value={draft.render}
              options={options("pets.render", PET_RENDERS, label)}
              onChange={(value) => update({ render: value as PetConfig["render"] })}
            />
          </div>
          {draft.render === "3d" ? (
            <div className="pets-field">
              <span>{t("pets.depth")}</span>
              <RumahlSelect
                label={t("pets.depth")}
                value={draft.depth}
                options={options("pets.depth", PET_3D_DEPTHS, label)}
                onChange={(value) => update({ depth: value as PetConfig["depth"] })}
              />
            </div>
          ) : null}
          {draft.material === "fur" ? (
            <>
              <div className="pets-field">
                <span>{t("pets.furLength")}</span>
                <RumahlSelect
                  label={t("pets.furLength")}
                  value={draft.furLength}
                  options={options("pets.furLength", PET_FUR_LENGTHS, label)}
                  onChange={(value) => update({ furLength: value as PetConfig["furLength"] })}
                />
              </div>
              <div className="pets-field">
                <span>{t("pets.furColor")}</span>
                <RumahlSelect
                  label={t("pets.furColor")}
                  value={draft.furColor}
                  options={options("pets.furColor", PET_FUR_COLORS, label)}
                  onChange={(value) => update({ furColor: value as PetConfig["furColor"] })}
                />
              </div>
            </>
          ) : null}
          <div className="pets-field">
            <span>{t("pets.energy")}</span>
            <RumahlSelect
              label={t("pets.energy")}
              value={draft.energy}
              options={options("pets.energy", PET_ENERGIES, label)}
              onChange={(value) => update({ energy: value as PetConfig["energy"] })}
            />
          </div>

          <div className="pets-field pets-field--wide pets-field--routine">
            <span>
              {t("pets.routine")}
              <small>{t("pets.routineHelp")}</small>
            </span>
            <div className="pets-routine">
              <label>
                <span>{t("pets.sleepStart")}</span>
                <RumahlInput
                  type="time"
                  aria-label={t("pets.sleepStart")}
                  value={timeValue(draft.sleepStart)}
                  onChange={(event) => {
                    const next = parseTime(event.target.value);
                    if (next !== null) update({ sleepStart: next });
                  }}
                />
              </label>
              <label>
                <span>{t("pets.sleepEnd")}</span>
                <RumahlInput
                  type="time"
                  aria-label={t("pets.sleepEnd")}
                  value={timeValue(draft.sleepEnd)}
                  onChange={(event) => {
                    const next = parseTime(event.target.value);
                    if (next !== null) update({ sleepEnd: next });
                  }}
                />
              </label>
            </div>
          </div>

          <div className="pets-field pets-field--wide pets-field--switch">
            <span>
              {t("pets.quietMode")}
              <small>{t("pets.quietModeHelp")}</small>
            </span>
            <RumahlSwitch
              label={t("pets.quietMode")}
              checked={draft.quiet}
              onChange={(event) => update({ quiet: event.target.checked })}
            />
          </div>

          <div className="pets-field pets-field--wide">
            <span>{t("pets.bodyColor")}</span>
            <div className="pets-swatches" role="group" aria-label={t("pets.bodyColor")}>
              {PET_BODY_COLORS.map((color) => (
                <button
                  key={color}
                  type="button"
                  className={`pets-swatch pets-swatch--body pet-body-color-${color}`}
                  aria-label={label(`pets.color.${color}`)}
                  aria-pressed={draft.bodyColor === color}
                  onClick={() => update({ bodyColor: color })}
                />
              ))}
            </div>
          </div>

          <div className="pets-field pets-field--wide">
            <span>{t("pets.accentColor")}</span>
            <div className="pets-swatches" role="group" aria-label={t("pets.accentColor")}>
              {PET_ACCENT_COLORS.map((color) => (
                <button
                  key={color}
                  type="button"
                  className={`pets-swatch pets-swatch--accent pet-accent-color-${color}`}
                  aria-label={label(`pets.color.${color}`)}
                  aria-pressed={draft.accentColor === color}
                  onClick={() => update({ accentColor: color })}
                />
              ))}
            </div>
          </div>
        </section>
      </div>
    </section>
  );
}
