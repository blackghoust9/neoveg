"""Downloads the open movies of the Blender Foundation to use as real test material.

Usage:
    python scripts/fetch_test_media.py            # the small set (about 110 MB)
    python scripts/fetch_test_media.py --all      # plus the 480p files in AVI, OGG and MOV (about 640 MB more)
    python scripts/fetch_test_media.py --big      # plus Sintel in 1080p and Big Buck Bunny in 1080p (about 1.9 GB more)
    python scripts/fetch_test_media.py --list

The files go to `test_media/` in the repository (ignored by Git, never committed). Tests that use them
look in that folder, or in the one the `NEOVEG_TEST_MEDIA` variable names, and are skipped when it is
empty, so nobody has to download anything.

The movies are free: Creative Commons Attribution 3.0, "(c) Blender Foundation | www.blender.org"
(Big Buck Bunny: peach.blender.org, Sintel: durian.blender.org, Elephants Dream: orange.blender.org,
Tears of Steel: mango.blender.org). If a picture from them is shown anywhere, credit the Blender
Foundation. Standard library only.
"""

import argparse
import sys
import urllib.request
import zipfile
from pathlib import Path

BASE = "https://download.blender.org"

# (set, url, bytes the server announces, what it is good for)
FILES = [
    ("small", f"{BASE}/durian/trailer/sintel_trailer-1080p.mp4", 14_621_544, "H.264 1080p with stereo AAC: a real picture and sound, short"),
    ("small", f"{BASE}/peach/bigbuckbunny_movies/BigBuckBunny_320x180.mp4.zip", 64_657_225, "H.264 320x180: long (10 minutes) and light, for seeking"),
    ("small", f"{BASE}/demo/movies/elephantsdream_teaser.mp4.zip", 29_793_487, "MP4 teaser"),
    ("all", f"{BASE}/peach/bigbuckbunny_movies/big_buck_bunny_480p_surround-fix.avi.zip", 220_514_660, "AVI (DivX/Xvid) with 5.1 surround sound: a sound codec the decoder lacks"),
    ("all", f"{BASE}/peach/bigbuckbunny_movies/big_buck_bunny_480p_stereo.ogg.zip", 166_825_977, "OGG with Theora video and Vorbis sound"),
    ("all", f"{BASE}/peach/bigbuckbunny_movies/big_buck_bunny_480p_h264.mov.zip", 249_230_089, "H.264 in a QuickTime container"),
    ("all", f"{BASE}/demo/movies/tears-of-steel_teaser.mp4.zip", 275_193_406, "live action with effects"),
    ("big", f"{BASE}/demo/movies/Sintel.2010.1080p.mkv", 1_172_428_172, "Matroska 1080p, several sound and subtitle tracks"),
    ("big", f"{BASE}/peach/bigbuckbunny_movies/big_buck_bunny_1080p_h264.mov.zip", 725_106_348, "H.264 1080p, 10 minutes"),
]


def target_folder() -> Path:
    return Path(__file__).resolve().parent.parent / "test_media"


def download(url: str, destination: Path, size: int) -> None:
    if destination.exists() and destination.stat().st_size == size:
        print(f"  already here: {destination.name}")
        return
    print(f"  downloading {destination.name} ({size / 1e6:.0f} MB) ...", flush=True)
    partial = destination.with_suffix(destination.suffix + ".part")
    with urllib.request.urlopen(url, timeout=60) as response, open(partial, "wb") as out:
        done = 0
        while chunk := response.read(1 << 20):
            out.write(chunk)
            done += len(chunk)
            print(f"\r    {done / 1e6:6.0f} / {size / 1e6:.0f} MB", end="", flush=True)
    print()
    if partial.stat().st_size != size:
        partial.unlink()
        raise SystemExit(f"{destination.name}: the size is not the one the server announced")
    partial.replace(destination)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--all", action="store_true", help="also the 480p files in AVI, OGG and MOV")
    parser.add_argument("--big", action="store_true", help="also the 1080p files (and everything above)")
    parser.add_argument("--list", action="store_true", help="only list what would be downloaded")
    args = parser.parse_args()
    wanted = {"small"} | ({"all"} if args.all or args.big else set()) | ({"big"} if args.big else set())
    chosen = [f for f in FILES if f[0] in wanted]
    folder = target_folder()
    total = sum(f[2] for f in chosen)
    print(f"{len(chosen)} files, {total / 1e6:.0f} MB, into {folder}")
    if args.list:
        for _, url, size, note in chosen:
            print(f"  {size / 1e6:7.0f} MB  {url.rsplit('/', 1)[1]}: {note}")
        return 0
    folder.mkdir(exist_ok=True)
    for _, url, size, note in chosen:
        name = url.rsplit("/", 1)[1]
        archive = folder / name
        download(url, archive, size)
        if name.endswith(".zip"):
            with zipfile.ZipFile(archive) as bundle:
                bundle.extractall(folder)
            archive.unlink()
    print("done:", ", ".join(sorted(p.name for p in folder.iterdir())))
    return 0


if __name__ == "__main__":
    sys.exit(main())
