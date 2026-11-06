"""Light-theme variants of the compact review mockups; app source is untouched."""
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).parent
SOURCE = ROOT.parent / 'simple-mode-v2'
ET.register_namespace('', 'http://www.w3.org/2000/svg')

# Use the existing app's light palette with blue-tinted selected surfaces.
PALETTE = {
    '#202020': '#f3f3f3',
    '#1b1b1b': '#f3f3f3',
    '#f3f3f3': '#1b1b1b',
    '#c0c0c0': '#616161',
    '#929292': '#707070',
    '#3c3c3c': '#dedede',
    '#4cc2ff': '#0067c0',
    '#82cd8c': '#0f7b0f',
    '#444444': '#d1d5db',
    '#17222b': '#ffffff',
    '#1d1d1d': '#e7e9ed',
    '#3b3b3b': '#ffffff',
    '#2c2c2c': '#ffffff',
    '#414141': '#dfe2e6',
    '#282828': '#ffffff',
    '#3e3e3e': '#dfe2e6',
    '#2b3033': '#edf6fd',
    '#262626': '#ffffff',
    '#42606c': '#8ab9dd',
    '#3d3d3d': '#dfe2e6',
    '#363636': '#edf0f3',
    '#29353b': '#eaf3fc',
    '#7f7f7f': '#8b8b8b',
    '#2b2b2b': '#ffffff',
    '#333333': '#ffffff',
    '#484848': '#b6bdc6',
    '#14212a': '#ffffff',
    '#c3c3c3': '#ffffff',
}

for name in ['01-compact-daily', '02-single-page-apps', '03-first-launch', '04-compact-settings']:
    tree = ET.parse(SOURCE / (name + '.svg'))
    for element in tree.iter():
        for attribute in ['fill', 'stroke']:
            value = element.get(attribute)
            if value and value.startswith('#'):
                assert value in PALETTE, (name, attribute, value)
                element.set(attribute, PALETTE[value])
        if element.tag.endswith('title'):
            element.text = (element.text or '') + ' · 亮色版'
    tree.write(ROOT / (name + '.svg'), encoding='utf-8', xml_declaration=True)

print('Saved four light-theme SVG previews')
