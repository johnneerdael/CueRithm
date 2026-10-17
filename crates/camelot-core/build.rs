use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn collect(root: &Path, files: &mut Vec<PathBuf>) {
    println!("cargo:rerun-if-changed={}", root.display());
    for entry in fs::read_dir(root).expect("read vendored native source") {
        let path = entry.expect("read native source entry").path();
        if path.is_dir() {
            collect(&path, files);
        } else {
            files.push(path);
        }
    }
}

fn main() {
    let mut files = Vec::new();
    collect(Path::new("native/openkeyscan"), &mut files);
    collect(Path::new("vendor/soxr"), &mut files);
    files.sort();
    let mut hash = Sha256::new();
    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        let name = path
            .to_str()
            .expect("UTF-8 native source path")
            .replace('\\', "/");
        let bytes = fs::read(&path).expect("read native source");
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    println!(
        "cargo:rustc-env=CAMELOT_OPENKEYSCAN_NATIVE_SHA256={:x}",
        hash.finalize()
    );
    #[cfg(feature = "openkeyscan-key")]
    {
        let dst = cmake::Config::new("native/openkeyscan")
            .profile("Release")
            .pic(true)
            .build();
        println!("cargo:rustc-link-search=native={}/lib", dst.display());
        println!("cargo:rustc-link-lib=static=camelot_soxr_half");
        println!("cargo:rustc-link-lib=static=soxr");
        if std::env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("unix") {
            println!("cargo:rustc-link-lib=m");
        }
    }
}
