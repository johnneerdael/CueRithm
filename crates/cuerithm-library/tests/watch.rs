use cuerithm_library::watch::{supported_path, wait_for_stable};
use std::{sync::atomic::AtomicBool, time::Duration};
#[test]
fn watches_common_extensions_and_waits_for_a_copy_to_settle() {
    for name in [
        "x.MP3", "x.aac", "x.m4a", "x.flac", "x.wav", "x.wave", "x.aif", "x.aiff", "x.aifc",
        "x.ogg", "x.oga", "x.opus", "x.wma",
    ] {
        assert!(supported_path(std::path::Path::new(name)), "{name}");
    }
    assert!(!supported_path(std::path::Path::new("._x.mp3")));
    assert!(!supported_path(std::path::Path::new("cover.jpg")));
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("copy.mp3");
    std::fs::write(&p, b"first").unwrap();
    let path = p.clone();
    let thread = std::thread::spawn(move || {
        for i in 0..5 {
            std::fs::write(&path, vec![1; i + 2]).unwrap();
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    wait_for_stable(&p, Duration::from_millis(20), &AtomicBool::new(false)).unwrap();
    thread.join().unwrap();
    assert_eq!(std::fs::metadata(p).unwrap().len(), 6);
}
