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
        } else if entry.path().extension().is_some_and(|e| e == "rs") {
            files.push(entry.path());
        }
    }
    Ok(())
}
fn main() -> std::io::Result<()> {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let mut files = vec![root.join("build.rs"), root.join("../../Cargo.lock")];
    for directory in ["src", "../cuerithm-smartlists/src", "../cuerithm-media/src"] {
        let path = root.join(directory);
        println!("cargo:rerun-if-changed={}", path.display());
        sources(&path, &mut files)?;
    }
    files.sort();
    let mut hash = Sha256::new();
    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        let name = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(path)?;
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    println!(
        "cargo:rustc-env=CUERITHM_DJ_SYNC_IMPLEMENTATION={:x}",
        hash.finalize()
    );
    Ok(())
}
