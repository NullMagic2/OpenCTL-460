"""Create a real RGBA cutout of the supplied icon, preserving its original foreground pixels.
The silhouette follows the tablet, red tag and circular settings badge; white badge pixels
are deliberately retained. Supersampling produces antialiased alpha without a white matte.
"""
from pathlib import Path
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parents[1]
source = Image.open(root / 'assets/app-icon-original.png').convert('RGBA')
size = source.size
scale = 4
mask = Image.new('L', (size[0]*scale, size[1]*scale), 0)
draw = ImageDraw.Draw(mask)
def box(values):
    return tuple(round(v*size[0]/1254*scale) for v in values)
def rectangle(values):
    draw.rectangle(box(values), fill=255)
def ellipse(values):
    draw.ellipse(box(values), fill=255)
# Tablet: its top corners have a larger radius than the lower corners.
rectangle((244,265,995,904))
rectangle((164,345,1074,824))
rectangle((244,824,994,904))
ellipse((164,265,324,425))
ellipse((914,265,1074,425))
ellipse((164,744,324,904))
ellipse((914,744,1074,904))
# The red tag sits behind the tablet. Its opaque interior lettering remains unchanged.
draw.rounded_rectangle(box((1055,463,1152,653)), radius=round(24*scale*size[0]/1254), fill=255)
# Settings badge: include the complete white foreground disc, excluding its floor shadow.
ellipse((743,668,1106,1031))
mask = mask.resize(size, Image.Resampling.LANCZOS)
source.putalpha(mask)
# Trim empty margins and center on a square transparent canvas, with modest icon padding.
bounds = mask.getbbox()
cutout = source.crop(bounds)
edge = round(max(cutout.size)*1.12)
icon = Image.new('RGBA', (edge,edge), (0,0,0,0))
icon.alpha_composite(cutout, ((edge-cutout.width)//2,(edge-cutout.height)//2))
icon.save(root / 'assets/app-icon.png')
outputs = root.parent
icon.save(outputs / 'OpenCTL460-icon-transparent.png')
qa = root / 'debug/generated'
qa.mkdir(parents=True, exist_ok=True)
background = Image.new('RGBA', icon.size, '#20242b')
background.alpha_composite(icon)
background.convert('RGB').save(qa / 'icon-on-dark.png')
alpha = icon.getchannel('A')
assert alpha.getextrema() == (0,255)
assert icon.getpixel((0,0))[3] == 0
assert alpha.histogram()[0] > icon.width*icon.height//4
print(f'RGBA PNG verified: {icon.width}x{icon.height}, {alpha.histogram()[0]} fully transparent pixels')
