use cuerithm_library::backup::{LogicalArchive, LogicalRecord, SpoolArchive};

#[test]
fn streaming_iteration_can_use_cached_indexed_lookups_without_holding_their_mutex() {
    let archive = LogicalArchive::empty("UTC").unwrap();
    let spool = SpoolArchive::create(archive.manifest).unwrap();
    for i in 0..100 {
        spool
            .insert(
                "roots",
                &LogicalRecord::new(format!("root-{i:03}"), serde_json::json!({"test":i})),
            )
            .unwrap();
    }
    let mut seen = 0;
    spool
        .for_each("roots", &mut |record| {
            assert_eq!(spool.record("roots", &record.id).unwrap(), Some(record));
            assert_eq!(spool.count("roots").unwrap(), 100);
            seen += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(seen, 100);
    let shared = spool.clone();
    std::thread::spawn(move || assert!(shared.record("roots", "root-099").unwrap().is_some()))
        .join()
        .unwrap();
}
