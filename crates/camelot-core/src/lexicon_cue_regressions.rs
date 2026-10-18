use super::{RampType, Section, find_earlier_drop};
#[test]
fn original_earlier_drop_requires_a_quieter_preceding_window() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/lexicon-cue-frontend/earlier-drop-reference.json"
    ))
    .unwrap();
    let mut consumed = 0;
    for row in reference["cases"].as_array().unwrap() {
        let sections: Vec<Section> = row["energies"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, energy)| Section {
                energy: energy.as_f64().unwrap(),
                beat_strength: 0.,
                ramp_type: RampType::Flat,
                beat_energies: vec![],
                start_time: i as f64,
                duration: 1.,
                energy_delta: 0.,
            })
            .collect();
        assert_eq!(
            find_earlier_drop(&sections, row["before"].as_u64().unwrap() as usize),
            row["expected"].as_u64().map(|v| v as usize),
            "{}",
            row["name"]
        );
        consumed += 1;
    }
    assert_eq!(consumed, 7);
}
