#!/usr/bin/env python3
"""Offline installer regression checks, using an existing native ARM64 executable."""

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib


def main():
    if len(sys.argv) != 2:
        raise SystemExit("Usage: check_install.py /absolute/path/to/native/solus")
    binary = Path(sys.argv[1]).resolve(strict=True)
    installer = Path(__file__).with_name("install.sh")
    with tempfile.TemporaryDirectory(prefix="solus-installer-check-") as tmp:
        base = Path(tmp)
        home = base / "temporary home"
        home.mkdir()
        prefix = home / "local prefix"
        downloads = home / "downloads with spaces"
        env = dict(os.environ, HOME=str(home), XDG_CONFIG_HOME=str(home / ".config"))
        # Tests never invoke apt/rustup or change the real HOME or desktop.
        common = ["bash", str(installer), "--skip-deps", "--binary", str(binary),
                  "--prefix", str(prefix)]

        def install(*args, ok=True):
            result = subprocess.run(common + list(args), env=env, text=True,
                                    capture_output=True)
            assert (result.returncode == 0) == ok, result.stdout + result.stderr
            return result

        for terminal in ("foot", "kitty", "keep"):
            install("--terminal", terminal, "--download-dir", str(downloads))
        launcher = prefix / "bin/solus"
        installed_binary = prefix / "share/solus-rpi/solus"
        config = home / ".config/solus/config.toml"
        assert launcher.is_file() and os.access(launcher, os.X_OK)
        assert installed_binary.stat().st_size == binary.stat().st_size
        assert (home / "Movies/Solus").resolve() == downloads
        with config.open("rb") as stream:
            data = tomllib.load(stream)
        assert data["update"] == {"check_on_startup": False, "auto_install": False}
        config.write_text("# existing user configuration must survive\n")
        install("--terminal", "keep")
        assert config.read_text() == "# existing user configuration must survive\n"
        assert list((prefix / "bin").glob("solus.backup-*"))

        # The selected folder and existing download contents must survive refusals.
        (downloads / "episode.mkv").write_text("existing download")
        install("--terminal", "invalid", ok=False)
        install("--terminal", "keep", "--jobs", "0", ok=False)
        install("--terminal", "keep", "--usb-uuid", "test-uuid", ok=False)
        install("--terminal", "keep", "--download-dir", "relative", ok=False)
        install("--terminal", "keep", "--download-dir", str(home / "other"), ok=False)
        assert (downloads / "episode.mkv").read_text() == "existing download"

        # Mock only the binary/USB utilities to exercise launcher branches offline.
        installed_binary.write_text('#!/bin/sh\nprintf "launched:%s\\n" "$ANI_CLI_DOWNLOAD_DIR"\n')
        installed_binary.chmod(0o755)
        result = subprocess.run([str(launcher)], env=env, text=True, capture_output=True)
        assert result.returncode == 0 and str(downloads) in result.stdout, result.stderr
        settings = prefix / "share/solus-rpi/launcher.conf"
        settings.write_text(settings.read_text().replace("SOLUS_RPI_USB_UUID=''",
                                                         "SOLUS_RPI_USB_UUID=test-uuid"))
        assert "test-uuid" in settings.read_text()
        mock_dir = base / "mock-bin"
        mock_dir.mkdir()
        findmnt = mock_dir / "findmnt"
        findmnt.write_text('#!/bin/sh\ncase "$*" in *-T*) echo wrong-uuid ;; *) exit 0 ;; esac\n')
        findmnt.chmod(0o755)
        guarded_env = dict(env, PATH=str(mock_dir) + os.pathsep + env["PATH"])
        result = subprocess.run([str(launcher)], env=guarded_env, text=True, capture_output=True)
        assert result.returncode != 0 and "not on the configured USB" in result.stderr
        result = subprocess.run(common + ["--terminal", "keep"], env=guarded_env,
                                text=True, capture_output=True)
        assert result.returncode != 0 and "not on the specified USB" in result.stderr
        assert "test-uuid" in settings.read_text()
        findmnt.write_text('#!/bin/sh\ncase "$*" in *-T*) echo test-uuid ;; *) exit 0 ;; esac\n')
        result = subprocess.run([str(launcher)], env=guarded_env, text=True, capture_output=True)
        assert result.returncode == 0 and "launched:" in result.stdout, result.stderr
        result = subprocess.run(common + ["--terminal", "keep"], env=guarded_env,
                                text=True, capture_output=True)
        assert result.returncode == 0 and "test-uuid" in settings.read_text(), result.stderr
        assert config.read_text() == "# existing user configuration must survive\n"
        findmnt.write_text('#!/bin/sh\nexit 1\n')
        udisks = mock_dir / "udisksctl"
        udisks.write_text('#!/bin/sh\nexit 1\n')
        udisks.chmod(0o755)
        result = subprocess.run([str(launcher)], env=guarded_env, text=True, capture_output=True)
        assert result.returncode != 0 and "Connect the configured USB" in result.stderr
        (home / "Movies/Solus").unlink()
        (home / "Movies/Solus").mkdir()
        (home / "Movies/Solus/keep.mkv").write_text("do not replace")
        install("--terminal", "keep", "--download-dir", str(downloads), ok=False)
        assert (home / "Movies/Solus/keep.mkv").read_text() == "do not replace"
    print("PASS: terminal choices, installation, backups, configuration preservation,")
    print("      paths with spaces, download protection, and USB launcher guards.")


if __name__ == "__main__":
    main()
