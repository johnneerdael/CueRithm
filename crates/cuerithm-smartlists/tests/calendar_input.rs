use cuerithm_smartlists::calendar_day_epoch;
#[test]
fn calendar_input_uses_the_selected_zone_and_source_gap_policy() {
    for (day, zone, utc) in [
        ("2026-10-18", "America/Los_Angeles", "2026-10-18T07:00:00Z"),
        ("2026-10-18", "Pacific/Kiritimati", "2026-10-17T10:00:00Z"),
        ("2026-10-25", "Europe/Amsterdam", "2026-10-24T22:00:00Z"),
        ("2011-12-30", "Pacific/Apia", "2011-12-30T10:00:00Z"),
    ] {
        let expected = chrono::DateTime::parse_from_rfc3339(utc)
            .unwrap()
            .timestamp_millis();
        assert_eq!(
            calendar_day_epoch(day, zone).unwrap(),
            expected,
            "{day}/{zone}"
        );
    }
    for day in ["2026-02-30", "not-a-date", "2026-1-2"] {
        assert!(calendar_day_epoch(day, "UTC").is_err());
    }
    assert!(calendar_day_epoch("2026-10-18", "not-a-zone").is_err());
}
