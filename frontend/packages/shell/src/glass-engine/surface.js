/** Clip the rendered effect in a separate rounded container. SVG backdrop
 * filters can paint a rectangular output despite their own border radius.
 *
 * The frame is deliberately style-neutral: the glass look (background, border,
 * shadow, backdrop-filter) belongs on the effect layer, which is a direct child
 * of the frame. If the frame carried the surface background, the effect's
 * backdrop-filter would sample that frame instead of the desktop behind it.
 *
 * Note: no `clip-path` here. On the SVG/CSS layers it would form a backdrop
 * root and break their `backdrop-filter` sampling. The WebGL renderer applies
 * its own clip (see `webgl-wallpaper.js`) because it has no backdrop-filter. */
export function mountSurface(host, effect) {
  const frame = document.createElement('div');
  frame.className = 'rumahl-glass-frame';
  Object.assign(frame.style, { position: 'absolute', inset: '0', borderRadius: 'inherit', overflow: 'hidden', pointerEvents: 'none', zIndex: '-1' });
  Object.assign(effect.style, { position: 'absolute', inset: '0', width: '100%', height: '100%', borderRadius: 'inherit', pointerEvents: 'none' });
  frame.append(effect);
  host.prepend(frame);
  return frame;
}
