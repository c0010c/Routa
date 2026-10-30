"""Restore the earlier navigation spacing without a second application brand."""
from pathlib import Path
import xml.etree.ElementTree as ET

root = Path(__file__).parent
source = root.parent / 'overview-v4'
for name in ('overview-mica-route', 'overview-mica-node-picker'):
    svg = (source / f'{name}.svg').read_text(encoding='utf-8')
    begin = svg.index('<rect x="9" y="54"')
    end = svg.index('<circle cx="34" cy="757" r="5.5"', begin)
    svg = (svg[:begin] + '<g id="navigation" transform="translate(0 57)">\n'
           + svg[begin:end] + '</g>\n' + svg[end:])
    ET.fromstring(svg)
    assert svg.count('>Routa</text>') == 1
    (root / f'{name}.svg').write_text(svg, encoding='utf-8')
