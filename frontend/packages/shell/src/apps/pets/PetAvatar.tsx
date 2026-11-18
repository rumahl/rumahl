import { useId } from "react";
import type { PetConfig } from "./pet-model";

export type PetAvatarSize = "xs" | "sm" | "md" | "lg" | "preview";

/**
 * The pet itself. Pure SVG with palette-driven CSS variables, so a stored pet
 * can never inject markup or arbitrary styles.
 *
 * Finishes share one body language:
 * - solid: opaque paint with a small highlight
 * - glass: translucent crystal with an inner rim, caustics and refracted bands
 * - plush: soft stitched fabric, muzzle, paws, button eyes and a woven texture
 * - fur: a plush base with a fuzzy coat around the whole silhouette
 *
 * `silhouette` renders only the opaque body shape. The 3D host uses it for
 * extrusion layers instead of stacking complete copies of the face.
 */
export function PetAvatar({
  pet,
  size = "md",
  className = "",
  silhouette = false
}: {
  pet: PetConfig;
  size?: PetAvatarSize;
  className?: string;
  silhouette?: boolean;
}) {
  const glass = !silhouette && pet.material === "glass";
  const fur = !silhouette && pet.material === "fur";
  const plush = !silhouette && (pet.material === "plush" || fur);
  const uid = useId().replaceAll(":", "");
  const bodyFill = glass ? `url(#${uid}-glass)` : "var(--pet-body)";
  const bodyStroke = glass ? "rgb(255 255 255 / 0.72)" : plush ? "rgb(72 54 42 / 0.32)" : "rgb(20 24 30 / 0.14)";
  const bodyWidth = glass ? 1.6 : plush ? 1.5 : 1.4;
  const furColor = pet.furColor === "accent" ? "var(--pet-accent)" : pet.furColor === "cream" ? "#f2e7d4" : "var(--pet-body)";
  const furLengthScale = pet.furLength === "short" ? 0.68 : pet.furLength === "long" ? 1.45 : 1;

  if (silhouette) {
    return (
      <svg
        className={`pet-avatar pet-avatar--${size} pet-material-${pet.material}${className ? ` ${className}` : ""}`}
        viewBox="0 0 120 120"
        aria-hidden="true"
        focusable="false"
      >
        <g fill="var(--pet-body)">
          <Tail tail={pet.tail} plush={false} detail={false} />
          <Arms plush={false} detail={false} />
          <BodyEars body={pet.body} plush={false} detail={false} />
          <MainBody body={pet.body} />
          <Paws plush={false} detail={false} />
        </g>
      </svg>
    );
  }

  return (
    <svg
      className={`pet-avatar pet-avatar--${size} pet-motion-${pet.motion} pet-material-${pet.material}${className ? ` ${className}` : ""}`}
      viewBox="0 0 120 120"
      role="img"
      aria-hidden="true"
      focusable="false"
    >
      <defs>
        <clipPath id={`${uid}-body-clip`}>
          <MainBody body={pet.body} />
        </clipPath>
        {glass ? (
          <>
            <linearGradient id={`${uid}-glass`} x1="0" y1="0" x2="0.25" y2="1">
              <stop offset="0" stopColor="var(--pet-body)" stopOpacity="0.96" />
              <stop offset="0.5" stopColor="var(--pet-body)" stopOpacity="0.78" />
              <stop offset="1" stopColor="var(--pet-body)" stopOpacity="0.48" />
            </linearGradient>
            <filter id={`${uid}-blur`} x="-40%" y="-40%" width="180%" height="180%">
              <feGaussianBlur stdDeviation="7" />
            </filter>
            <filter id={`${uid}-soft`} x="-40%" y="-40%" width="180%" height="180%">
              <feGaussianBlur stdDeviation="2.2" />
            </filter>
            <filter id={`${uid}-glow`} x="-50%" y="-50%" width="200%" height="200%">
              <feGaussianBlur stdDeviation="4" />
            </filter>
            <clipPath id={`${uid}-glass-clip`}>
              <MainBody body={pet.body} />
            </clipPath>
          </>
        ) : null}
        {plush ? (
          <>
            <pattern id={`${uid}-weave`} width="5" height="5" patternUnits="userSpaceOnUse" patternTransform="rotate(18)">
              <path d="M0 0h5M0 0v5" stroke="#fff" strokeOpacity="0.06" strokeWidth="0.7" />
              <circle cx="2.5" cy="2.5" r="0.55" fill="#000" fillOpacity="0.05" />
            </pattern>
            {fur ? (
              <>
                <clipPath id={`${uid}-fur-clip`}>
                  <Arms plush detail={false} />
                  <BodyEars body={pet.body} plush={false} detail={false} />
                  <MainBody body={pet.body} />
                  <Paws plush detail={false} />
                </clipPath>
              </>
            ) : null}
          </>
        ) : null}
      </defs>

      <g className="pet-avatar__motion">
        {pet.accessory === "halo" ? (
          <ellipse
            cx="60"
            cy="12"
            rx="21"
            ry="6"
            fill="none"
            stroke="#ffd36e"
            strokeWidth="3"
            opacity="0.9"
            transform="rotate(-7 60 12)"
          />
        ) : null}

        {glass ? (
          <>
            <ellipse
              cx="60"
              cy="64"
              rx="49"
              ry="47"
              fill="var(--pet-body)"
              opacity="0.42"
              filter={`url(#${uid}-blur)`}
            />
          </>
        ) : null}

        <g className={`pet-avatar__tail pet-avatar__tail--${pet.tail}`} fill="var(--pet-body)"><Tail tail={pet.tail} plush={plush} /></g>

        <g className="pet-avatar__body" fill={bodyFill} stroke={bodyStroke} strokeWidth={bodyWidth}>
          <Arms plush={plush} />
          <BodyEars body={pet.body} plush={plush} />
          <MainBody body={pet.body} />
          <Paws plush={plush} />
        </g>

        {glass ? <GlassEffects uid={uid} body={pet.body} /> : null}

        {plush ? (
          <>
            <g clipPath={`url(#${uid}-body-clip)`}>
              {!fur ? <rect width="120" height="120" fill={`url(#${uid}-weave)`} opacity="0.75" /> : null}
            </g>
            {fur ? (
              <g clipPath={`url(#${uid}-fur-clip)`} opacity="0.95">
                <FurTexture color={furColor} lengthScale={furLengthScale} />
              </g>
            ) : null}
            <g
              className="pet-avatar__seam"
              fill="none"
              stroke="rgb(86 62 47 / 0.28)"
              strokeWidth="1.1"
              strokeDasharray="2.4 4"
              strokeLinecap="round"
              transform="translate(60 62) scale(0.9) translate(-60 -62)"
            >
              <MainBody body={pet.body} />
            </g>
            <path d="M47 20 Q60 16 73 20" fill="none" stroke="rgb(86 62 47 / 0.2)" strokeWidth="1" strokeDasharray="2 3" />
            <g className="pet-avatar__tag">
              <rect x="94" y="72" width="11" height="16" rx="2.5" fill="#f1e6d6" stroke="#b7a38d" strokeWidth="0.8" />
              <path d="M97 77h5M97 80h5M97 83h3" stroke="#b7a38d" strokeWidth="0.8" strokeLinecap="round" />
            </g>
          </>
        ) : null}

        {!glass ? <RoundShading uid={uid} plush={plush} /> : null}

        {plush ? (
          <ellipse cx="45" cy="36" rx="17" ry="9" fill="#fff" opacity="0.16" transform="rotate(-22 45 36)" />
        ) : glass ? null : (
          <ellipse cx="44" cy="39" rx="15" ry="9" fill="#fff" opacity="0.17" />
        )}

        <g className="pet-avatar__hair"><Hair hair={pet.hair} plush={plush} /></g>

        <g className={`pet-avatar__eyes pet-avatar__eyes--${pet.eyes}`}>
          <Eyes style={pet.eyes} glass={glass} />
        </g>

        {pet.body === "bear" || pet.body === "puppy" ? (
          <Muzzle plush={plush} />
        ) : null}

        {!glass && pet.body !== "bear" && pet.body !== "puppy" ? (
          <>
            <ellipse cx="35" cy="72" rx="7" ry="4" fill="var(--pet-accent)" opacity={plush ? 0.36 : 0.5} />
            <ellipse cx="85" cy="72" rx="7" ry="4" fill="var(--pet-accent)" opacity={plush ? 0.36 : 0.5} />
            <path
              d="M55 73 Q60 78 65 73"
              fill="none"
              stroke="#20242c"
              strokeWidth="2.6"
              strokeLinecap="round"
              opacity={plush ? 0.68 : 0.85}
            />
          </>
        ) : null}

        {pet.accessory === "leaf" ? (
          <g>
            <path
              d="M58 15 C58 4 71 0 81 6 C79 17 67 23 58 15 Z"
              fill="#6fca8f"
              stroke="#3f9b66"
              strokeWidth="1.2"
            />
            <path d="M58 15 C55 9 51 5 46 3" fill="none" stroke="#3f9b66" strokeWidth="2" strokeLinecap="round" />
          </g>
        ) : null}

        {pet.accessory === "star" ? (
          <path
            d="M92 10 L95 18 L104 19 L97 25 L99 34 L92 29 L85 34 L87 25 L80 19 L89 18 Z"
            fill="var(--pet-accent)"
            stroke="#fff"
            strokeWidth="1.2"
            strokeLinejoin="round"
          />
        ) : null}

        {pet.accessory === "headphones" ? (
          <g>
            <path
              d="M24 56 C24 28 39 13 60 13 C81 13 96 28 96 56"
              fill="none"
              stroke="#262b33"
              strokeWidth="6"
              strokeLinecap="round"
            />
            <rect x="16" y="48" width="14" height="23" rx="7" fill="var(--pet-accent)" stroke="#262b33" strokeWidth="2" />
            <rect x="90" y="48" width="14" height="23" rx="7" fill="var(--pet-accent)" stroke="#262b33" strokeWidth="2" />
          </g>
        ) : null}

        {pet.accessory === "bow" ? (
          <g transform="translate(82 20)" stroke="rgb(90 55 65 / 0.28)" strokeWidth="1">
            <path d="M0 0 C-12 -8 -18 4 -8 8 C-14 14 -4 18 0 8 Z" fill="var(--pet-accent)" />
            <path d="M0 0 C12 -8 18 4 8 8 C14 14 4 18 0 8 Z" fill="var(--pet-accent)" />
            <circle cx="0" cy="7" r="4.2" fill="var(--pet-accent)" />
            <path d="M-2 9 Q0 13 2 9" fill="none" stroke="rgb(255 255 255 / 0.5)" strokeWidth="0.8" />
          </g>
        ) : null}

        {pet.accessory === "scarf" ? (
          <g>
            <path d="M28 82 Q60 96 92 82" fill="none" stroke="var(--pet-accent)" strokeWidth="9" strokeLinecap="round" />
            <path d="M82 88 L89 108 L79 104 Z" fill="var(--pet-accent)" stroke="rgb(90 55 65 / 0.18)" strokeWidth="1" />
            <path d="M28 82 Q60 96 92 82" fill="none" stroke="#fff" strokeOpacity="0.16" strokeWidth="2" strokeLinecap="round" transform="translate(0 -2)" />
          </g>
        ) : null}

        {pet.accessory === "glasses" ? (
          <g className="pet-avatar__glasses" fill="none" stroke="#2b211b" strokeWidth="2">
            <circle cx="47" cy="59" r="11" />
            <circle cx="73" cy="59" r="11" />
            <path d="M58 59h4M36 57l-8-3M84 57l8-3" strokeLinecap="round" />
            <path d="M42 52 Q47 49 52 52M68 52 Q73 49 78 52" stroke="#fff" strokeOpacity="0.3" strokeWidth="1.2" />
          </g>
        ) : null}

        {pet.accessory === "flower" ? (
          <g className="pet-avatar__flower" transform="translate(33 18)">
            {[0, 60, 120, 180, 240, 300].map((angle) => (
              <ellipse key={angle} cx="0" cy="-8" rx="4.6" ry="7.2" fill="#fff3d6" stroke="#d9b878" strokeWidth="0.8" transform={`rotate(${angle})`} />
            ))}
            <circle cx="0" cy="0" r="4.5" fill="#ffd36e" stroke="#d9a53b" strokeWidth="0.8" />
          </g>
        ) : null}
      </g>
    </svg>
  );
}

function Hair({ hair, plush }: { hair: PetConfig["hair"]; plush: boolean }) {
  if (hair === "none") return null;
  const stroke = plush ? "rgb(86 62 47 / 0.24)" : "rgb(20 24 30 / 0.16)";
  const stitch = plush ? "2 3" : undefined;

  if (hair === "tuft") {
    return (
      <g fill="var(--pet-accent)" stroke={stroke} strokeWidth="1" strokeLinejoin="round">
        <path d="M45 22 C42 8 52 3 56 15 C59 4 69 5 68 17 C73 8 80 12 75 24 C66 19 54 19 45 22 Z" />
        <path d="M50 17 C50 12 53 9 55 15M63 16 C63 10 66 8 67 15" fill="none" strokeDasharray={stitch} />
      </g>
    );
  }

  if (hair === "fringe") {
    return (
      <g fill="var(--pet-accent)" stroke={stroke} strokeWidth="1" strokeLinejoin="round">
        <path d="M29 38 C33 20 47 14 60 14 C73 14 87 20 91 38 C84 32 78 35 72 31 C66 36 59 30 52 34 C45 30 38 35 29 38 Z" />
        <path d="M39 30 C45 25 51 25 57 29M63 29 C69 25 75 25 81 30" fill="none" strokeDasharray={stitch} strokeLinecap="round" />
      </g>
    );
  }

  if (hair === "curls") {
    return (
      <g fill="var(--pet-accent)" stroke={stroke} strokeWidth="1">
        {[
          { cx: 32, cy: 35 },
          { cx: 45, cy: 25 },
          { cx: 60, cy: 20 },
          { cx: 75, cy: 25 },
          { cx: 88, cy: 35 },
          { cx: 25, cy: 47 },
          { cx: 95, cy: 47 },
          { cx: 21, cy: 60 },
          { cx: 99, cy: 60 }
        ].map(({ cx, cy }, index) => (
          <g key={index}>
            <circle cx={cx} cy={cy} r={index < 5 ? 8.6 : 7.6} />
            <path d={`M${cx - 5} ${cy + 2} C${cx - 2} ${cy - 5} ${cx + 5} ${cy - 4} ${cx + 4} ${cy + 1} C${cx + 3} ${cy + 5} ${cx - 2} ${cy + 5} ${cx - 1} ${cy + 1}`} fill="none" strokeDasharray={stitch} strokeWidth="1" />
          </g>
        ))}
      </g>
    );
  }

  if (hair === "fluff") {
    return (
      <g fill="none" strokeLinecap="round">
        {Array.from({ length: 30 }, (_, index) => {
          const angle = (index / 30) * Math.PI * 2;
          const wobble = (index % 3) * 0.05;
          const x = 60 + Math.cos(angle) * (44 + wobble * 12);
          const y = 62 + Math.sin(angle) * (42 + wobble * 10);
          const rotation = (angle * 180) / Math.PI + 90;
          const length = index % 2 === 0 ? 9 : 6.5;
          return (
            <g key={index} transform={`translate(${x} ${y}) rotate(${rotation})`} opacity={0.78 + (index % 4) * 0.05}>
              <path d={`M0 0 C-2 -${length * 0.55} -1 -${length} 3 -${length * 0.8}`} stroke={stroke} strokeWidth="3.6" />
              <path d={`M0 0 C-2 -${length * 0.55} -1 -${length} 3 -${length * 0.8}`} stroke="var(--pet-accent)" strokeWidth="2.3" />
            </g>
          );
        })}
      </g>
    );
  }

  if (hair === "mane") {
    return (
      <g fill="var(--pet-accent)" stroke={stroke} strokeWidth="1">
        {Array.from({ length: 12 }, (_, index) => {
          const angle = (index / 12) * Math.PI * 2;
          return (
            <circle
              key={index}
              cx={60 + Math.cos(angle) * 45}
              cy={62 + Math.sin(angle) * 43}
              r={index % 2 === 0 ? 10.5 : 8.5}
            />
          );
        })}
        <g fill="none" stroke={stroke} strokeWidth="1" strokeDasharray={stitch} opacity="0.8">
          <path d="M30 32 C36 25 43 22 50 20M70 20 C78 22 85 26 90 32" />
          <path d="M18 63 C20 74 25 83 32 90M88 90 C95 83 100 74 102 63" />
        </g>
      </g>
    );
  }

  return (
    <g fill="var(--pet-accent)" stroke={stroke} strokeWidth="1" strokeLinejoin="round">
      <path d="M25 31 C12 44 10 67 17 90 C21 79 22 62 34 42 Z" />
      <path d="M95 31 C108 44 110 67 103 90 C99 79 98 62 86 42 Z" />
      <path d="M20 50 C17 63 18 76 22 86M100 50 C103 63 102 76 98 86" fill="none" strokeDasharray={stitch} strokeLinecap="round" />
      <path d="M35 27 C31 39 31 49 34 57M85 27 C89 39 89 49 86 57" fill="none" strokeDasharray={stitch} strokeLinecap="round" />
    </g>
  );
}

function Tail({ tail, plush, detail = true }: { tail: PetConfig["tail"]; plush: boolean; detail?: boolean }) {
  if (tail === "none") return null;
  const stroke = plush ? "rgb(72 54 42 / 0.3)" : "rgb(20 24 30 / 0.14)";
  if (tail === "stub") {
    return <ellipse cx="97" cy="86" rx="10" ry="8" transform="rotate(-18 97 86)" stroke={stroke} strokeWidth="1.2" />;
  }
  if (tail === "fluffy") {
    return (
      <g stroke={stroke} strokeWidth="1.2">
        <circle cx="96" cy="86" r="12" />
        <circle cx="105" cy="78" r="8" />
        <circle cx="101" cy="95" r="7" />
        {detail ? (
          <g fill="none" stroke={stroke} strokeWidth="0.8" opacity="0.7">
            <path d="M92 82 C95 86 98 88 101 89M98 72 C99 77 101 80 104 82" strokeLinecap="round" />
          </g>
        ) : null}
      </g>
    );
  }
  return (
    <path
      d="M91 86 C112 92 114 65 99 64 C92 64 89 70 91 76"
      fill="none"
      stroke="var(--pet-body)"
      strokeWidth="10"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  );
}

function RoundShading({ uid, plush }: { uid: string; plush: boolean }) {
  return (
    <g clipPath={`url(#${uid}-body-clip)`} pointerEvents="none">
      <ellipse cx="42" cy="32" rx="38" ry="29" fill="#fff" opacity={plush ? 0.27 : 0.16} />
      <ellipse cx="79" cy="96" rx="42" ry="31" fill="#000" opacity={plush ? 0.16 : 0.09} />
      <ellipse cx="60" cy="64" rx="41" ry="41" fill="none" stroke="#fff" strokeWidth="2.2" opacity={plush ? 0.12 : 0.08} />
    </g>
  );
}

function Arms({ plush, detail = true }: { plush: boolean; detail?: boolean }) {
  if (!plush) return null;
  return (
    <g>
      <ellipse cx="21" cy="78" rx="9" ry="15" transform="rotate(-18 21 78)" />
      <ellipse cx="99" cy="78" rx="9" ry="15" transform="rotate(18 99 78)" />
      {detail ? (
        <g fill="none" stroke="rgb(86 62 47 / 0.26)" strokeWidth="0.9" strokeLinecap="round">
          <path d="M18 73v5M21 72v6M24 73v5" />
          <path d="M96 73v5M99 72v6M102 73v5" />
        </g>
      ) : null}
    </g>
  );
}

/**
 * Dense short fur on the body surface, clipped to the silhouette. Strands
 * spread in a sunflower pattern and point outward, so the finish reads like the
 * fuzzy surface of a plush/ChatGPT-style orb rather than a fringe.
 */
function FurTexture({ color, lengthScale }: { color: string; lengthScale: number }) {
  const strands = 124;
  const golden = Math.PI * (3 - Math.sqrt(5));
  return (
    <g className="pet-avatar__fur" fill="none" strokeLinecap="round">
      {Array.from({ length: strands }, (_, index) => {
        const radius = Math.sqrt((index + 0.5) / strands) * 45;
        const angle = index * golden;
        const x = 60 + Math.cos(angle) * radius;
        const y = 64 + Math.sin(angle) * radius * 0.9;
        const rotation = (angle * 180) / Math.PI + 90 + ((index * 37) % 13) - 6;
        const length = (2.4 + (index % 4) * 0.75) * lengthScale;
        const bend = ((index * 17) % 9) / 8 - 0.5;
        const d = `M0 0 q ${bend * length} -${length * 0.6} ${bend * length * 1.2} -${length}`;
        return (
          <g key={index} transform={`translate(${x} ${y}) rotate(${rotation})`} opacity={0.34 + (index % 3) * 0.08}>
            <path d={d} stroke="rgb(65 45 34 / 0.58)" strokeWidth="1.2" />
            <path d={d} stroke={color} strokeWidth="0.7" opacity="0.86" />
            {index % 5 === 0 ? <path d={d} stroke="#fff" strokeOpacity="0.22" strokeWidth="0.55" transform="translate(0.5 -0.4)" /> : null}
          </g>
        );
      })}
    </g>
  );
}
function GlassEffects({ uid, body }: { uid: string; body: PetConfig["body"] }) {
  return (
    <>
      <g clipPath={`url(#${uid}-glass-clip)`}>
        <ellipse cx="60" cy="96" rx="36" ry="18" fill="#101820" opacity="0.1" />
        <path
          d="M18 63 C31 44 49 73 64 55 C78 39 88 54 103 48"
          fill="none"
          stroke="#fff"
          strokeWidth="9"
          strokeLinecap="round"
          opacity="0.12"
          filter={`url(#${uid}-soft)`}
        />
        <path
          d="M25 39 C34 25 46 18 61 16"
          fill="none"
          stroke="#fff"
          strokeWidth="3"
          strokeLinecap="round"
          opacity="0.5"
          filter={`url(#${uid}-soft)`}
        />
        <ellipse cx="41" cy="35" rx="19" ry="10" fill="#fff" opacity="0.3" transform="rotate(-24 41 35)" filter={`url(#${uid}-soft)`} />
        <ellipse cx="79" cy="88" rx="17" ry="6" fill="#fff" opacity="0.22" transform="rotate(-14 79 88)" filter={`url(#${uid}-soft)`} />
        <g fill="#fff">
          <circle cx="35" cy="57" r="1.6" opacity="0.5" />
          <circle cx="81" cy="48" r="1.2" opacity="0.42" />
          <circle cx="52" cy="87" r="1.4" opacity="0.36" />
          <circle cx="90" cy="70" r="1.8" opacity="0.3" />
        </g>
      </g>
      <g
        fill="none"
        stroke="#fff"
        strokeWidth="1.2"
        opacity="0.34"
        transform="translate(60 64) scale(0.9) translate(-60 -64)"
      >
        <MainBody body={body} />
      </g>
      <ellipse cx="60" cy="64" rx="48" ry="46" fill="none" stroke="#fff" strokeWidth="1" opacity="0.12" />
    </>
  );
}

function BodyEars({ body, plush, detail = true }: { body: PetConfig["body"]; plush: boolean; detail?: boolean }) {
  if (body === "cat") {
    return (
      <g>
        <path d="M28 44 L22 13 L49 28 Z" />
        <path d="M92 44 L98 13 L71 28 Z" />
      </g>
    );
  }
  if (body === "bunny") {
    return (
      <g>
        <ellipse cx="43" cy="24" rx="8.5" ry="23" transform="rotate(-10 43 24)" />
        <ellipse cx="77" cy="24" rx="8.5" ry="23" transform="rotate(10 77 24)" />
      </g>
    );
  }
  if (body === "bear") {
    return (
      <g>
        <circle cx="35" cy="31" r="15" />
        <circle cx="85" cy="31" r="15" />
        {detail ? (
          <>
            <circle cx="35" cy="31" r="7.5" fill="var(--pet-accent)" opacity={plush ? 0.35 : 0.25} stroke="none" />
            <circle cx="85" cy="31" r="7.5" fill="var(--pet-accent)" opacity={plush ? 0.35 : 0.25} stroke="none" />
          </>
        ) : null}
      </g>
    );
  }
  if (body === "puppy") {
    return (
      <g>
        <ellipse cx="25" cy="52" rx="13" ry="24" transform="rotate(18 25 52)" />
        <ellipse cx="95" cy="52" rx="13" ry="24" transform="rotate(-18 95 52)" />
      </g>
    );
  }
  return null;
}

function MainBody({ body }: { body: PetConfig["body"] }) {
  if (body === "blob") {
    return <path d="M60 12c25 0 44 18 44 43 0 28-20 51-44 51S16 83 16 55C16 30 35 12 60 12Z" />;
  }
  return <circle cx="60" cy="66" r="40" />;
}

function Paws({ plush, detail = true }: { plush: boolean; detail?: boolean }) {
  if (!plush) {
    return (
      <>
        <ellipse cx="43" cy="102" rx="11" ry="5.2" />
        <ellipse cx="77" cy="102" rx="11" ry="5.2" />
      </>
    );
  }
  return (
    <g>
      <ellipse cx="43" cy="102" rx="11" ry="6" />
      <ellipse cx="77" cy="102" rx="11" ry="6" />
      {detail ? (
        <g fill="none" stroke="rgb(86 62 47 / 0.3)" strokeWidth="1" strokeLinecap="round">
          <path d="M39 100v3M43 99v4M47 100v3" />
          <path d="M73 100v3M77 99v4M81 100v3" />
        </g>
      ) : null}
    </g>
  );
}

function Muzzle({ plush }: { plush: boolean }) {
  return (
    <g>
      <ellipse cx="60" cy="77" rx="19" ry="13.5" fill="#fff" opacity={plush ? 0.24 : 0.16} />
      <ellipse cx="60" cy="72" rx="5.2" ry="3.8" fill="#3a302b" />
      <path d="M60 76v3M56 82 Q60 85 64 82" fill="none" stroke="#3a302b" strokeWidth="2.2" strokeLinecap="round" />
      <ellipse cx="33" cy="71" rx="6" ry="3.5" fill="var(--pet-accent)" opacity="0.32" />
      <ellipse cx="87" cy="71" rx="6" ry="3.5" fill="var(--pet-accent)" opacity="0.32" />
    </g>
  );
}


function Eyes({ style, glass }: { style: PetConfig["eyes"]; glass: boolean }) {
  const stroke = glass ? "rgb(38 64 86 / 0.78)" : "#20242c";
  const fill = glass ? "rgb(38 64 86 / 0.78)" : "#20242c";
  if (style === "button" && !glass) {
    return (
      <g>
        <ellipse cx="47" cy="59" rx="8.2" ry="7.8" fill="#2b211b" opacity="0.07" />
        <ellipse cx="73" cy="59" rx="8.2" ry="7.8" fill="#2b211b" opacity="0.07" />
        <circle cx="47" cy="59" r="6.4" fill="#3b3029" stroke="#6e5544" strokeWidth="1.1" />
        <circle cx="73" cy="59" r="6.4" fill="#3b3029" stroke="#6e5544" strokeWidth="1.1" />
        <circle cx="47" cy="59" r="4.2" fill="#211a16" />
        <circle cx="73" cy="59" r="4.2" fill="#211a16" />
        <g fill="#0e0b09">
          <circle cx="45.4" cy="57.4" r="0.65" />
          <circle cx="48.6" cy="57.4" r="0.65" />
          <circle cx="45.4" cy="60.6" r="0.65" />
          <circle cx="48.6" cy="60.6" r="0.65" />
          <circle cx="71.4" cy="57.4" r="0.65" />
          <circle cx="74.6" cy="57.4" r="0.65" />
          <circle cx="71.4" cy="60.6" r="0.65" />
          <circle cx="74.6" cy="60.6" r="0.65" />
        </g>
        <g fill="none" stroke="#c8b39d" strokeWidth="0.55" opacity="0.7">
          <path d="M45.4 57.4h3.2M45.4 60.6h3.2M71.4 57.4h3.2M71.4 60.6h3.2" />
        </g>
        <circle cx="44.9" cy="56.6" r="1.45" fill="#fff" opacity="0.58" />
        <circle cx="70.9" cy="56.6" r="1.45" fill="#fff" opacity="0.58" />
        <g fill="none" stroke="#2b211b" strokeWidth="1" strokeLinecap="round" opacity="0.35">
          <path d="M41.5 52.6 Q47 50 52.5 52.6M67.5 52.6 Q73 50 78.5 52.6" />
        </g>
      </g>
    );
  }
  if (style === "happy") {
    return (
      <g fill="none" stroke={stroke} strokeWidth="3.6" strokeLinecap="round">
        <path d="M40 61 Q47 52 54 61" />
        <path d="M66 61 Q73 52 80 61" />
      </g>
    );
  }
  if (style === "sleepy") {
    return (
      <g fill="none" stroke={stroke} strokeWidth="3" strokeLinecap="round">
        <path d="M40 60 Q47 65 54 60" />
        <path d="M66 60 Q73 65 80 60" />
      </g>
    );
  }
  if (style === "sparkle") {
    return (
      <g fill={fill}>
        <path d="M47 50 L49.5 56 L55.5 58.5 L49.5 61 L47 67 L44.5 61 L38.5 58.5 L44.5 56 Z" />
        <path d="M73 50 L75.5 56 L81.5 58.5 L75.5 61 L73 67 L70.5 61 L64.5 58.5 L70.5 56 Z" />
      </g>
    );
  }
  return (
    <g fill="#20242c">
      <ellipse cx="47" cy="59" rx="5.6" ry="6.6" fill={fill} stroke={glass ? "rgb(30 80 140 / 0.25)" : "none"} />
      <ellipse cx="73" cy="59" rx="5.6" ry="6.6" fill={fill} stroke={glass ? "rgb(30 80 140 / 0.25)" : "none"} />
      {!glass ? (
        <>
          <circle cx="48.8" cy="57" r="1.8" fill="#fff" />
          <circle cx="74.8" cy="57" r="1.8" fill="#fff" />
        </>
      ) : null}
    </g>
  );
}
