use cuerithm_autotag::{AutotagConfig, ProviderId, Tagger};
#[test]
fn exposes_only_the_requested_provider_options_and_limits() {
    let options = Tagger::provider_options();
    assert_eq!(
        options
            .iter()
            .map(|option| option.id.as_str())
            .collect::<Vec<_>>(),
        [
            "beatport",
            "traxsource",
            "musicbrainz",
            "sonovault",
            "discogs",
            "volumo",
            "itunes",
            "bpmsupreme",
            "bandcamp",
            "deezer"
        ]
    );
    assert_eq!(options.len(), ProviderId::ALL.len());
    assert!(
        options
            .iter()
            .all(|option| ProviderId::parse(&option.id).is_some())
    );
    for excluded in ["beatsource", "junodownload", "spotify"] {
        assert!(ProviderId::parse(excluded).is_none());
    }
    assert_eq!(
        AutotagConfig::default().options.platforms,
        ["beatport", "traxsource"]
    );
    assert_eq!(options[0].id, "beatport");
    assert_eq!(options[1].id, "traxsource");
    assert_eq!(options[1].max_threads, 1);
    assert!(AutotagConfig::default().validate().is_ok());
    assert_eq!(ProviderId::Traxsource.as_str(), "traxsource");
}
