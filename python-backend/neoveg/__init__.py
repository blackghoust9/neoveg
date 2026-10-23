"""
Neoveg Official Python Package (`neoveg`).
Exposes the core Python automation, FFmpeg isolation, Rust FFI bridge,
XML interchange, missing media relinker, and AI scripting modules.
"""

from __future__ import annotations
import sys
from pathlib import Path

_BACKEND_DIR = Path(__file__).resolve().parent.parent
if str(_BACKEND_DIR) not in sys.path:
    sys.path.insert(0, str(_BACKEND_DIR))

import error_logger
import ffmpeg_manager
import media_relinker
import neoveg_integration
import plugins_engine
import rust_bridge
import neoveg_scripting
import workspace_sync
import xml_interchange

__version__ = "2.1.0"
__all__ = [
    "error_logger",
    "ffmpeg_manager",
    "media_relinker",
    "neoveg_integration",
    "plugins_engine",
    "rust_bridge",
    "neoveg_scripting",
    "workspace_sync",
    "xml_interchange",
]
