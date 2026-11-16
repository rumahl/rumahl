import { useRef, type PointerEvent as ReactPointerEvent } from "react";
import { PetAvatar, type PetAvatarSize } from "./PetAvatar";
import type { PetConfig } from "./pet-model";

/**
 * A CSS-3D companion with a real extruded body.
 *
 * The depth is built from opaque silhouette layers (body shapes only), not
 * stacked copies of the face. The front layer is the only one with eyes, hair
 * and accessories, so tilting reveals a solid side instead of ghost faces.
 * No WebGL and no app-supplied markup.
 */
export function Pet3D({
  pet,
  size = "md",
  className = ""
}: {
  pet: PetConfig;
  size?: PetAvatarSize;
  className?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const translucent = pet.material === "glass";
  const depthPreset = pet.depth === "soft"
    ? { layers: 8, from: -13, to: 16, minScale: 0.93 }
    : pet.depth === "deep"
      ? { layers: 16, from: -27, to: 32, minScale: 0.82 }
      : { layers: 12, from: -20, to: 24, minScale: 0.88 };
  const depthLayers = translucent ? Math.min(10, depthPreset.layers) : depthPreset.layers;

  const tilt = (event: ReactPointerEvent<HTMLDivElement>) => {
    const element = ref.current;
    if (!element || (event.pointerType === "mouse" && event.buttons !== 0)) return;
    const rect = element.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;
    const x = Math.max(-1, Math.min(1, ((event.clientX - rect.left) / rect.width) * 2 - 1));
    const y = Math.max(-1, Math.min(1, ((event.clientY - rect.top) / rect.height) * 2 - 1));
    element.style.setProperty("--pet3d-rotate-y", `${(x * 18).toFixed(2)}deg`);
    element.style.setProperty("--pet3d-rotate-x", `${(-y * 13).toFixed(2)}deg`);
    element.style.setProperty("--pet3d-gaze-x", x.toFixed(3));
    element.style.setProperty("--pet3d-gaze-y", y.toFixed(3));
    element.style.setProperty("--pet3d-shadow-x", `${(-x * 5).toFixed(2)}px`);
    element.style.setProperty("--pet3d-shadow-y", `${(-y * 2).toFixed(2)}px`);
  };

  const reset = () => {
    const element = ref.current;
    if (!element) return;
    element.style.setProperty("--pet3d-rotate-y", "0deg");
    element.style.setProperty("--pet3d-rotate-x", "0deg");
    element.style.setProperty("--pet3d-gaze-x", "0");
    element.style.setProperty("--pet3d-gaze-y", "0");
    element.style.setProperty("--pet3d-shadow-x", "0px");
    element.style.setProperty("--pet3d-shadow-y", "0px");
  };

  return (
    <div
      ref={ref}
      className={`pet3d pet3d--${size} pet-body-color-${pet.bodyColor} pet-accent-color-${pet.accentColor}${pet.quiet ? " is-quiet" : ""}${className ? ` ${className}` : ""}`}
      onPointerMove={tilt}
      onPointerLeave={reset}
      onPointerCancel={reset}
    >
      <div className="pet3d__scene">
        <span className="pet3d__aura" aria-hidden="true" />
        <span className="pet3d__floor" aria-hidden="true" />
        <span className="pet3d__ring" aria-hidden="true" />
        <span className="pet3d__rim" aria-hidden="true" />
        {Array.from({ length: depthLayers }, (_, index) => {
          const t = index / Math.max(1, depthLayers - 1);
          const z = depthPreset.from + t * (depthPreset.to - depthPreset.from);
          const minScale = translucent ? Math.max(0.9, depthPreset.minScale) : depthPreset.minScale;
          const scale = minScale + (1 - minScale) * Math.sin((t * Math.PI) / 2);
          return (
            <div
              key={index}
              className="pet3d__layer pet3d__layer--depth"
              aria-hidden="true"
              style={{ transform: `translateZ(${z.toFixed(2)}px) scale(${scale.toFixed(3)})` }}
            >
              <div
                className="pet3d__depth-tint"
                style={{
                  opacity: translucent ? 0.24 : 1,
                  filter: `brightness(${(0.76 + t * 0.22).toFixed(2)}) saturate(${translucent ? 0.85 : 0.98})`
                }}
              >
                <PetAvatar pet={pet} size={size} silhouette />
              </div>
            </div>
          );
        })}
      </div>
      {/* The face is a normal 2D layer above the 3D scene. It rotates with the
          same tilt but can never be hidden by a silhouette depth layer. */}
      <div className="pet3d__front">
        <PetAvatar pet={pet} size={size} />
      </div>
    </div>
  );
}
