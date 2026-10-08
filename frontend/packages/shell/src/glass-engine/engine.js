import {DEFAULT_MATERIAL,PROFILES,normalizeMaterial,percentile,chooseProfile} from './core.js';
import {SvgBackdrop,svgBackdropSupported} from './svg-backdrop.js';
import {WebGLWallpaper} from './webgl-wallpaper.js';

class CssFrosted {
  constructor(host,material){this.host=host;this.layer=document.createElement('div');this.layer.className='rumahl-glass-surface';host.prepend(this.layer);this.setMaterial(material)}
  setMaterial(material){const m=normalizeMaterial(material);this.layer.style.backdropFilter=`blur(${Math.max(8,m.blur*4)}px) saturate(${m.saturation}) brightness(${m.brightness})`;this.layer.style.webkitBackdropFilter=this.layer.style.backdropFilter;this.layer.style.background=m.tint}
  destroy(){this.layer.remove()}
}
/** Stable theme API: surface attributes are the contract, shaders are private. */
export class GlassEngine extends EventTarget {
  constructor({source=()=>null,quality='auto',backend='auto',targetFps=60}={}){
    super();this.source=source;this.preference=quality;this.requestedBackend=backend;this.targetFps=targetFps;
    this.profile=quality==='auto'?'high':quality;this.backendIndex=0;this.lastChange=performance.now();this.surfaces=new Map();this.alive=true;
    this.samples=[];this.lastSample=performance.now();this.prevFrame=this.lastSample;this.slowWindows=0;this.fastWindows=0;this.longTasks=0;
    this.reduced=matchMedia('(prefers-reduced-transparency: reduce)');this.contrast=matchMedia('(prefers-contrast: more)');
    this.onPreference=()=>this.reconcile();this.reduced.addEventListener('change',this.onPreference);this.contrast.addEventListener('change',this.onPreference);
    try{this.longObserver=new PerformanceObserver(list=>{this.longTasks+=list.getEntries().length});this.longObserver.observe({entryTypes:['longtask']})}catch{}
    this.tick=this.tick.bind(this);this.raf=requestAnimationFrame(this.tick);
  }
  get capability(){return {svgBackdrop:svgBackdropSupported(),webgl:!!document.createElement('canvas').getContext('webgl'),reducedTransparency:this.reduced.matches||this.contrast.matches}}
  get ladder(){const l=[];if(svgBackdropSupported())l.push('svg');if(this.capability.webgl)l.push('webgl');l.push('css');return l;}
  get renderer(){if(this.reduced.matches||this.contrast.matches)return'css';
    if(this.requestedBackend==='css')return'css';if(this.requestedBackend==='svg')return svgBackdropSupported()?'svg':'css';
    if(this.requestedBackend==='webgl')return this.capability.webgl?'webgl':'css';
    const l=this.ladder;return l[Math.min(this.backendIndex,l.length-1)];}
  mount(element,material={}){
    if(this.surfaces.has(element))return this.surfaces.get(element).handle;
    const ownsHostClass=!element.classList.contains('rumahl-glass-host');element.classList.add('rumahl-glass-host');const record={element,ownsHostClass,material:normalizeMaterial(material),impl:null,kind:''};
    const handle={update:(patch)=>{record.material=normalizeMaterial({...record.material,...patch});this.updateRecord(record)},destroy:()=>{this.removeRecord(record)}};
    record.handle=handle;this.surfaces.set(element,record);this.updateRecord(record);return handle;
  }
  removeRecord(record){record.impl?.destroy();this.surfaces.delete(record.element);if(record.ownsHostClass)record.element.classList.remove('rumahl-glass-host')}
  updateRecord(record){const kind=this.renderer,profile=PROFILES[this.profile];
    if(record.kind!==kind){record.impl?.destroy();record.impl=null;record.kind=kind;try{
      record.impl=kind==='svg'?new SvgBackdrop(record.element,record.material,profile):kind==='webgl'?new WebGLWallpaper(record.element,record.material,profile,this.source):new CssFrosted(record.element,record.material);
    }catch(e){console.warn('[rumahl/glass] renderer failed; CSS fallback',e);record.kind='css';record.impl=new CssFrosted(record.element,record.material)}
    }else record.impl?.setMaterial(record.material,profile);
  }
  reconcile(){for(const record of this.surfaces.values())this.updateRecord(record);this.dispatchEvent(new CustomEvent('qualitychange',{detail:{quality:this.profile,renderer:this.renderer}}))}
  setQuality(quality){if(!['auto','high','balanced','low'].includes(quality))throw Error('Unknown quality');this.preference=quality;this.profile=quality==='auto'?'high':quality;this.samples.length=0;this.slowWindows=0;this.fastWindows=0;this.reconcile()}
  setBackend(backend){if(!['auto','svg','webgl','css'].includes(backend))throw Error('Unknown backend');this.requestedBackend=backend;this.reconcile()}
  tick(now){if(!this.alive)return;this.raf=requestAnimationFrame(this.tick);const elapsed=now-this.prevFrame;this.prevFrame=now;
    if(document.hidden){this.samples.length=0;this.lastSample=now;return}
    if(elapsed>0&&elapsed<250)this.samples.push(elapsed);
    if(now-this.lastSample<3000)return;
    const p95=percentile(this.samples,95),avg=this.samples.reduce((a,b)=>a+b,0)/(this.samples.length||1);
    const missed=this.samples.filter(x=>x>1000/this.targetFps*1.55).length/(this.samples.length||1);
    const metrics={p95,avg,longTasks:this.longTasks,dropped:missed,target:this.targetFps,quality:this.profile,renderer:this.renderer};
    this.dispatchEvent(new CustomEvent('metrics',{detail:metrics}));
    if(this.preference==='auto'&&this.samples.length>=50){
      const proposed=chooseProfile(metrics,this.profile);
      if(PROFILES[proposed].resolution<PROFILES[this.profile].resolution){
        // Stress: reduce glass QUALITY first (high→balanced→low). Only when
        // quality is already lowest do we step the RENDERER down the ladder.
        // A slow frame is not necessarily the glass (another app may load the
        // GPU), so we never switch renderer on the first dip.
        this.slowWindows++;this.fastWindows=0;
        if(this.slowWindows>=2){
          this.slowWindows=0;
          if(this.profile!=='low'){this.profile=proposed;this.reconcile()}
          else if(this.backendIndex<this.ladder.length-1){this.backendIndex++;this.profile='high';this.lastChange=now;this.reconcile()}
        }
      } else if(PROFILES[proposed].resolution>PROFILES[this.profile].resolution){
        this.fastWindows++;this.slowWindows=0;
        if(this.fastWindows>=5){this.fastWindows=0;this.profile=proposed;this.reconcile()}
      } else {
        // Stable at this quality: recover the renderer slowly and only after a
        // cooldown, so a momentary recovery cannot cause back-and-forth switching.
        this.fastWindows++;this.slowWindows=0;
        if(this.fastWindows>=8&&now-this.lastChange>15000&&this.backendIndex>0){this.backendIndex--;this.fastWindows=0;this.lastChange=now;this.reconcile()}
      }
    }
    this.samples.length=0;this.longTasks=0;this.lastSample=now;
  }
  destroy(){this.alive=false;cancelAnimationFrame(this.raf);this.longObserver?.disconnect();this.reduced.removeEventListener('change',this.onPreference);this.contrast.removeEventListener('change',this.onPreference);for(const record of [...this.surfaces.values()])this.removeRecord(record)}
}
export {DEFAULT_MATERIAL,PROFILES};
