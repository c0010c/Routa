from pathlib import Path
from PIL import Image
root=Path(__file__).resolve().parent
files=sorted((root/'motion-frames').glob('*.png'));assert len(files)==80
atlas=Image.new('RGB',(732,232*4),'white')
for i,n in enumerate([0,15,28,68]):atlas.paste(Image.open(files[n]).convert('RGB').resize((732,232)),(0,i*232))
palette=atlas.quantize(colors=256)
images=[Image.open(p).convert('RGB').quantize(palette=palette,dither=Image.Dither.FLOYDSTEINBERG) for p in files]
target=root/'04-motion-directions.gif'
images[0].save(target,save_all=True,append_images=images[1:],duration=100,loop=0,disposal=1,optimize=True)
print(f'Created 8-second motion comparison: {target.stat().st_size} bytes.')
