from pathlib import Path
from PIL import Image

root=Path(__file__).resolve().parent
files=sorted((root/'motion-frames').glob('*.png'))
assert len(files)==72
samples=[Image.open(files[n]).convert('RGB') for n in [0,15,40]]
atlas=Image.new('RGB',(640,224*3),'white')
for i,im in enumerate(samples):atlas.paste(im.resize((640,224)),(0,224*i))
palette=atlas.quantize(colors=256)
frames=[Image.open(f).convert('RGB').quantize(palette=palette,dither=Image.Dither.FLOYDSTEINBERG) for f in files]
output=root/'03-node-switch.gif'
frames[0].save(output,save_all=True,append_images=frames[1:],duration=100,loop=0,optimize=True,disposal=1)
print(f'{output.name}: {output.stat().st_size} bytes; 7.2 seconds; 1280 x 448')
