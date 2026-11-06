import os
import re

# List of third-party NLE trademarks to scan for
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
for root, dirs, files in os.walk('crates'):
    for f in files:
        if f.endswith('.rs'):
            path = os.path.join(root, f)
            with open(path, 'r', encoding='utf-8', errors='ignore') as fp:
                for idx, line in enumerate(fp, 1):
                    stripped = line.strip()
                    # We want to check string literals specifically
                    str_matches = re.findall(r'"([^"\\]*(?:\\.[^"\\]*)*)"', line)
                    for s in str_matches:
                        if pattern.search(s):
                            found.append((path, idx, s))

print(f'Total matches in string literals: {len(found)}')
for path, idx, s in found:
    print(f'{path}:{idx}: "{s}"')
