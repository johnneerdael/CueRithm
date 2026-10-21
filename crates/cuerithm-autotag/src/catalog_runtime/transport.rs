use super::*;
pub(super) struct HttpTransport;
impl CatalogTransport for HttpTransport {
    fn execute(
        &self,
        request: CatalogHttpRequest,
        cancel: &Arc<AtomicBool>,
        revocation: Option<&Revocation>,
    ) -> Result<CatalogHttpResponse> {
        // Joining a scoped driver also supports callers already inside another Tokio runtime.
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()?
                        .block_on(send(request, cancel, revocation))
                })
                .join()
                .map_err(|_| anyhow::anyhow!("Catalog transport interrupted"))?
        })
    }
}
async fn send(
    r: CatalogHttpRequest,
    cancel: &Arc<AtomicBool>,
    revocation: Option<&Revocation>,
) -> Result<CatalogHttpResponse> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_millis(r.timeout_ms))
        .build()?;
    let mut request = client.request(
        if r.method == CatalogMethod::Get {
            reqwest::Method::GET
        } else {
            reqwest::Method::POST
        },
        &r.url,
    );
    for (key, value) in &r.headers {
        request = request.header(key, value);
    }
    let operation = async {
        let mut response = request.body(r.body).send().await?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(key, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (key.as_str().into(), value.into()))
            })
            .collect();
        if response
            .content_length()
            .is_some_and(|n| n > r.max_bytes as u64)
        {
            return Err(CatalogError::new(
                r.service,
                CatalogErrorKind::TooLarge,
                Some(status),
                None,
            )
            .into());
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if body.len().saturating_add(chunk.len()) > r.max_bytes {
                return Err(CatalogError::new(
                    r.service,
                    CatalogErrorKind::TooLarge,
                    Some(status),
                    None,
                )
                .into());
            }
            body.extend_from_slice(&chunk);
        }
        Ok(CatalogHttpResponse {
            status,
            headers,
            body,
        })
    };
    tokio::pin!(operation);
    loop {
        tokio::select! {
            result = &mut operation => return result,
            _ = tokio::time::sleep(std::time::Duration::from_millis(25)) => {
                if cancel.load(Ordering::Relaxed) { return Err(CatalogError::new(r.service,CatalogErrorKind::Cancelled,None,None).into()); }
                if revocation.is_some_and(Revocation::is_revoked) { return Err(CatalogError::new(r.service,CatalogErrorKind::CredentialRevoked,None,None).into()); }
            }
        }
    }
}
