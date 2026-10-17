use sha2::{Digest, Sha256};
use std::{fs, path::Path};

fn collect(dir: &Path, paths: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).expect("read source directory") {
        let path = entry.expect("read source entry").path();
        if path.is_dir() {
            collect(&path, paths);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            paths.push(path);
        }
    }
}

fn main() {
    let root = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let mut paths = vec![root.join("Cargo.toml"), root.join("build.rs")];
    collect(&root.join("src"), &mut paths);
    paths.sort();
    let mut hash = Sha256::new();
    for path in paths {
        let name = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(&path).expect("read runtime source");
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-changed=src");
    println!(
        "cargo:rustc-env=CAMELOT_SOURCE_ANALYSIS_SHA256={:x}",
        hash.finalize()
    );
}
