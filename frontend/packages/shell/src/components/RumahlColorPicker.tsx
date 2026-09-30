import { RumahlSelect } from "./RumahlSelect";
import { useEffect, useRef, useState } from "react";

import type {
  CSSProperties,
  PointerEvent as ReactPointerEvent,
  KeyboardEvent,
} from "react";

import "./RumahlColorPicker.css";
import { useControlPopover } from "./useControlPopover";

type RGB = {
  r: number;
  g: number;
  b: number;
};

type HSV = {
  h: number;
  s: number;
  v: number;
};

export interface RumahlColorPickerProps {
  value?: string;
  defaultValue?: string;
  onChange?: (color: string) => void;

  label?: string;
  presets?: ({ value: string; label: string } | string)[] | undefined;
}

const DEFAULT_PRESETS = [
  "#0B84FF",
  "#3478F6",
  "#4D91F7",
  "#5BA5FF",
  "#8B5CF6",
  "#A463F2",
  "#F45B85",
  "#FF5B62",
  "#FF9F0A",

  "#22C7A9",
  "#20BFA5",
  "#27C6E5",
  "#1E2A3B",
  "#354052",
  "#64748B",
  "#9BA9BA",
  "#BEC6CE",
  "#E4E7EA",
];

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function rgbToHex({ r, g, b }: RGB) {
  return (
    "#" +
    [r, g, b]
      .map((value) =>
        clamp(Math.round(value), 0, 255)
          .toString(16)
          .padStart(2, "0"),
      )
      .join("")
  );
}

function hexToRgb(hex: string): RGB | null {
  const match = hex.trim().match(/^#?([0-9a-f]{6})$/i);

  if (!match) return null;

  const value = match[1] ?? "";

  return {
    r: parseInt(value.slice(0, 2), 16),
    g: parseInt(value.slice(2, 4), 16),
    b: parseInt(value.slice(4, 6), 16),
  };
}

function hsvToRgb({ h, s, v }: HSV): RGB {
  const saturation = s / 100;
  const value = v / 100;

  const c = value * saturation;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = value - c;

  let r = 0;
  let g = 0;
  let b = 0;

  if (h < 60) {
    r = c;
    g = x;
  } else if (h < 120) {
    r = x;
    g = c;
  } else if (h < 180) {
    g = c;
    b = x;
  } else if (h < 240) {
    g = x;
    b = c;
  } else if (h < 300) {
    r = x;
    b = c;
  } else {
    r = c;
    b = x;
  }

  return {
    r: Math.round((r + m) * 255),
    g: Math.round((g + m) * 255),
    b: Math.round((b + m) * 255),
  };
}

function rgbToHsv({ r, g, b }: RGB): HSV {
  const nr = r / 255;
  const ng = g / 255;
  const nb = b / 255;

  const max = Math.max(nr, ng, nb);
  const min = Math.min(nr, ng, nb);
  const delta = max - min;

  let h = 0;

  if (delta !== 0) {
    if (max === nr) {
      h = 60 * (((ng - nb) / delta) % 6);
    } else if (max === ng) {
      h = 60 * ((nb - nr) / delta + 2);
    } else {
      h = 60 * ((nr - ng) / delta + 4);
    }
  }

  if (h < 0) {
    h += 360;
  }

  return {
    h,
    s: max === 0 ? 0 : (delta / max) * 100,
    v: max * 100,
  };
}

function normalizeHex(value: string) {
  const rgb = hexToRgb(value);

  if (!rgb) return null;

  return rgbToHex(rgb);
}

export function RumahlColorPicker({
  value,
  defaultValue = "#64748B",
  onChange,
  label = "Persönliche Akzentfarbe",
  presets = DEFAULT_PRESETS,
}: RumahlColorPickerProps) {
  const initial =
    normalizeHex(value ?? defaultValue) ??
    normalizeHex(defaultValue) ??
    "#64748B";

  const [open, setOpen] = useState(false);
  const { triggerRef, popupRef } = useControlPopover(open, setOpen, 288);
  const [format, setFormat] = useState<"hex" | "rgb">("hex");

  const [hsv, setHsv] = useState<HSV>(() => {
    return rgbToHsv(hexToRgb(initial)!);
  });

  const [hexInput, setHexInput] = useState(initial);

  const dragging = useRef(false);
  const saturationRef = useRef<HTMLDivElement>(null);

  const rgb = hsvToRgb(hsv);
  const currentHex = rgbToHex(rgb);

  useEffect(() => {
    if (!value) return;

    const normalized = normalizeHex(value);

    if (!normalized) return;

    const nextRgb = hexToRgb(normalized)!;

    setHsv(rgbToHsv(nextRgb));
    setHexInput(normalized);
  }, [value]);

  function updateColor(next: HSV) {
    const normalized: HSV = {
      h: clamp(next.h, 0, 360),
      s: clamp(next.s, 0, 100),
      v: clamp(next.v, 0, 100),
    };

    setHsv(normalized);

    const hex = rgbToHex(hsvToRgb(normalized));

    setHexInput(hex);
    onChange?.(hex);
  }

  function updateFromRgb(nextRgb: RGB) {
    const normalized: RGB = {
      r: clamp(nextRgb.r, 0, 255),
      g: clamp(nextRgb.g, 0, 255),
      b: clamp(nextRgb.b, 0, 255),
    };

    updateColor(rgbToHsv(normalized));
  }

  function updateSaturationFromPointer(
    event: ReactPointerEvent<HTMLDivElement>,
  ) {
    const element = saturationRef.current;

    if (!element) return;

    const rect = element.getBoundingClientRect();

    const saturation = clamp(
      ((event.clientX - rect.left) / rect.width) * 100,
      0,
      100,
    );

    const brightness = clamp(
      100 - ((event.clientY - rect.top) / rect.height) * 100,
      0,
      100,
    );

    updateColor({
      ...hsv,
      s: saturation,
      v: brightness,
    });
  }

  function handlePointerDown(
    event: ReactPointerEvent<HTMLDivElement>,
  ) {
    dragging.current = true;

    event.currentTarget.setPointerCapture(event.pointerId);

    updateSaturationFromPointer(event);
  }

  function handlePointerMove(
    event: ReactPointerEvent<HTMLDivElement>,
  ) {
    if (!dragging.current) return;

    updateSaturationFromPointer(event);
  }

  function handlePointerUp(
    event: ReactPointerEvent<HTMLDivElement>,
  ) {
    dragging.current = false;

    if (
      event.currentTarget.hasPointerCapture(event.pointerId)
    ) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  function handleSaturationKeyDown(
    event: KeyboardEvent<HTMLDivElement>,
  ) {
    const step = event.shiftKey ? 5 : 1;

    switch (event.key) {
      case "ArrowLeft":
        event.preventDefault();

        updateColor({
          ...hsv,
          s: hsv.s - step,
        });

        break;

      case "ArrowRight":
        event.preventDefault();

        updateColor({
          ...hsv,
          s: hsv.s + step,
        });

        break;

      case "ArrowUp":
        event.preventDefault();

        updateColor({
          ...hsv,
          v: hsv.v + step,
        });

        break;

      case "ArrowDown":
        event.preventDefault();

        updateColor({
          ...hsv,
          v: hsv.v - step,
        });

        break;
    }
  }

  function commitHex() {
    const normalized = normalizeHex(hexInput);

    if (!normalized) {
      setHexInput(currentHex);
      return;
    }

    updateFromRgb(hexToRgb(normalized)!);
  }

  const cssVariables = {
    "--cp-color": currentHex,
    "--cp-hue": hsv.h,
  } as CSSProperties;

  return (
    <div
      className="rumahl-color-picker"
      style={cssVariables}
    >
      <button
        type="button"
        ref={triggerRef}
        className="rumahl-color-picker__trigger"
        aria-label={label}
        aria-expanded={open}
        onClick={() => setOpen((state) => !state)}
      >
        <span
          className="rumahl-color-picker__trigger-swatch"
          aria-hidden
        />

        <span className="rumahl-color-picker__trigger-content">
          <span className="rumahl-color-picker__label">
            {label}
          </span>

          <span className="rumahl-color-picker__value">
            {currentHex}
          </span>
        </span>

        <svg
          className="rumahl-color-picker__chevron"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          aria-hidden
        >
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>

      {open && (
        <div ref={popupRef} popover="auto" className="rumahl-color-picker__popover">
          <div className="rumahl-color-picker__color-area-wrapper">
            <div
              ref={saturationRef}
              className="rumahl-color-picker__color-area"
              role="slider"
              tabIndex={0}
              aria-label="Sättigung und Helligkeit"
              onPointerDown={handlePointerDown}
              onPointerMove={handlePointerMove}
              onPointerUp={handlePointerUp}
              onPointerCancel={() => {
                dragging.current = false;
              }}
              onKeyDown={handleSaturationKeyDown}
            >
              <span
                className="rumahl-color-picker__color-cursor"
                style={{
                  left: `${hsv.s}%`,
                  top: `${100 - hsv.v}%`,
                }}
              />
            </div>

            <span className="rumahl-color-picker__color-value">
              {currentHex}
            </span>
          </div>

          <div className="rumahl-color-picker__body">
            <div className="rumahl-color-picker__hue-wrapper">
              <div className="rumahl-color-picker__hue-track" />

              <input
                className="rumahl-color-picker__hue"
                type="range"
                min={0}
                max={360}
                value={hsv.h}
                aria-label="Farbton"
                onChange={(event) => {
                  updateColor({
                    ...hsv,
                    h: Number(event.target.value),
                  });
                }}
              />
            </div>

            <div className="rumahl-color-picker__format-row">
              <RumahlSelect value={format} label="Farbformat"
                onChange={value => setFormat(value === "rgb" ? "rgb" : "hex")}
                options={[{ value: "hex", label: "HEX" }, { value: "rgb", label: "RGB" }]} />

              {format === "hex" ? (
                <input
                  className="rumahl-color-picker__input rumahl-color-picker__hex"
                  aria-label={`${label} value`}
                  value={hexInput}
                  maxLength={7}
                  spellCheck={false}
                  onChange={(event) =>
                    setHexInput(event.target.value)
                  }
                  onBlur={commitHex}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      commitHex();
                      event.currentTarget.blur();
                    }
                  }}
                />
              ) : (
                <div className="rumahl-color-picker__rgb">
                  <RGBInput
                    label="R"
                    value={rgb.r}
                    onChange={(r) =>
                      updateFromRgb({
                        ...rgb,
                        r,
                      })
                    }
                  />

                  <RGBInput
                    label="G"
                    value={rgb.g}
                    onChange={(g) =>
                      updateFromRgb({
                        ...rgb,
                        g,
                      })
                    }
                  />

                  <RGBInput
                    label="B"
                    value={rgb.b}
                    onChange={(b) =>
                      updateFromRgb({
                        ...rgb,
                        b,
                      })
                    }
                  />
                </div>
              )}
            </div>

            <div className="rumahl-color-picker__divider" />

            <div>
              <div className="rumahl-color-picker__preset-header">
                <span>Akzentfarben</span>
              </div>

              <div className="rumahl-color-picker__presets">
                {presets.map((preset) => {
                  const value = typeof preset === "string" ? preset : preset.value;
                  const name = typeof preset === "string" ? preset : preset.label;
                  const normalized =
                    normalizeHex(value) ?? value;

                  const selected =
                    normalized.toUpperCase() ===
                    currentHex.toUpperCase();

                  return (
                    <button
                      key={value}
                      type="button"
                      className="rumahl-color-picker__preset"
                      aria-label={name}
                      title={name}
                      aria-pressed={selected}
                      style={{
                        background: value,
                      }}
                      onClick={() => {
                        const presetRgb = hexToRgb(value);

                        if (presetRgb) {
                          updateFromRgb(presetRgb);
                        }
                      }}
                    />
                  );
                })}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

interface RGBInputProps {
  label: string;
  value: number;
  onChange: (value: number) => void;
}

function RGBInput({
  label,
  value,
  onChange,
}: RGBInputProps) {
  return (
    <label className="rumahl-color-picker__rgb-field">
      <span>{label}</span>

      <input
        type="number"
        min={0}
        max={255}
        value={value}
        onChange={(event) =>
          onChange(
            clamp(
              Number(event.target.value),
              0,
              255,
            ),
          )
        }
      />
    </label>
  );
}