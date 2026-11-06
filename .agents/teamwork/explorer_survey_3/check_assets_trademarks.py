import os
import re

trademarks = [
    r'\bvegas\b',
    r'\bpremiere\b',
    r'\bmagix\b',
    r'\bdavinci\b',
    r'\bfinal cut\b',
    r'\bpro tools\b',
    r'\bavid\b',
    r'\bsound forge\b',
    r'\bacid\b',
    r'\bsony\b',
    r'\badobe\b',
    r'\bedius\b',
    r'\blightworks\b',
    r'\bmedia composer\b',
    r'\bhitfilm\b',
    r'\bcamtasia\b',
    r'\bfilmora\b',
]

pattern = re.compile('|'.join(trademarks), re.IGNORECASE)
found = []
for root, dirs, files in os.walk('Assets'):
    for f in files:
        if f.endswith('.json'):
            path = os.path.join(root, f)
            with open(path, 'r', encoding='utf-8', errors='ignore') as fp:
                for idx, line in enumerate(fp, 1):
                    m = pattern.search(line)
                    if m:
                        found.append((path, idx, line.strip()))

print(f'Total matches in Assets: {len(found)}')
for path, idx, line in found:
    print(f'{path}:{idx}: {line}')
