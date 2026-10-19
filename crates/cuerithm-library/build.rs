use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn sources(path: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            sources(&entry.path(), files)?;
        } else if entry
            .path()
            .extension()
            .is_some_and(|e| matches!(e.to_str(), Some("rs" | "cpp" | "h" | "hpp")))
        {
            files.push(entry.path());
        }
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let mut files = vec![root.join("build.rs"), root.join("../../Cargo.lock")];
    for folder in [
        "src/playlists",
        "../cuerithm-smartlists/src",
        "../cuerithm-regex/src",
        "../../vendor/regress/src",
        "../cuerithm-media/src",
        "../cuerithm-autotag/src",
        "../cuerithm-autotag/native",
        "../../vendor/onetagger/onetagger-tag/src",
        "../cuerithm-cleanup/src",
    ] {
        // Watch directories too, so adding a module invalidates the fingerprint.
        let path = root.join(folder);
        println!("cargo:rerun-if-changed={}", path.display());
        sources(&path, &mut files)?;
    }
    files.sort();
    let mut hash = Sha256::new();
    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        let bytes = fs::read(&path)?;
        let name = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    println!(
        "cargo:rustc-env=CUERITHM_SMART_ENGINE_ID={:x}",
        hash.finalize()
    );
    Ok(())
}
