import os, glob

crates = [
    "neoveg_common", "neoveg_audio", "neoveg_video", "neoveg_engine",
    "neoveg_text", "neoveg_media", "neoveg_h264", "neoveg_aac",
    "neoveg_cfb", "neoveg_aaf", "neoveg_ofx", "neoveg_ofx_sample",
    "neoveg_vst3_sample", "neoveg_ui"
]

for c in crates:
    toml_path = f"crates/{c}/Cargo.toml"
    with open(toml_path, "r", encoding="utf-8") as f:
        content = f.read()
    print(f"=== {c} ===")
    # Extract dependencies
    in_deps = False
    in_dev_deps = False
    deps = []
    dev_deps = []
    crate_type = []
    for line in content.splitlines():
        line_s = line.strip()
        if line_s.startswith("crate-type"):
            crate_type.append(line_s)
        if line_s.startswith("[dependencies]"):
            in_deps = True
            in_dev_deps = False
            continue
        elif line_s.startswith("[dev-dependencies]"):
            in_deps = False
            in_dev_deps = True
            continue
        elif line_s.startswith("["):
            in_deps = False
            in_dev_deps = False
            continue
        if in_deps and line_s and not line_s.startswith("#"):
            deps.append(line_s.split("=")[0].strip())
        elif in_dev_deps and line_s and not line_s.startswith("#"):
            dev_deps.append(line_s.split("=")[0].strip())
    print(f"crate-type: {crate_type or 'rlib'}")
    print(f"dependencies: {', '.join(deps)}")
    print(f"dev-dependencies: {', '.join(dev_deps)}")
    # Find modules / files in src
    src_files = os.listdir(f"crates/{c}/src") if os.path.exists(f"crates/{c}/src") else []
    print(f"src files: {', '.join(sorted(src_files))}")
    print()
