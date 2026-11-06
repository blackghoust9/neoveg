import re
from pathlib import Path

readme = Path("README.md").read_text(encoding="utf-8")
lines = readme.splitlines()

unchecked = []
current_h2 = "Top"
current_h3 = ""

for idx, line in enumerate(lines, 1):
    if line.startswith("## "):
        current_h2 = line[3:].strip()
        current_h3 = ""
    elif line.startswith("### "):
        current_h3 = line[4:].strip()
    
    m = re.match(r"^\s*-\s*\[ \]\s*(.*)", line)
    if m:
        item_text = m.group(1)
        header = f"{current_h2} > {current_h3}" if current_h3 else current_h2
        unchecked.append((idx, header, item_text))

print(f"Total unchecked checkboxes: {len(unchecked)}")
print("=" * 80)
for idx, header, text in unchecked:
    print(f"L{idx:4d} | [{header}] | {text[:100]}...")
