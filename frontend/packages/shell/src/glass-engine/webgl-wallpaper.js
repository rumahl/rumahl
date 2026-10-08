import {mountSurface} from './surface.js';
import {normalizeMaterial,sourceReady,sourceDimensions,sourceToScreenRect} from './core.js';
// Wallpaper-only path: genuinely refracts image/video texels. It cannot see
// arbitrary DOM content, which is why SVG live backdrop remains a distinct mode.
const VS=`attribute vec2 position;varying vec2 uv;void main(){uv=vec2((position.x+1.)*.5,(1.-position.y)*.5);gl_Position=vec4(position,0.,1.);}`;
const FS=`precision highp float;varying vec2 uv;uniform sampler2D image;uniform vec2 size,offset,step;uniform float radius,bevel,bend,chroma,brightness,saturation,blur;
vec2 displacement(vec2 p){vec2 h=size*.5;vec2 c=p-h;vec2 q=abs(c)-(h-vec2(radius));vec2 o=max(q,0.);float sd=length(o)+min(max(q.x,q.y),0.)-radius;float v=clamp(1.+sd/max(bevel,1.),0.,1.);float b=v*v*(3.-2.*v);vec2 n=q.x>0.&&q.y>0.?o/max(length(o),.0001):q.x>q.y?vec2(1.,0.):vec2(0.,1.);return n*sign(c)*b;}
vec3 sampleGlass(vec2 p){vec2 d=displacement(p);vec2 location=offset+p*step;vec2 shift=d*bend*step;return vec3(texture2D(image,location+shift*(1.+chroma)).r,texture2D(image,location+shift).g,texture2D(image,location+shift*(1.-chroma)).b);}
void main(){vec2 p=uv*size;vec3 c=sampleGlass(p);float s=blur*.75;c=(c*2.+sampleGlass(p+vec2(s,s))+sampleGlass(p+vec2(-s,s))+sampleGlass(p+vec2(s,-s))+sampleGlass(p-vec2(s,s)))/6.;float gray=dot(c,vec3(.2126,.7152,.0722));c=mix(vec3(gray),c,saturation)*brightness;gl_FragColor=vec4(c,1.);}`;
function shader(gl,type,source){const s=gl.createShader(type);gl.shaderSource(s,source);gl.compileShader(s);if(!gl.getShaderParameter(s,gl.COMPILE_STATUS))throw new Error(gl.getShaderInfoLog(s)||'Shader compilation error');return s}
export class WebGLWallpaper {
  constructor(host,material,profile,getSource){
    this.host=host;this.getSource=getSource;this.material=normalizeMaterial(material);this.profile=profile;
    this.canvas=document.createElement('canvas');this.canvas.className='rumahl-glass-webgl';
    // The GPU drawing buffer MUST NOT participate in CSS layout. Intrinsic
    // canvas width/height change every time the renderer chooses resolution;
    // strict containment prevents those dimensions feeding back into flex sizing.
    Object.assign(this.canvas.style,{position:'absolute',inset:'0',width:'100%',height:'100%',maxWidth:'100%',maxHeight:'100%',minWidth:'0',minHeight:'0',contain:'strict',pointerEvents:'none',opacity:'0'});
    this.frame=mountSurface(host,this.canvas);
    const gl=this.canvas.getContext('webgl',{alpha:false,antialias:false,preserveDrawingBuffer:false});if(!gl)throw Error('WebGL unavailable');this.gl=gl;
    this.lost=false;this.dead=false;this.last='';this.uploaded=null;this.lastVideoTime=-1;this.lastDraw=0;
    this.onLost=e=>{e.preventDefault();this.lost=true;this.canvas.style.opacity='0'};
    this.onRestore=()=>{this.lost=false;this.init();this.last='';this.uploaded=null};
    this.canvas.addEventListener('webglcontextlost',this.onLost);this.canvas.addEventListener('webglcontextrestored',this.onRestore);
    this.init();this.visible=true;
    this.observer=new IntersectionObserver(entries=>{this.visible=!!entries[0]?.isIntersecting});this.observer.observe(host);
    this.frame=this.frame.bind(this);this.frameId=requestAnimationFrame(this.frame);
  }
  init(){const gl=this.gl;const vert=shader(gl,gl.VERTEX_SHADER,VS),frag=shader(gl,gl.FRAGMENT_SHADER,FS),program=gl.createProgram();gl.attachShader(program,vert);gl.attachShader(program,frag);gl.linkProgram(program);if(!gl.getProgramParameter(program,gl.LINK_STATUS))throw Error(gl.getProgramInfoLog(program)||'Program linking error');this.vert=vert;this.frag=frag;this.program=program;
    gl.useProgram(program);this.buffer=gl.createBuffer();gl.bindBuffer(gl.ARRAY_BUFFER,this.buffer);gl.bufferData(gl.ARRAY_BUFFER,new Float32Array([-1,-1,3,-1,-1,3]),gl.STATIC_DRAW);const loc=gl.getAttribLocation(program,'position');gl.enableVertexAttribArray(loc);gl.vertexAttribPointer(loc,2,gl.FLOAT,false,0,0);
    this.texture=gl.createTexture();gl.bindTexture(gl.TEXTURE_2D,this.texture);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);
    this.uniform={};for(const name of ['size','offset','step','radius','bevel','bend','chroma','brightness','saturation','blur'])this.uniform[name]=gl.getUniformLocation(program,name);
  }
  setMaterial(material,profile){this.material=normalizeMaterial(material);this.profile=profile;this.last=''}
  frame(now){if(this.dead)return;this.frameId=requestAnimationFrame(this.frame);if(this.lost||!this.visible)return;
    if(now-this.lastDraw<1000/this.profile.maxFps-1)return;
    const source=this.getSource();if(!sourceReady(source))return;
    const srcBox=source.getBoundingClientRect(),box=this.host.getBoundingClientRect();const isVideo=source instanceof HTMLVideoElement;
    const identity=isVideo?'video':source.currentSrc||source.src;
    const style=getComputedStyle(source);const state=[identity,box.x,box.y,box.width,box.height,srcBox.x,srcBox.y,srcBox.width,srcBox.height,style.objectFit,style.objectPosition,this.material.radius,this.material.refraction,this.material.chroma,this.material.blur,this.material.saturation,this.material.brightness,this.profile.resolution].join(':');
    if(this.last===state&&(!isVideo||source.currentTime===this.lastVideoTime))return;
    const gl=this.gl,m=this.material,p=this.profile;
    // Layout dimensions are the host's content box, never the canvas bitmap.
    // Reading canvas.getBoundingClientRect here can cause runaway scaling.
    const w=this.host.clientWidth,h=this.host.clientHeight;
    if(w<2||h<2)return;
    const dpr=Math.min(devicePixelRatio||1,2)*p.resolution;const W=Math.max(2,Math.round(w*dpr)),H=Math.max(2,Math.round(h*dpr));
    if(this.canvas.width!==W||this.canvas.height!==H){this.canvas.width=W;this.canvas.height=H}gl.viewport(0,0,W,H);
    const [iw,ih]=sourceDimensions(source);
    // Match the visible replaced element: CSS object-fit / object-position and
    // its transformed bounding box, rather than assuming centered 'cover'.
    const display=sourceToScreenRect(srcBox,iw,ih,getComputedStyle(source));
    // The fragment shader uses top-left UVs. For HTML image/video uploads
    // UNPACK_FLIP_Y_WEBGL=false keeps texture coordinate (0,0) at its top.
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL,false);
    try{if(identity!==this.uploaded||isVideo){gl.bindTexture(gl.TEXTURE_2D,this.texture);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,gl.RGBA,gl.UNSIGNED_BYTE,source);this.uploaded=identity}}catch{this.canvas.style.opacity='0';return}
    gl.useProgram(this.program);
    gl.uniform2f(this.uniform.size,w,h);gl.uniform2f(this.uniform.offset,(box.left-display.left)/display.width,(box.top-display.top)/display.height);
    gl.uniform2f(this.uniform.step,(box.width/w)/display.width,(box.height/h)/display.height);
    for(const [key,val] of Object.entries({radius:Math.min(m.radius,w/2,h/2),bevel:m.bevel,bend:m.refraction*p.refraction,chroma:m.chroma,brightness:m.brightness,saturation:m.saturation,blur:m.blur*p.blur}))gl.uniform1f(this.uniform[key],val);
    gl.drawArrays(gl.TRIANGLES,0,3);this.canvas.style.opacity='1';this.last=state;this.lastVideoTime=isVideo?source.currentTime:-1;this.lastDraw=now;
  }
  destroy(){this.dead=true;cancelAnimationFrame(this.frameId);this.observer.disconnect();this.canvas.removeEventListener('webglcontextlost',this.onLost);this.canvas.removeEventListener('webglcontextrestored',this.onRestore);const gl=this.gl;if(!gl.isContextLost()){gl.deleteTexture(this.texture);gl.deleteBuffer(this.buffer);gl.deleteProgram(this.program);gl.deleteShader(this.vert);gl.deleteShader(this.frag)}this.frame.remove()}
}
