use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    watch_git_head_refs();

    let git_sha = std::env::var("GITHUB_SHA")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| git_output(&["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());

    let commit_ts = git_output(&["show", "-s", "--format=%ct", "HEAD"])
        .and_then(|s| s.parse::<i64>().ok())
        .map(|ts| ts.to_string())
        .unwrap_or_else(|| "0".to_string());

    println!("cargo:rustc-env=SOLUS_GIT_SHA={}", git_sha.trim());
    println!("cargo:rustc-env=SOLUS_COMMIT_TS={}", commit_ts);

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_resources();
    }
}

fn embed_windows_resources() {
    let Ok(out_dir_str) = std::env::var("OUT_DIR") else {
        return;
    };
    let out_dir = PathBuf::from(out_dir_str);
    let pkg_ver = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "1.0.7".to_string());
    let mut ver_parts: Vec<u16> = pkg_ver
        .split('.')
        .filter_map(|p| {
            let digits: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse::<u16>().ok()
        })
        .take(4)
        .collect();
    while ver_parts.len() < 4 {
        ver_parts.push(0);
    }
    let ver_commas = format!(
        "{},{},{},{}",
        ver_parts[0], ver_parts[1], ver_parts[2], ver_parts[3]
    );

    let manifest = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" processorArchitecture="*" name="Solus.TerminalAnimeTheatre" type="win32"/>
  <description>Solus — Terminal Anime Theatre &amp; AniList/MAL Companion</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#;

    let rc = format!(
        r#"1 24 "solus.manifest"

1 VERSIONINFO
FILEVERSION {ver_commas}
PRODUCTVERSION {ver_commas}
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "Solus Open Source Project\0"
      VALUE "FileDescription", "Solus - Terminal Anime Theatre & AniList/MAL Companion\0"
      VALUE "FileVersion", "{pkg_ver}\0"
      VALUE "InternalName", "solus\0"
      VALUE "LegalCopyright", "Copyright (c) Abdullah Binmadhi & Contributors (MIT License)\0"
      VALUE "OriginalFilename", "solus.exe\0"
      VALUE "ProductName", "Solus\0"
      VALUE "ProductVersion", "{pkg_ver}\0"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    );

    let manifest_path = out_dir.join("solus.manifest");
    let rc_path = out_dir.join("solus.rc");
    if std::fs::write(&manifest_path, manifest).is_err() || std::fs::write(&rc_path, rc).is_err() {
        return;
    }

    let is_gnu = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu");
    let res_path = out_dir.join(if is_gnu { "solus_res.o" } else { "solus.res" });

    if compile_windows_rc(&out_dir, &rc_path, &res_path, is_gnu) && res_path.exists() {
        println!("cargo:rustc-link-arg-bins={}", res_path.display());
    }
}

fn compile_windows_rc(out_dir: &Path, rc_path: &Path, res_path: &Path, is_gnu: bool) -> bool {
    if is_gnu {
        for tool in ["x86_64-w64-mingw32-windres", "windres"] {
            if let Ok(st) = Command::new(tool)
                .current_dir(out_dir)
                .args(["-O", "coff", "-i"])
                .arg(rc_path)
                .arg("-o")
                .arg(res_path)
                .status()
            {
                if st.success() {
                    return true;
                }
            }
        }
        return false;
    }

    // Try rc.exe on PATH or inside Windows Kits 10, then llvm-rc
    let mut rc_candidates: Vec<PathBuf> = vec![PathBuf::from("rc.exe"), PathBuf::from("rc")];
    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        let kits_bin = Path::new(&pf86).join("Windows Kits").join("10").join("bin");
        if let Ok(entries) = std::fs::read_dir(kits_bin) {
            let mut versions: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path().join("x64").join("rc.exe"))
                .filter(|p| p.is_file())
                .collect();
            versions.sort();
            versions.reverse();
            rc_candidates.extend(versions);
        }
    }

    for rc_bin in rc_candidates {
        if let Ok(st) = Command::new(&rc_bin)
            .current_dir(out_dir)
            .arg("/nologo")
            .arg("/fo")
            .arg(res_path)
            .arg(rc_path)
            .status()
        {
            if st.success() {
                return true;
            }
        }
    }

    if let Ok(st) = Command::new("llvm-rc")
        .current_dir(out_dir)
        .arg("/fo")
        .arg(res_path)
        .arg(rc_path)
        .status()
    {
        if st.success() {
            return true;
        }
    }

    false
}

/// Watch both `.git/HEAD` and its active branch ref file so local commits always rebake `SOLUS_GIT_SHA`.
fn watch_git_head_refs() {
    let git_dir = git_output(&["rev-parse", "--git-dir"])
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".git"));

    let head_path = git_dir.join("HEAD");
    if head_path.exists() {
        println!("cargo:rerun-if-changed={}", head_path.display());
        if let Ok(head_contents) = std::fs::read_to_string(&head_path) {
            if let Some(rel_ref) = head_contents.trim().strip_prefix("ref:") {
                let ref_path = git_dir.join(rel_ref.trim());
                if ref_path.exists() {
                    println!("cargo:rerun-if-changed={}", ref_path.display());
                }
            }
        }
    }

    let packed_refs = Path::new(&git_dir).join("packed-refs");
    if packed_refs.exists() {
        println!("cargo:rerun-if-changed={}", packed_refs.display());
    }
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
