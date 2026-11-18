#!/usr/bin/env python3
"""Preserve existing TOML while adding only missing Solus defaults.

This intentionally does not rewrite existing keys or existing [update] settings.
It uses text insertion rather than a TOML serializer so comments and unknown keys
remain intact.
"""

import datetime
from pathlib import Path
import sys
import tempfile


TOP_LEVEL_DEFAULTS = {
    "quality": 'quality = "720p"',
    "audio_mode": 'audio_mode = "sub"',
    "player": 'player = "mpv"',
    "anime4k_mode": 'anime4k_mode = "off"',
    "config_version": "config_version = 2",
}

CACHE_DEFAULTS = {
    "trending_ttl": "trending_ttl = 86400",
    "stable_ttl": "stable_ttl = 604800",
    "detail_ttl": "detail_ttl = 172800",
}


def section_bounds(text: str, name: str):
    marker = f"[{name}]"
    start = text.find(marker)
    if start < 0:
        return None
    line_end = text.find("\n", start)
    if line_end < 0:
        line_end = len(text)
    next_marker = text.find("\n[", line_end)
    end = len(text) if next_marker < 0 else next_marker + 1
    return start, line_end, end


def missing_key(text: str, key: str) -> bool:
    import re
    return re.search(rf"(?m)^\s*{re.escape(key)}\s*=", text) is None


def merge_config(path: Path) -> bool:
    if not path.exists():
        return False

    original = path.read_text(encoding="utf-8")
    text = original.rstrip() + "\n"
    changed = False

    first_section = text.find("\n[")
    insert_top = 0 if first_section < 0 else first_section + 1

    top_lines = []
    for key, line in TOP_LEVEL_DEFAULTS.items():
        if missing_key(text[:first_section if first_section >= 0 else len(text)], key):
            top_lines.append(line)
            changed = True

    if top_lines:
        block = "\n".join(top_lines) + "\n"
        text = text[:insert_top] + block + text[insert_top:]

    cache = section_bounds(text, "cache")
    cache_lines = []
    if cache is None:
        for key, line in CACHE_DEFAULTS.items():
            cache_lines.append(line)
            changed = True
        if cache_lines:
            text = text.rstrip() + '\n\n[cache]\n' + "\n".join(cache_lines) + "\n"
    else:
        start, line_end, end = cache
        section_text = text[start:end]
        missing = [line for key, line in CACHE_DEFAULTS.items() if missing_key(section_text, key)]
        if missing:
            insertion = "\n".join(missing) + "\n"
            text = text[:line_end + 1] + insertion + text[line_end + 1:]
            changed = True

    if not changed:
        return False

    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    backup = path.with_name(f"{path.name}.backup-{stamp}")
    backup.write_text(original, encoding="utf-8")

    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.{stamp}-", dir=str(path.parent))
    try:
        with open(fd, "w", encoding="utf-8", closefd=True) as handle:
            handle.write(text)
        Path(tmp_name).replace(path)
    except Exception:
        Path(tmp_name).unlink(missing_ok=True)
        raise
    return True


def main():
    if len(sys.argv) != 2:
        raise SystemExit("Usage: merge_config.py /absolute/path/to/config.toml")
    path = Path(sys.argv[1])
    try:
        changed = merge_config(path)
    except Exception as exc:
        raise SystemExit(f"Failed to merge config safely: {exc}")
    print("Config merged: missing Solus defaults added" if changed else "Config unchanged: existing values preserved")


if __name__ == "__main__":
    main()
