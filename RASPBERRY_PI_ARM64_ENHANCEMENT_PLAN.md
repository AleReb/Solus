# Solus Raspberry Pi ARM64 Pull-Request Enhancement Plan

**Target PR:** `AleReb:raspberry-pi-arm64`
**Goal:** Turn the optional Raspberry Pi/CM5 installer into a secure, repeatable, low-risk installation path that can be maintained alongside Solus without changing existing macOS/Linux/Windows installers or application behavior.

## 1. Desired Outcome

After this enhancement, a normal user can:

1. Run an ARM64 installer on Debian/Raspberry Pi OS without accidentally modifying unrelated system settings.
2. Preview exactly what will change before installation.
3. Reinstall or uninstall Solus reliably.
4. Safely use a local build or prebuilt native ARM64 binary.
5. Preserve existing Solus configuration, playlists, downloads, and launcher files.
6. Optionally place downloads on a USB filesystem with a verified UUID and mount guard.
7. Install Foot, Kitty, or keep the current terminal setup.
8. Know whether every downloaded dependency has been pinned and verified.

The original application, `Cargo.toml`, `Cargo.lock`, existing installers, and release workflow must remain unchanged unless explicitly approved in a separate change.

---

## Implementation Status

- **Phase 1 — Safety foundations:** implemented on `raspberry-pi-arm64-review`
- **Phase 2 — Supply chain and lifecycle:** implemented on `raspberry-pi-arm64-review`; final validation in progress
- **Phase 3 — Configuration and USB:** implemented on `raspberry-pi-arm64-review`; final validation in progress
- **Phase 4 — CI and documentation:** implemented on `raspberry-pi-arm64-review`; final validation in progress
- **Phase 5 — Maintainer review:** pending

## 2. Scope

### In scope

- `scripts/raspberry-pi/install.sh`
- `scripts/raspberry-pi/build.sh`
- `scripts/raspberry-pi/launcher.sh`
- `scripts/raspberry-pi/check_install.py`
- Raspberry Pi documentation
- ARM64 GitHub Actions coverage
- Optional README additions

### Out of scope

- Rewriting Solus playback or UI code
- Removing or replacing the existing upstream installers
- Cross-compiling from non-ARM64 systems
- Supporting 32-bit Raspberry Pi OS
- Claiming universal codec compatibility
- Modifying application release binaries or the Rust package manifest

---

## 3. Reliability Principles

1. **No surprise mutation:** print a change plan before apt, rustup, file, symlink, launcher, or config changes.
2. **Preserve by default:** never overwrite or delete existing config, downloads, or launchers without an explicit migration strategy.
3. **Fail before mutation:** validate architecture, arguments, paths, USB state, disk space, tools, and permissions before package installation or compilation.
4. **Repeatable:** a second installation must repair or refresh the same installation without destroying user data.
5. **Local and isolated:** native ARM64 output and configuration must remain separate from upstream release paths.
6. **Smallest privilege:** only apt commands require sudo; no whole-script sudo execution.
7. **Honest support:** document tested hardware and known limitations instead of implying universal compatibility.

---

## 4. Security and Supply-Chain Enhancements

### 4.1 Pin external downloads

For ani-cli and rustup:

- Prefer distro packages for ani-cli if an official package is available.
- Otherwise pin a reviewed ani-cli release/tag or commit.
- Verify the downloaded file against a stored SHA256 checksum.
- Pin rustup distribution information or at least verify the rustup script before execution.
- Reject unexpected file types, architectures, or redirected content.

Add these functions:

```bash
download_verified()
sha256_verify()
require_http_url()
```

A checksum mismatch must stop installation and remove temporary files.

### 4.2 Use safe temporary files

- Create temporary files with `mktemp` and restrictive permissions.
- Add cleanup traps for successful and failed exits.
- Avoid executing downloaded shell scripts without first saving and validating them.

### 4.3 Reduce shell injection risk

- Continue using `printf %q` for generated shell configuration.
- Validate UUIDs using both character and length constraints.
- Verify every user path is absolute and normalized.
- Reject newline or control characters in pasted paths and configuration values.
- Quoting must be tested with spaces, apostrophes, Unicode, and `$`, backticks, and shell metacharacters.

### 4.4 Permission validation

Before any write:

- Verify the target prefix is writable by the current user.
- Verify the download directory is writable.
- Refuse to run the installer as root.
- Reject writes to `/`, `/boot`, `/etc`, `/usr`, `/bin`, `/sbin`, or `/lib` unless explicitly supported in a future dedicated installer mode.

---

## 5. Preflight and Preview Mode

Add:

```text
--dry-run
--no-sudo
--json-summary
```

The initial implementation should include `--dry-run` at minimum.

### Planned preflight checks

- OS = Linux
- Architecture = `aarch64`
- Debian/Raspberry Pi OS detected
- Memory and free disk space estimated
- Rust stable version >= 1.88
- C compiler, pkg-config, OpenSSL, mpv, ffmpeg, yt-dlp, and ani-cli availability
- Existing `solus` executable or launcher
- Existing config path and migration plan
- Download directory existence and writability
- USB UUID/mount state, if configured
- Prefix writability
- Cargo target size if a build is expected

### Preview output

Example:

```text
Raspberry Pi ARM64 installer preview
  Architecture: Linux ARM64 (aarch64)
  Prefix: ~/.local
  Terminal: Foot
  Download storage: ~/Movies/Solus
  USB requirement: none
  Existing launcher: backup before replacement
  Existing config: preserve
  Update action: build native release; self-updates remain configured by user
  Estimated disk: 4.2 GB

Run without --dry-run to apply.
```

A preview must not invoke apt, rustup, create directories, or download dependencies.

---

## 6. Install and Reinstall Lifecycle

### 6.1 Installation phases

1. Parse and validate arguments.
2. Run preflight checks.
3. Print preview.
4. Install dependencies with sudo only for package operations.
5. Install or verify rustup stable.
6. Build locally or validate a supplied binary.
7. Validate binary architecture and ELF format.
8. Create/repair directories.
9. Backup any existing launcher safely.
10. Install the native binary and launcher.
11. Update the launcher configuration atomically.
12. Preserve or minimally merge the user config.
13. Verify the completed installation.
14. Print the launch command and recovery instructions.

### 6.2 Atomic file updates

- Write launcher config to a temporary file, validate it, then `mv` atomically.
- Use staged temporary copies before replacing the executable or launcher.
- Keep timestamped backups of launchers and config files.
- Add a manifest listing installed files and versions.

### 6.3 Uninstall support

Add:

```text
--uninstall
```

Uninstall behavior:

- Remove only files created under the selected prefix.
- Remove the launcher and native binary.
- Remove the generated symlink only if it still points to this installation.
- Preserve Solus configuration, database, playlists, profile data, and downloaded media.
- Print preserved and removed paths explicitly.
- Offer `--purge-config` as a separate opt-in action.

### 6.4 Repair support

Add:

```text
--repair
--verify
```

Repair should re-check binaries, links, launcher config, and optional download directories without rebuilding unless necessary.

---

## 7. Configuration Safety

### 7.1 Preserve existing configuration

Do not overwrite an existing TOML file.

For a new installation only:

- Use a documented minimal default config.
- Set `check_on_startup = false` and `auto_install = false` for local builds.
- Keep the existing supported defaults for quality, audio, player, and update interval.

### 7.2 Merge missing fields safely

For future installer changes:

- Parse existing TOML.
- Add missing installer-specific fields only.
- Preserve all explicit user choices.
- Preserve comments? The application does not currently use a TOML library that guarantees comment preservation. The installer should create a backup before any rewrite and log every changed key.
- Never silently change update settings in an existing configuration.

### 7.3 Update-policy clarification

Separate two concepts:

1. **Local build self-update:** can be disabled by the installer because local builds may not correspond to a published release.
2. **System-packaged or binary self-update:** should remain an independent choice.

The generated config should be transparent:

```text
Local ARM64 build detected; self-updates disabled in the new config.
Existing config detected; no update preferences changed.
```

---

## 8. USB Download Hardening

### 8.1 UUID validation and mount verification

Improve launcher and installer behavior:

- Use `findmnt --verify` or equivalent validation where available.
- Verify that the UUID is a filesystem UUID, not a partition label or stale mount.
- Verify the mount is readable and writable as the current user.
- Re-check the UUID after `udisksctl mount`.
- Re-check the actual backing filesystem after resolving the nearest existing parent directory.

### 8.2 Symlink safety

Before creating or replacing `~/Movies/Solus`:

- If absent: create a symlink.
- If a symlink points to the same download directory: leave it unchanged.
- If a symlink points elsewhere: require explicit confirmation or `--replace-vault-link`.
- If a real directory exists: never replace or delete it; print recovery steps.
- Never follow symlinks for destructive operations.

### 8.3 Download behavior

- Create the directory before setting the environment variable.
- Verify free space and create only the configured directory.
- Add a marker file or mount metadata to avoid accidental use of a lookalike filesystem.
- If the USB becomes unavailable at launch, fail with a clear message and no fallback to system storage.
- Export `ANI_CLI_DOWNLOAD_DIR` only after all USB checks pass.

### 8.4 Existing download protection

- Refuse to remove or overwrite existing episode files.
- Add tests for real directories, symlinks, stale mounts, wrong UUIDs, unwritable folders, and insufficient space.

---

## 9. Binary and Build Validation

### 9.1 Supplied binary mode

Keep `--binary` for offline or prebuilt installation, but validate:

- File exists and is executable.
- ELF class is 64-bit.
- Machine type is `AArch64`.
- OS ABI is Linux.
- Dynamic dependencies are present or report a clear missing-library message.
- Optional SHA256 or `.sha256` sidecar verification, if supplied.

### 9.2 Native build mode

- Keep `--jobs` positive and default to 1.
- Preserve `Cargo.lock` with `--locked`.
- Preserve all package behavior except the local release package's `opt-level=1`.
- Detect memory pressure and recommend `CARGO_BUILD_JOBS=1`.
- Add optional `--jobs` and disk-space validation.
- Print the exact Cargo command before building.

### 9.3 Build output

- Build into a documented target directory.
- Verify the resulting executable.
- Do not install a stale binary: compare source timestamp or build fingerprint when re-running.
- Keep temporary build logs and provide a retry command on failure.

---

## 10. Terminal Integration

Keep the three choices:

- Foot
- Kitty
- Keep existing terminal

The installer should:

- Detect `WAYLAND_DISPLAY` for Foot and warn clearly in X11-only sessions.
- Verify the selected executable exists before adding it as a dependency.
- Not modify desktop files or terminal defaults.
- Continue to keep terminal behavior isolated to the launcher invocation.
- Optionally detect if the app should be started with `foot -e` or `kitty` based on the selected mode.

---

## 11. Automated ARM64 CI

Add a dedicated workflow or a workflow file in a follow-up PR.

### 11.1 Pull-request jobs

1. Shell syntax: `bash -n`.
2. Shell quality: ShellCheck.
3. Python quality: syntax compilation and regression script execution.
4. TOML/config checks.
5. ARM64 container build on Debian 13.
6. Native Rust build or binary smoke test on ARM64.
7. Installer tests using a temporary HOME.

### 11.2 Suggested matrix

| Test | Environment |
|---|---|
| Script and Python checks | Linux x86_64 runner |
| ARM64 native build | ARM64 runner or arm64 container |
| Installer regression | Temporary HOME on ARM64 |
| Deb package smoke test | Debian 13 ARM64 |
| Raspberry Pi OS smoke test | Self-hosted Pi 4 or CM5 runner |

Do not claim Raspberry Pi compatibility until at least one real Raspberry Pi OS instance performs a manual or CI smoke test.

### 11.3 Required shell checks

- ShellCheck with a pinned version.
- `bash -n` for every installer script.
- Python compile check and regression test.
- Markdown link check for the Raspberry Pi guide.
- No `git diff` changes in the upstream application or original installers.

---

## 12. Regression Test Expansion

The existing Python test should be expanded or split into named test cases.

### Installer tests

- Foot choice
- Kitty choice
- Keep choice
- Invalid terminal choice
- Invalid jobs
- Invalid prefix
- Relative download directory
- Absolute path with spaces
- Apostrophe and shell metacharacters in paths
- Existing launcher backup
- Multiple reinstalls
- Existing config preservation
- Existing config backup before merge
- Missing or corrupt launcher config
- Existing binary without build dependencies
- Native build invocation
- `--dry-run` no filesystem mutation
- `--uninstall` preservation
- `--repair` and `--verify`

### USB tests

- UUID mounted correctly
- UUID not mounted
- Wrong mounted filesystem
- Udisks mount success
- Udisks mount failure
- Download path missing
- Download path is a real directory
- Download path is a symlink
- Download path becomes unavailable
- Wrong UUID length or characters
- Existing download file remains intact
- Existing `~/Movies/Solus` real directory remains intact

### Supply-chain tests

- Valid checksum
- Missing checksum
- Invalid checksum
- Interrupted download
- Wrong architecture download
- Temporary-file cleanup

### Binary tests

- ELF 64-bit ARM64 accepted
- x86_64 rejected
- 32-bit ARM rejected
- Missing executable permission rejected
- Missing shared library produces a useful error

---

## 13. Documentation Enhancements

Update `docs/raspberry-pi.md` with:

- A supported-hardware table
- Tested OS versions
- Tested terminal/protocol combinations
- Known unsupported codecs or terminals
- Minimum/free-memory and disk requirements
- Fresh apt/rustup instructions
- Offline binary installation
- USB setup and UUID discovery
- Exact uninstall and rollback steps
- A checksum verification section
- Reinstallation and launcher recovery
- A troubleshooting matrix

Keep a clear distinction between:

- **Tested**
- **Expected to work**
- **Not tested**

No claim of universal Raspberry Pi compatibility should be included.

---

## 14. PR Review Strategy

Before merging:

1. Run the complete PR review on a clean checkout.
2. Verify only the seven intended files change.
3. Verify original application and existing installers have no diffs.
4. Run ShellCheck and syntax checks.
5. Run temporary-HOME installer tests.
6. Execute on at least one real Debian 13 ARM64 device.
7. Verify the USB guard using a real external filesystem.
8. Review all network downloads and sudo calls manually.
9. Require a maintainer to approve checksum, config, and uninstall behavior.

After merging:

1. Add ARM64 CI coverage as a separate follow-up if not included initially.
2. Publish tested hardware/OS information.
3. Add release notes for the Raspberry Pi installer.
4. Continue keeping the feature opt-in and isolated.

---

## 15. Positive Impact Summary

This enhancement would add:

- A trusted, repeatable ARM64 installation path
- Lower-risk reinstalls and upgrades
- Real USB download protection
- Stronger supply-chain verification
- Better diagnostics and rollback
- Cleaner config preservation
- More maintainable shell code
- Automated ARM64 coverage
- Honest and useful compatibility documentation

The expected result is that the PR remains additive and isolated, while becoming a safer distribution method that is practical for contributors, Raspberry Pi users, and future maintainers.

---

## 16. Implementation Order

### Phase A — Safety foundations

- Preflight validation
- `--dry-run`
- Atomic launcher/config writes
- Existing symlink/directory protection
- Installer rollback/backup improvements

### Phase B — Supply chain and lifecycle

- Download pinning and checksum verification
- `--verify`, `--repair`, and `--uninstall`
- Binary-sidecar validation
- Disk and memory checks

### Phase C — Configuration and USB

- Existing config merge rules
- Better update-policy separation
- Real mount/UUID verification
- Mount and filesystem recovery tests

### Phase D — CI and documentation

- ARM64 workflow
- Shell/Python tests
- Hardware compatibility matrix
- English/Spanish guide updates

### Phase E — Maintainer review

- Real-device smoke test
- Security review
- Support limitations documented
- Merge or follow-up issue split
