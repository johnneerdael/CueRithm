use crate::{
    commands::{AppError, work},
    host::Host,
    metadata_commands,
};
use cuerithm_library::{
    ProviderCredentialId, ProviderCredentialInput, ProviderCredentialRef, ProviderCredentialStatus,
    ProviderSetupStatus,
};
use std::sync::Arc;
use tauri::{Emitter, State};
use tauri_plugin_opener::OpenerExt;
type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;

fn catalog_source(provider: &str, url: &str) -> anyhow::Result<String> {
    cuerithm_autotag::ProviderId::parse(provider)
        .and_then(|provider| cuerithm_autotag::review_catalog_url(provider, url, false))
        .ok_or_else(|| anyhow::anyhow!("This catalog source page is not allowed"))
}

#[tauri::command]
pub async fn open_catalog_source(app: tauri::AppHandle, provider: String, url: String) -> Ipc<()> {
    let url = catalog_source(&provider, &url)?;
    work(move || {
        app.opener()
            .open_url(url, None::<&str>)
            .map_err(|_| anyhow::anyhow!("The catalog source page could not be opened"))
    })
    .await
}
fn changed(app: &tauri::AppHandle) {
    let _ = app.emit("catalog-access-change", ());
}
#[tauri::command]
pub async fn provider_setup_statuses(host: Managed<'_>) -> Ipc<Vec<ProviderSetupStatus>> {
    let service = host.provider_setup.clone();
    work(move || service.statuses()).await
}
#[tauri::command]
pub async fn save_provider_credential(
    host: Managed<'_>,
    app: tauri::AppHandle,
    expected: ProviderCredentialRef,
    input: ProviderCredentialInput,
) -> Ipc<ProviderCredentialStatus> {
    let service = host.provider_setup.clone();
    let result = work(move || service.save(&expected, input)).await;
    changed(&app);
    result
}
#[tauri::command]
pub async fn remove_provider_credential(
    host: Managed<'_>,
    app: tauri::AppHandle,
    provider: ProviderCredentialId,
) -> Ipc<ProviderCredentialStatus> {
    let service = host.provider_setup.clone();
    let result = work(move || service.remove(provider)).await;
    changed(&app);
    result
}
#[tauri::command]
pub async fn test_provider_credential(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    expected: ProviderCredentialRef,
) -> Ipc<ProviderSetupStatus> {
    let result = metadata_commands::run(host.inner().clone(), operation_id, move |host, cancel| {
        host.provider_setup.test(&expected, cancel)
    })
    .await;
    changed(&app);
    result
}

#[cfg(test)]
mod source_link_tests {
    use super::*;

    #[test]
    fn catalog_sources_use_existing_provider_policy_before_opening() {
        for (provider, url) in [
            ("beatport", "https://www.beatport.com/track/example/11"),
            ("bandcamp", "https://artist.bandcamp.com/track/example"),
            ("volumo", "https://volumo.com/track/123-example"),
            (
                "musicbrainz",
                "https://musicbrainz.org/recording/11111111-1111-4111-8111-111111111111",
            ),
        ] {
            assert_eq!(catalog_source(provider, url).unwrap(), url);
        }
        for (provider, url) in [
            ("beatport", "https://foreign.invalid/track/example/11"),
            (
                "bandcamp",
                "https://foreign.invalid/artist.bandcamp.com/track/example",
            ),
            (
                "bandcamp",
                "https://artist.bandcamp.com.foreign.invalid/track/example",
            ),
            ("volumo", "http://volumo.com/track/123"),
            ("volumo", "https://volumo.com:444/track/123"),
            ("volumo", "https://user:private@volumo.com/track/123"),
            ("volumo", "https://127.0.0.1/track/123"),
            ("volumo", "file:///private/source.flac"),
            ("volumo", "javascript:alert(1)"),
            ("beatsource", "https://www.beatsource.com/track/example/11"),
            (
                "junodownload",
                "https://www.junodownload.com/products/example/11",
            ),
            ("spotify", "https://open.spotify.com/track/example"),
        ] {
            let error = catalog_source(provider, url).unwrap_err().to_string();
            assert_eq!(error, "This catalog source page is not allowed");
            assert!(!error.contains(url));
        }
    }

    #[test]
    fn automatic_opener_permission_contains_only_two_exact_signup_urls() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/main.json")).unwrap();
        let permissions = capability["permissions"].as_array().unwrap();
        let opener = permissions
            .iter()
            .filter(|permission| {
                permission
                    .as_str()
                    .is_some_and(|name| name.starts_with("opener:"))
                    || permission["identifier"]
                        .as_str()
                        .is_some_and(|name| name.starts_with("opener:"))
            })
            .collect::<Vec<_>>();
        assert_eq!(opener.len(), 1);
        assert_eq!(opener[0]["identifier"], "opener:allow-open-url");
        assert_eq!(
            opener[0]["allow"],
            serde_json::json!([
                {"url":"https://sonovault.now/signup"},
                {"url":"https://volumo.com/profile/api-tokens"}
            ])
        );
    }
}
