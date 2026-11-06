import json
import glob

files = sorted(glob.glob('Assets/i18n/strings/*.json'))
sets = {}
for f in files:
    with open(f, 'r', encoding='utf-8') as fp:
        data = json.load(fp)
        sets[f] = set(data.keys())
        for k, v in data.items():
            assert k.count('{}') == v.count('{}'), f'Mismatch in {f} for key {k}: {k.count("{}")} vs {v.count("{}")}'

all_keys = set().union(*sets.values())
print(f'Total unique keys: {len(all_keys)}')
for f, s in sets.items():
    print(f'{f}: {len(s)} keys (missing: {len(all_keys - s)})')
assert all(len(s) == 1795 and len(all_keys - s) == 0 for s in sets.values()), "Mismatch in keys!"
print('SUCCESS: All 7 catalogs 100% matched, synchronized, 1795 keys, {} counts valid!')
