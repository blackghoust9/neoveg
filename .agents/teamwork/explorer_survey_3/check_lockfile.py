import re

with open("Cargo.lock", "r", encoding="utf-8") as f:
    text = f.read()

pkgs = re.findall(r'name = "([^"]+)"', text)
print(f"Total packages in Cargo.lock: {len(pkgs)}")
prohibited = ["openssl-src", "gpl", "agpl"]
found_prohibited = [p for p in pkgs if any(x in p.lower() for x in prohibited)]
print(f"Prohibited packages found: {found_prohibited}")
