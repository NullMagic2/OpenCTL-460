"""Remove the pale pen backdrop with a scanline silhouette while retaining interior highlights.
The supplied pen is a single dark, upright object. Each scanline's outer dark boundaries
form the mask; interior lettering and highlights remain opaque. No image generation is used.
"""
from pathlib import Path
from PIL import Image, ImageDraw
import numpy as np
root=Path(__file__).resolve().parents[1]
im=Image.open(root/'assets/bamboo-pen-original.png').convert('RGB')
a=np.asarray(im)
left=[];right=[]
for y in range(im.height):
    xs=np.flatnonzero(a[y].max(axis=1)<170)
    if xs.size:
        left.append((float(xs[0]),float(y)))
        right.append((float(xs[-1]+1),float(y)))
scale=4
mask=Image.new('L',(im.width*scale,im.height*scale),0)
ImageDraw.Draw(mask).polygon([(round(x*scale),round(y*scale)) for x,y in left+right[::-1]],fill=255)
mask=mask.resize(im.size,Image.Resampling.LANCZOS)
alpha=np.asarray(mask).astype(float)/255
rgb=a.astype(float)
# Remove the gray matte from antialiased boundary pixels only.
edge=(alpha>0)&(alpha<1)
rgb[edge]=np.clip((rgb[edge]-(1-alpha[edge,None])*225)/alpha[edge,None],0,255)
out=Image.fromarray(rgb.astype('uint8'),'RGB').convert('RGBA')
out.putalpha(mask)
out.save(root/'assets/bamboo-pen.png')
out.save(root.parent/'OpenCTL460-pen-transparent.png')
preview=Image.new('RGBA',out.size,'#20242b');preview.alpha_composite(out)
preview.convert('RGB').save(root/'debug/generated/pen-on-dark.png')
assert out.getpixel((0,0))[3]==0
print('Pen RGBA alpha range:',mask.getextrema())
