from pathlib import Path
import re

text = Path(r"C:\Users\paulomatos\Documents\GitHub\neoveg\scripts\parity_matrix.py").read_text(encoding="utf-8")
keywords = ["multicam", "take", "sincron", "sync", "link", "split", "corte", "fade", "j-cut", "l-cut"]

for i, line in enumerate(text.splitlines(), 1):
    if any(k in line.lower() for k in keywords):
        if '("' in line:
            print(f"{i}: {line}")
