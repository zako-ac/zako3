use anyhow::{Context, Result};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};

pub struct HqClient {
    client: HttpClient,
}

impl HqClient {
    pub fn new(url: &str, admin_token: Option<&str>) -> Result<Self> {
        let mut builder = HttpClientBuilder::default();
        if let Some(token) = admin_token {
            builder = builder.set_headers(http::HeaderMap::from_iter(vec![(
                http::HeaderName::from_static("x-admin-token"),
                http::HeaderValue::from_str(token)?,
            )]));
        }
        let client = builder
            .build(url)
            .context("Failed to build HQ RPC client")?;
        Ok(Self { client })
    }

    pub fn inner(&self) -> &HttpClient {
        &self.client
    }
}
