"""Bundle the installed Material icon paths for the offline design prototype."""
from pathlib import Path
import re
import json

ROOT=Path(__file__).parent
ICONS=ROOT.parents[1]/'node_modules'/'@mui'/'icons-material'
source=(ROOT/'prototype.js').read_text(encoding='utf-8')
names=sorted(set(re.findall(r"\b[A-Z][A-Za-z0-9]*Outlined\b",source)))
paths={}
for name in names:
    content=(ICONS/(name+'.mjs')).read_text(encoding='utf-8')
    ds=re.findall(r'd:\s*"([^"]+)"',content)
    assert ds,name
    paths[name]=ds
(ROOT/'icons.js').write_text('/* Material icons from the existing project dependency. */\nwindow.ROUTA_ICONS='+json.dumps(paths,ensure_ascii=False)+';\n',encoding='utf-8')
print(f'Bundled {len(paths)} standard icons')
