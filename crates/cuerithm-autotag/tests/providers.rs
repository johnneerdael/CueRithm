use cuerithm_autotag::{AutotagConfig, ProviderId, Tagger};
#[test]
fn exposes_only_the_requested_provider_options_and_limits() {
    let options = Tagger::provider_options();
    assert_eq!(options.len(), 2);
    assert_eq!(options[0].id, "beatport");
    assert_eq!(options[1].id, "traxsource");
    assert_eq!(options[1].max_threads, 1);
    assert!(AutotagConfig::default().validate().is_ok());
    assert_eq!(ProviderId::Traxsource.as_str(), "traxsource");
}
