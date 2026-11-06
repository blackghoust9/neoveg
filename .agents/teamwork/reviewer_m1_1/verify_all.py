import os
import re
import json

def check_trademarks_in_app():
    with open('crates/neoveg_ui/src/app.rs', 'r', encoding='utf-8') as f:
        lines = f.readlines()

    trademarks = ['vegas', 'premiere', 'magix', 'davinci', 'final cut', 'pro tools', 'avid', 'sound forge', 'acid', 'sony', 'adobe', 'edius', 'lightworks', 'media composer', 'hitfilm', 'camtasia', 'filmora']
    pat = re.compile(r'\b(' + '|'.join(trademarks) + r')\b', re.I)

    matches = []
    str_pat = re.compile(r'"([^"\\]*(?:\\.[^"\\]*)*)"')
    for idx, line in enumerate(lines, 1):
        for s in str_pat.findall(line):
            if pat.search(s):
                matches.append((idx, s))
    print(f"app.rs string literal matches: {len(matches)}")
    for idx, s in matches:
        print(f"  Line {idx}: {s}")
    return len(matches)

def check_assets_i18n():
    trademarks = ['vegas', 'premiere', 'magix', 'davinci', 'final cut', 'pro tools', 'avid', 'sound forge', 'acid', 'sony', 'adobe', 'edius', 'lightworks', 'media composer', 'hitfilm', 'camtasia', 'filmora']
    pat = re.compile(r'\b(' + '|'.join(trademarks) + r')\b', re.I)

    files = [
        'assets/i18n/strings/de-DE.json',
        'assets/i18n/strings/en-US.json',
        'assets/i18n/strings/es-ES.json',
        'assets/i18n/strings/fr-FR.json',
        'assets/i18n/strings/ja-JP.json',
        'assets/i18n/strings/ru-RU.json',
        'assets/i18n/strings/zh-CN.json',
    ]

    total_matches = 0
    catalogs = {}
    for path in files:
        with open(path, 'r', encoding='utf-8') as f:
            data = json.load(f)
            catalogs[path] = data
            for k, v in data.items():
                if pat.search(k):
                    print(f"Trademark in key in {path}: {k}")
                    total_matches += 1
                if pat.search(v):
                    print(f"Trademark in value in {path} ({k}): {v}")
                    total_matches += 1
    print(f"i18n trademark matches: {total_matches}")

    # Check keys count and format strings
    base_file = files[0]
    base_keys = set(catalogs[base_file].keys())
    print(f"Total keys in {base_file}: {len(base_keys)}")

    fmt_mismatches = 0
    missing_keys = 0
    for path in files:
        data = catalogs[path]
        keys = set(data.keys())
        diff1 = base_keys - keys
        diff2 = keys - base_keys
        if diff1 or diff2:
            print(f"Key mismatch in {path}: missing {len(diff1)}, extra {len(diff2)}")
            missing_keys += len(diff1) + len(diff2)

        for k in base_keys:
            if k in data:
                base_braces = catalogs[base_file][k].count('{}')
                cur_braces = data[k].count('{}')
                if base_braces != cur_braces:
                    print(f"Brace mismatch in {path} for key '{k}': {base_braces} vs {cur_braces}")
                    fmt_mismatches += 1

    print(f"Total key discrepancies: {missing_keys}")
    print(f"Total format string mismatches: {fmt_mismatches}")
    return total_matches, missing_keys, fmt_mismatches

if __name__ == '__main__':
    app_matches = check_trademarks_in_app()
    i18n_matches, missing, fmt = check_assets_i18n()
    print("Verification complete.")
