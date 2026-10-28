/** Independent rumahl Glass Engine MVP. Architecture informed by liquidglass; no third-party source copied. */
export const DEFAULT_MATERIAL = Object.freeze({
  radius: 24, bevel: 15, refraction: 22, chroma: .14,
  blur: 2.5, saturation: 1.35, brightness: 1, tint: 'rgba(18,25,46,.16)'
});
export const PROFILES = Object.freeze({
  high: { backend: 'auto', resolution: 1, maxFps: 60, refraction: 1, blur: 1 },
  balanced: { backend: 'auto', resolution: .72, maxFps: 30, refraction: .72, blur: .7 },
  low: { backend: 'css', resolution: .5, maxFps: 0, refraction: 0, blur: .6 },
});
export const clamp=(v,a,b)=>Math.min(b,Math.max(a,v));
export function normalizeMaterial(material={}) {
  const m={...DEFAULT_MATERIAL,...material};
  const ranges={radius:[0,200],bevel:[2,100],refraction:[0,80],chroma:[0,.6],blur:[0,30],saturation:[.3,2.5],brightness:[.35,1.7]};
  for(const [key,[a,b]] of Object.entries(ranges)) {
    const n=Number(m[key]);m[key]=Number.isFinite(n)?clamp(n,a,b):DEFAULT_MATERIAL[key];
  }
  m.tint=typeof m.tint==='string'&&m.tint.length<120?m.tint:DEFAULT_MATERIAL.tint;
  return m;
}
export function percentile(values, pct){if(!values.length)return 0;const sorted=[...values].sort((a,b)=>a-b);return sorted[Math.ceil((pct/100)*sorted.length)-1]}
export function chooseProfile({p95, longTasks=0, dropped=0, target=60}, previous='high'){
  // Conservative heuristic, not a GPU benchmark: observe compositor-driven frame cadence.
  const budget=1000/target;
  const stress=p95>budget*1.65||longTasks>=3||dropped>.3;
  const stable=p95<budget*1.22&&longTasks===0&&dropped<.12;
  if(stress)return previous==='high'?'balanced':'low';
  if(stable)return previous==='low'?'balanced':'high';
  return previous;
}
export function sourceReady(source){if(!source)return false;return source instanceof HTMLVideoElement?source.readyState>=2&&source.videoWidth>0:source.complete&&source.naturalWidth>0;}
export function sourceDimensions(source){return source instanceof HTMLVideoElement?[source.videoWidth,source.videoHeight]:[source.naturalWidth,source.naturalHeight]}

// Map a replaced element's source pixels into its visible viewport coordinates.
// This deliberately follows CSS replaced-element sizing (object-fit/position).
// Bounding-box dimensions account for translate/scale transforms as used by the
// animated demo; rotated/perspective-transformed media require a quad mapping.
export function sourceToScreenRect(box, width, height, style={}) {
  if(!(width>0 && height>0 && box.width>0 && box.height>0))throw Error('Invalid source rectangle');
  const fit=style.objectFit||'fill';
  let scale=1;
  if(fit==='cover')scale=Math.max(box.width/width,box.height/height);
  else if(fit==='contain')scale=Math.min(box.width/width,box.height/height);
  else if(fit==='scale-down')scale=Math.min(1,Math.min(box.width/width,box.height/height));
  else if(fit==='none')scale=1;
  let outWidth=fit==='fill'?box.width:width*scale;
  let outHeight=fit==='fill'?box.height:height*scale;
  // Percentages of object-position align corresponding points in the image
  // and container. Pixel offsets are supported as well.
  const raw=(style.objectPosition||'50% 50%').trim().split(/\s+/);
  const resolve=(token,free)=>{if(!token)return free*.5;if(token.endsWith('%'))return free*(parseFloat(token)/100);if(token==='center')return free*.5;if(token==='right'||token==='bottom')return free;if(token==='left'||token==='top')return 0;return parseFloat(token)||0};
  const x=resolve(raw[0],box.width-outWidth),y=resolve(raw[1]||'50%',box.height-outHeight);
  return {left:box.left+x,top:box.top+y,width:outWidth,height:outHeight};
}
