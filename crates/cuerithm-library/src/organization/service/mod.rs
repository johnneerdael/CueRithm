#[cfg(test)]
mod batch_tests;
mod batches;
mod capture;
mod checkpoints;
mod examples;
mod inventory;
mod model;
mod planning;
mod roots;
mod storage;
mod workflows;
use crate::Library;
pub use model::*;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_example_keeps_an_accepted_mix_title_once() {
        let temp = tempfile::tempdir().unwrap();
        let library = Library::open(&temp.path().join("library.sqlite")).unwrap();
        let example = library
            .organization_example(OrganizationSettings::default())
            .unwrap();
        assert!(example.valid);
        assert_eq!(
            example.examples[0].result.as_deref(),
            Some("Example Artist - Example (Club Mix).flac")
        );
    }
    #[test]
    fn rename_settings_and_example_refuse_folder_grammar() {
        let temp = tempfile::tempdir().unwrap();
        let library = Library::open(&temp.path().join("library.sqlite")).unwrap();
        let settings = OrganizationSettings {
            mode: OrganizationSettingMode::RenameInPlace,
            pattern: "{artist}/{title}".into(),
            ..Default::default()
        };
        assert!(
            settings
                .validate()
                .unwrap_err()
                .to_string()
                .contains("cannot create folders")
        );
        let example = library.organization_example(settings).unwrap();
        assert!(!example.valid);
        assert!(example.error.unwrap().contains("cannot create folders"));
    }
    #[test]
    fn native_root_registration_preserves_authority_without_mutating_source_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let library = Library::open(&root.join("data/library.sqlite")).unwrap();
        let destination = root.join("managed");
        std::fs::create_dir(&destination).unwrap();
        let registered = library.register_organization_root(destination);
        assert!(
            registered.is_ok(),
            "Expected persisted native root grant: {registered:?}"
        );
    }
}
#[cfg(test)]
mod planning_tests;
