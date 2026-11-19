import { useState } from "react";
import { Button } from "../Button/Button";
import { RumahlModal } from "../RumahlDialog/RumahlDialog";

export interface LightboxProps {
  images: readonly { src: string; alt: string }[];
  index: number;
  onIndexChange: (index: number) => void;
  onClose: () => void;
  title: string;
  labels: { close: string; previous: string; next: string; zoomIn: string; zoomOut: string; unavailable: string };
}

/** Controlled image viewer. Native modality restores focus to the gallery opener. */
export function Lightbox({ images, index, onIndexChange, onClose, title, labels }: LightboxProps) {
  const [zoomed, setZoomed] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);
  const current = images[index];
  if (!current) return null;
  const move = (next: number) => {
    setZoomed(false);
    setFailed(null);
    onIndexChange((next + images.length) % images.length);
  };
  return <RumahlModal open title={title} closeLabel={labels.close} onClose={onClose}>
    <div className="rumahl-lightbox" onKeyDown={(event) => {
      if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
        event.preventDefault();
        event.stopPropagation();
        move(index + (event.key === "ArrowLeft" ? -1 : 1));
      }
    }}>
      <div className="rumahl-lightbox__toolbar">
        <Button blurOnMouseUp={false} size="sm" disabled={images.length < 2} onClick={() => move(index - 1)}>{labels.previous}</Button>
        <span aria-live="polite">{index + 1} / {images.length}</span>
        <Button blurOnMouseUp={false} size="sm" disabled={images.length < 2} onClick={() => move(index + 1)}>{labels.next}</Button>
        <Button blurOnMouseUp={false} size="sm" disabled={failed === current.src} active={zoomed} aria-pressed={zoomed} onClick={() => setZoomed(!zoomed)}>{zoomed ? labels.zoomOut : labels.zoomIn}</Button>
      </div>
      <div className="rumahl-lightbox__stage" data-zoomed={zoomed || undefined} tabIndex={0} data-autofocus>
        {failed === current.src ? <p role="status">{labels.unavailable}</p> : <img key={current.src} src={current.src} alt={current.alt} onError={() => setFailed(current.src)} />}
      </div>
      <p className="rumahl-lightbox__caption">{current.alt}</p>
      {images.length > 1 ? <div className="rumahl-lightbox__thumbnails">{images.map((item, i) => <button type="button" key={`${item.src}:${i}`} aria-label={item.alt} aria-pressed={i === index} onClick={() => move(i)}><img src={item.src} alt="" loading="lazy" /></button>)}</div> : null}
    </div>
  </RumahlModal>;
}
