/** Clip the rendered effect in a separate rounded container. SVG backdrop
 * filters can paint a rectangular output despite their own border radius. */
export function mountSurface(host, effect) {
  const frame = document.createElement('div');
  frame.className = 'rumahl-glass-surface';
  Object.assign(frame.style, { position: 'absolute', inset: '0', borderRadius: 'inherit', overflow: 'hidden', pointerEvents: 'none', zIndex: '-1' });
  Object.assign(effect.style, { position: 'absolute', inset: '0', width: '100%', height: '100%', borderRadius: 'inherit', pointerEvents: 'none' });
  frame.append(effect);
  host.prepend(frame);
  return frame;
}
