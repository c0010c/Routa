from pathlib import Path
from PIL import Image
root=Path(__file__).resolve().parent
files=sorted((root/'motion-frames').glob('*.png'))
assert len(files)==64
atlas=Image.new('RGB',(524,284*3),'white')
for i,n in enumerate([0,19,36]):atlas.paste(Image.open(files[n]).convert('RGB').resize((524,284)),(0,i*284))
palette=atlas.quantize(colors=256)
images=[Image.open(p).convert('RGB').quantize(palette=palette,dither=Image.Dither.FLOYDSTEINBERG) for p in files]
images[0].save(root/'03-inline-node-switch.gif',save_all=True,append_images=images[1:],duration=100,loop=0,disposal=1,optimize=True)
print('Created 6.4 second inline node switch GIF.')
