import json

with open(".agents/teamwork/explorer_m6_3/unchecked_items.json", "r", encoding="utf-8") as f:
    items = json.load(f)

for i in range(11, 62):
    it = items[i]
    print(f"{i+1:2d}. Line {it['line']:4d} | {it['section']}")
    print(f"    {it['text']}")
