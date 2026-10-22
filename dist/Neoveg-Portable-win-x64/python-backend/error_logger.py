"""
Neoveg — Centralized Multi-Language Logging & Crash Diagnostics Engine (`logs/neoveg.log` & `logs/crash.log`)
Implements Section 8 (Logging & Crash Diagnostics Protocol) and Section 7 (Workspace & Directory Synchronization Protocol)
of AGENTS.md:
- Configures Python `logging` with `RotatingFileHandler` (`logs/neoveg.log`, 10MB limit, 5 historical backups)
  and Console sink.
- Records unhandled exceptions into `logs/crash.log` via `sys.excepthook`.
- Synchronizes `logs/neoveg.log` and `logs/crash.log` with `G:\\Meu Drive\\vp-rev-eng\\teste\\teste2\\logs`.
"""

from __future__ import annotations
import datetime
import logging
from logging.handlers import RotatingFileHandler
import shutil
import sys
import threading
import traceback
from pathlib import Path
from typing import Dict, List

ROOT_DIR = Path(__file__).resolve().parent.parent
LOGS_DIR = ROOT_DIR / "logs"
NEOVEG_LOG_FILE = LOGS_DIR / "neoveg.log"
CRASH_LOG_FILE = LOGS_DIR / "crash.log"
ERROR_LOG_FILE = ROOT_DIR / "log_erros.txt"
GDRIVE_SYNC_ROOT = Path(r"G:\Meu Drive\vp-rev-eng\teste\teste2")
GDRIVE_LOGS_DIR = GDRIVE_SYNC_ROOT / "logs"

MAX_LOG_BYTES = 10 * 1024 * 1024  # 10 MB per log file
MAX_LOG_BACKUPS = 5               # Up to 5 historical log files

_LOCK = threading.Lock()
_LOGGER_INITIALIZED = False


def _timestamp() -> str:
    return datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")


def get_neoveg_logger() -> logging.Logger:
    """Configures and returns the centralized Python `neoveg` logger writing to `logs/neoveg.log`."""
    global _LOGGER_INITIALIZED
    logger = logging.getLogger("neoveg")
    if _LOGGER_INITIALIZED and logger.handlers:
        return logger

    LOGS_DIR.mkdir(parents=True, exist_ok=True)
    logger.setLevel(logging.DEBUG)
    formatter = logging.Formatter("[%(asctime)s] [%(levelname)s] [Python/%(name)s] %(message)s", "%Y-%m-%d %H:%M:%S")

    file_handler = RotatingFileHandler(
        NEOVEG_LOG_FILE,
        maxBytes=MAX_LOG_BYTES,
        backupCount=MAX_LOG_BACKUPS,
        encoding="utf-8",
    )
    file_handler.setLevel(logging.DEBUG)
    file_handler.setFormatter(formatter)
    logger.addHandler(file_handler)

    daily_file = LOGS_DIR / f"neoveg.{datetime.date.today().strftime('%Y-%m-%d')}.log"
    daily_handler = RotatingFileHandler(
        daily_file,
        maxBytes=MAX_LOG_BYTES,
        backupCount=MAX_LOG_BACKUPS,
        encoding="utf-8",
    )
    daily_handler.setLevel(logging.INFO)
    daily_handler.setFormatter(formatter)
    logger.addHandler(daily_handler)

    _install_sys_excepthook(logger)
    _LOGGER_INITIALIZED = True
    return logger


def _install_sys_excepthook(logger: logging.Logger) -> None:
    orig_hook = sys.excepthook

    def _neoveg_excepthook(exc_type, exc_value, exc_tb):
        tb_str = "".join(traceback.format_exception(exc_type, exc_value, exc_tb))
        logger.critical("Unhandled Python exception:\n%s", tb_str)
        try:
            LOGS_DIR.mkdir(parents=True, exist_ok=True)
            with open(CRASH_LOG_FILE, "a", encoding="utf-8") as cf:
                cf.write(f"[{_timestamp()}] [FATAL] [Python/UnhandledException] {exc_type.__name__}: {exc_value}\n{tb_str}\n")
            sync_logs_to_gdrive()
        except OSError:
            pass
        orig_hook(exc_type, exc_value, exc_tb)

    sys.excepthook = _neoveg_excepthook


def sync_logs_to_gdrive() -> bool:
    """Synchronizes `logs/` with `G:\\Meu Drive\\vp-rev-eng\\teste\\teste2\\logs` when accessible."""
    try:
        if not LOGS_DIR.exists():
            return False
        if GDRIVE_SYNC_ROOT.exists():
            GDRIVE_LOGS_DIR.mkdir(parents=True, exist_ok=True)
            if LOGS_DIR.resolve() != GDRIVE_LOGS_DIR.resolve():
                for item in LOGS_DIR.iterdir():
                    if item.is_file():
                        shutil.copy2(item, GDRIVE_LOGS_DIR / item.name)
            return True
    except OSError:
        pass
    return False


def init_error_log() -> None:
    """Initializes `logs/neoveg.log` and `log_erros.txt`."""
    logger = get_neoveg_logger()
    with _LOCK:
        if not ERROR_LOG_FILE.exists():
            entries = [
                "=" * 88,
                "NEOVEG — REGISTRO DE ERROS, DIAGNÓSTICOS E CORREÇÕES (log_erros.txt & logs/neoveg.log)",
                f"Iniciado em: {_timestamp()}",
                "=" * 88,
                "",
                "[1] DIAGNÓSTICO DE AMBIENTE E COMPILADORES:",
                f"  - [{_timestamp()}] [INFO/ENV] Protocolo centralizado de logs ativo em `logs/neoveg.log` e `logs/crash.log`.",
                "",
                "[2] REGISTRO DE EVENTOS EM TEMPO DE EXECUÇÃO (RUNTIME LOG):",
            ]
            ERROR_LOG_FILE.write_text("\n".join(entries) + "\n", encoding="utf-8")
    logger.debug("Initialized Neoveg logging subsystem")
    sync_logs_to_gdrive()


def log_error(category: str, message: str, detail: str = "", level: str = "INFO") -> Dict[str, str]:
    init_error_log()
    logger = get_neoveg_logger()
    ts = _timestamp()
    line = f"  - [{ts}] [{category.upper()}] {message}"
    if detail:
        clean_detail = " | ".join(d.strip() for d in detail.strip().splitlines() if d.strip())
        line += f" -> Detalhes: {clean_detail}"

    lvl = level.upper()
    log_msg = f"[{category.upper()}] {message}" + (f" ({detail})" if detail else "")
    if lvl == "ERROR":
        logger.error(log_msg)
    elif lvl == "WARN":
        logger.warning(log_msg)
    elif lvl == "FATAL":
        logger.critical(log_msg)
    elif lvl == "DEBUG":
        logger.debug(log_msg)
    else:
        logger.info(log_msg)

    with _LOCK:
        with open(ERROR_LOG_FILE, "a", encoding="utf-8") as f:
            f.write(line + "\n")
    sync_logs_to_gdrive()
    return {"timestamp": ts, "category": category, "message": message, "detail": detail}


def log_exception(category: str, exc: BaseException) -> Dict[str, str]:
    tb = "".join(traceback.format_exception(type(exc), exc, exc.__traceback__))
    try:
        LOGS_DIR.mkdir(parents=True, exist_ok=True)
        with open(CRASH_LOG_FILE, "a", encoding="utf-8") as cf:
            cf.write(f"[{_timestamp()}] [ERROR] [Python/{category}] {type(exc).__name__}: {exc}\n{tb}\n")
    except OSError:
        pass
    return log_error(category, f"{type(exc).__name__}: {exc}", tb[-500:], level="ERROR")


def read_error_log() -> List[str]:
    init_error_log()
    try:
        return ERROR_LOG_FILE.read_text(encoding="utf-8").splitlines()
    except Exception as exc:
        return [f"Error reading log_erros.txt: {exc}"]
