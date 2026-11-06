import re
import json
import glob

terms = [r'\bsony\b', r'\bvegas\b', r'\bmagix\b', r'\bborisfx\b', r'\bboris\s*fx\b', r'\bsonic\s*foundry\b', r'\bpremiere\b', r'\bdavinci\b', r'\bfinal\s*cut\b', r'\bavid\b']
pattern = re.compile('|'.join(terms), re.IGNORECASE)

files = [
    'crates/neoveg_engine/src/multicam.rs',
    'crates/neoveg_engine/src/sync_link.rs',
    'crates/neoveg_engine/src/takes.rs',
    'crates/neoveg_engine/src/ops.rs',
    'crates/neoveg_ui/src/app.rs',
    'crates/neoveg_engine/tests/m6_features_35_42.rs'
]

print('=== Scanning string literals in Rust source files ===')
for p in files:
    with open(p, 'r', encoding='utf-8') as f:
        for idx, line in enumerate(f, 1):
            for match in re.finditer(r'"([^"\\]*(?:\\.[^"\\]*)*)"', line):
                literal = match.group(1)
                if pattern.search(literal):
                    print(f'{p}:{idx}: Prohibited trademark in string literal: "{literal}"')

print('=== Scanning 31 new translation keys in Assets/i18n/strings/*.json ===')
for path in glob.glob('assets/i18n/strings/*.json'):
    with open(path, 'r', encoding='utf-8') as f:
        data = json.load(f)
    for k, v in data.items():
        if pattern.search(k):
            print(f'{path} key: {k}')
        if pattern.search(v):
            print(f'{path} val: {k} -> {v}')

print('=== Check complete ===')
