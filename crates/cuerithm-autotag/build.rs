use sha2::{Digest, Sha256};
use std::{fs::File, path::PathBuf};
fn main() {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let vendor = root.join("../../vendor/taglib");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(vendor.join("manifest.json")).unwrap()).unwrap();
    let archive = vendor.join(manifest["archive"].as_str().unwrap());
    println!("cargo:rerun-if-changed={}", archive.display());
    println!(
        "cargo:rerun-if-changed={}",
        vendor.join("manifest.json").display()
    );
    println!("cargo:rerun-if-changed=native/asf.cpp");
    assert_eq!(
        format!("{:x}", Sha256::digest(std::fs::read(&archive).unwrap())),
        manifest["sha256"].as_str().unwrap(),
        "TagLib source checksum mismatch"
    );
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let source = out.join(format!("taglib-{}", manifest["version"].as_str().unwrap()));
    if !source.is_dir() {
        tar::Archive::new(flate2::read::GzDecoder::new(File::open(archive).unwrap()))
            .unpack(&out)
            .unwrap();
    }
    let mut config = cmake::Config::new(&source);
    config.profile("Release");
    let msvc = std::env::var("TARGET").unwrap().contains("msvc");
    let static_crt = std::env::var("CARGO_CFG_TARGET_FEATURE")
        .unwrap_or_default()
        .split(',')
        .any(|feature| feature == "crt-static");
    if msvc {
        // CMP0091 selects the CRT through a target property; TagLib's legacy
        // flag replacement alone does not affect modern CMake generators.
        config
            .static_crt(static_crt)
            .define(
                "CMAKE_MSVC_RUNTIME_LIBRARY",
                if static_crt {
                    "MultiThreaded"
                } else {
                    "MultiThreadedDLL"
                },
            )
            .define(
                "ENABLE_STATIC_RUNTIME",
                if static_crt { "ON" } else { "OFF" },
            );
    }
    for flag in [
        "BUILD_SHARED_LIBS",
        "BUILD_TESTING",
        "BUILD_BINDINGS",
        "BUILD_EXAMPLES",
        "WITH_ZLIB",
        "WITH_APE",
        "WITH_DSF",
        "WITH_MATROSKA",
        "WITH_MOD",
        "WITH_MP4",
        "WITH_RIFF",
        "WITH_SHORTEN",
        "WITH_TRUEAUDIO",
        "WITH_VORBIS",
    ] {
        config.define(flag, "OFF");
    }
    config
        .define("WITH_ASF", "ON")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .define("CMAKE_DISABLE_FIND_PACKAGE_utf8cpp", "ON")
        .define("utf8cpp_INCLUDE_DIR", source.join("3rdparty/utfcpp/source"));
    let dest = config.build();
    let mut wrapper = cc::Build::new();
    wrapper
        .cpp(true)
        .std("c++17")
        .opt_level(2)
        .define("TAGLIB_STATIC", None)
        .include(dest.join("include/taglib"))
        .file(root.join("native/asf.cpp"));
    if msvc {
        wrapper.static_crt(static_crt);
    }
    wrapper.compile("cuerithm_asf");
    println!(
        "cargo:rustc-link-search=native={}",
        dest.join("lib").display()
    );
    println!("cargo:rustc-link-lib=static=tag");
}
