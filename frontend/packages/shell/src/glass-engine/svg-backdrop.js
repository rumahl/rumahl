import {mountSurface} from './surface.js';
import {normalizeMaterial} from './core.js';
// Chromium can run an SVG graph on the *real* CSS backdrop. A per-surface Canvas
// displacement texture drives R/G offsets; three passes give chromatic fringing.
const NS='http://www.w3.org/2000/svg';
const make=(name,attrs={})=>{const node=document.createElementNS(NS,name);for(const [k,v] of Object.entries(attrs))node.setAttribute(k,String(v));return node;};
let nextId=0;
export function svgBackdropSupported(){
  return /Chrom(e|ium)|Edg\//.test(navigator.userAgent)&&CSS.supports('backdrop-filter','url(#glass-check)');
}
export class SvgBackdrop {
  constructor(host,material,profile){
    this.host=host;this.material=normalizeMaterial(material);this.profile=profile;
    this.id=`rumahl-glass-${++nextId}`;this.version=0;this.graphVersion=0;this.url=null;this.timer=0;this.destroyed=false;
    this.svg=make('svg',{width:0,height:0,'aria-hidden':'true'});
    Object.assign(this.svg.style,{position:'absolute',width:'0',height:'0',pointerEvents:'none'});
    this.filter=null;document.body.append(this.svg);
    this.layer=document.createElement('div');this.layer.className='rumahl-glass-effect';this.frame=mountSurface(host,this.layer);
    this.geometry='';this.resize=new ResizeObserver(()=>this.schedule());this.resize.observe(host);this.schedule();
  }
  setMaterial(material,profile){this.material=normalizeMaterial(material);this.profile=profile;this.schedule()}
  schedule(){clearTimeout(this.timer);this.timer=setTimeout(()=>this.build(),80)}
  build(){
    if(this.destroyed)return;
    const w=Math.round(this.host.clientWidth),h=Math.round(this.host.clientHeight);
    if(w<3||h<3)return;
    const geometry=[w,h,this.material.radius,this.material.bevel,this.profile.resolution].join(':');
    // Refraction/chroma are shader parameters, not displacement-map geometry.
    // Rebuild the filter graph even when the map can be reused.
    if(this.geometry===geometry&&this.url){this.installFilter(w,h,this.url);return;}
    const ratio=this.profile.resolution;const W=Math.max(3,Math.round(w*ratio)),H=Math.max(3,Math.round(h*ratio));
    const canvas=document.createElement('canvas');canvas.width=W;canvas.height=H;
    const ctx=canvas.getContext('2d',{willReadFrequently:false});if(!ctx)return;
    const image=ctx.createImageData(W,H),px=image.data;
    const r=Math.min(this.material.radius,w/2,h/2),b=Math.min(this.material.bevel,Math.min(w,h)/2);
    for(let y=0;y<H;y++)for(let x=0;x<W;x++){
      const X=(x+.5)/ratio,Y=(y+.5)/ratio;
      const ax=Math.abs(X-w/2),ay=Math.abs(Y-h/2);
      const qx=ax-(w/2-r),qy=ay-(h/2-r),ox=Math.max(qx,0),oy=Math.max(qy,0);
      const sd=Math.hypot(ox,oy)+Math.min(Math.max(qx,qy),0)-r;
      const rim=sd<0?Math.max(0,1+sd/b):0;
      const bend=rim*rim*(3-2*rim);
      const nx=qx>0&&qy>0?ox/(Math.hypot(ox,oy)||1):qx>qy?1:0;
      const ny=qx>0&&qy>0?oy/(Math.hypot(ox,oy)||1):qx>qy?0:1;
      const i=(y*W+x)*4;
      px[i]=128+127*nx*Math.sign(X-w/2)*bend;
      px[i+1]=128+127*ny*Math.sign(Y-h/2)*bend;
      px[i+2]=Math.round(rim*255);px[i+3]=255;
    }
    ctx.putImageData(image,0,0);const ver=++this.version;
    canvas.toBlob(blob=>{
      if(!blob||this.destroyed||ver!==this.version)return;
      const href=URL.createObjectURL(blob);
      const img=new Image();img.onerror=()=>{URL.revokeObjectURL(href);this.geometry='';};
      img.onload=()=>{
        if(this.destroyed||ver!==this.version){URL.revokeObjectURL(href);return}
        this.installFilter(w,h,href);
        this.geometry=geometry;
        if(this.url)URL.revokeObjectURL(this.url);this.url=href;
      };img.src=href;
    });
  }
  installFilter(w,h,href){
    if(this.destroyed)return;
    // A fresh filter ID forces Chromium to invalidate its cached SVG backdrop.
    // The PNG map is reused when the material changes: no Canvas encode here.
    const id=`${this.id}-${++this.graphVersion}`;
    const filter=make('filter',{id,x:0,y:0,width:w,height:h,
      filterUnits:'userSpaceOnUse','color-interpolation-filters':'sRGB'});
    filter.append(make('feImage',{href,x:0,y:0,width:w,height:h,preserveAspectRatio:'none',result:'map'}));
// An SVG filter operates on premultiplied-alpha intermediates.
// Additive feComposite on separately masked RGB passes drives alpha
// above 1, changing hue/brightness or making the filtered backdrop
// disappear. Screen-blend the full-alpha channel passes instead, as
// defined for backdrop color-channel recombination.
// Each matrix preserves source alpha while isolating its channel.
const chroma=this.material.chroma,amount=this.material.refraction*this.profile.refraction;
if(chroma<=0.001){
  filter.append(make('feDisplacementMap',{
    in:'SourceGraphic',in2:'map',scale:amount.toFixed(3),
    xChannelSelector:'R',yChannelSelector:'G'
  }));
}else{
  const channels=[
    ['r',1+chroma,'1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0'],
    ['g',1,'0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 1 0'],
    ['b',1-chroma,'0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 1 0']
  ];
  for(const [name,k,matrix] of channels){
    filter.append(make('feDisplacementMap',{
      in:'SourceGraphic',in2:'map',scale:(amount*k).toFixed(3),
      xChannelSelector:'R',yChannelSelector:'G',result:`d${name}`
    }));
    filter.append(make('feColorMatrix',{in:`d${name}`,type:'matrix',values:matrix,result:name}));
  }
  filter.append(make('feBlend',{in:'r',in2:'g',mode:'screen',result:'rg'}));
  filter.append(make('feBlend',{in:'rg',in2:'b',mode:'screen'}));
}
    const previous=this.filter;
    this.filter=filter;
    this.svg.append(filter);
    this.layer.style.backdropFilter=`url("#${id}") blur(${this.material.blur*this.profile.blur}px) saturate(${this.material.saturation}) brightness(${this.material.brightness})`;
    this.layer.style.webkitBackdropFilter=this.layer.style.backdropFilter;
    this.layer.style.background=this.material.tint;
    if(previous)requestAnimationFrame(()=>previous.remove());
  }
  destroy(){this.destroyed=true;this.version++;clearTimeout(this.timer);this.resize.disconnect();this.frame.remove();this.svg.remove();if(this.url)URL.revokeObjectURL(this.url)}
}
