import json

with open(".agents/teamwork/explorer_m6_3/unchecked_items.json", "r", encoding="utf-8") as f:
    items = json.load(f)

for i, it in enumerate(items, 1):
    print(f"{i:2d}. Line {it['line']:4d} | {it['section']}")
    print(f"    {it['text']}")
