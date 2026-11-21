//! The minimal Hindsight HTTP surface the two hooks use. Everything is
//! request-scoped (per-call timeouts), so a hung server can never outlive
//! its hook budget.

use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::config::Config;
use super::state::PageTitle;

/// One knowledge-page search hit (the fallback tier's payload).
#[derive(Debug, Clone)]
pub(crate) struct PageHit {
    pub id: String,
    pub name: String,
    pub snippet: String,
}

/// One item of the retain request.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct RetainItem {
    pub content: String,
    pub context: String,
    pub document_id: String,
    pub tags: Vec<String>,
    pub strategy: String,
    pub observation_scopes: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_mode: Option<String>,
    pub metadata: serde_json::Value,
}

/// The retain envelope: `async` lets the server enqueue extraction.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct RetainRequest {
    pub items: Vec<RetainItem>,
    #[serde(rename = "async")]
    pub asynchronous: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

/// A failed retain keeps its status so the caller can fall back from
/// `append` to `replace` on a capability mismatch.
#[derive(Debug, Clone)]
pub(crate) struct RetainFailure {
    pub status: Option<u16>,
    pub message: String,
}

/// The bank-scoped client.
pub(crate) struct Client {
    http: reqwest::Client,
    base: String,
    token: Option<String>,
    bank: String,
}

impl Client {
    pub(crate) fn new(config: &Config, bank: &str) -> Client {
        // The shared TLS builder: the `rustls-no-provider` feature every
        // workspace client enables installs no process-default crypto
        // provider, so a bare `reqwest::Client::builder()` panics on first
        // use. `pa-ai`'s builder preconfigures the ring provider.
        let http = pa_ai::utils::tls::http_client_builder()
            .connect_timeout(Duration::from_secs(2))
            .build()
            .expect("the shared TLS client builds");
        Client {
            http,
            base: config.api_url.trim_end_matches('/').to_string(),
            token: config.api_token.clone(),
            bank: bank.to_string(),
        }
    }

    fn bank_url(&self, suffix: &str) -> String {
        format!(
            "{}/v1/default/banks/{}{suffix}",
            self.base,
            encode_component(&self.bank)
        )
    }

    async fn request(
        &self,
        method: reqwest::Method,
        url: String,
        body: Option<serde_json::Value>,
        timeout: Duration,
    ) -> Result<reqwest::Response> {
        let mut request = self.http.request(method, url).timeout(timeout);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        request.send().await.context("hindsight request failed")
    }

    /// One bounded reflection synthesis.
    pub(crate) async fn reflect(&self, query: &str, timeout: Duration) -> Result<String> {
        let body = serde_json::json!({ "query": query, "budget": "low" });
        let response = self
            .request(
                reqwest::Method::POST,
                self.bank_url("/reflect"),
                Some(body),
                timeout,
            )
            .await?;
        let response = ensure_success(response).await?;
        let value: ReflectResponse = response.json().await.context("reflect response")?;
        Ok(value.text.unwrap_or_default().trim().to_string())
    }

    /// The first fallback tier: knowledge-page search.
    pub(crate) async fn search_pages(
        &self,
        query: &str,
        limit: usize,
        timeout: Duration,
    ) -> Result<Vec<PageHit>> {
        let url = format!(
            "{}?q={}&limit={limit}",
            self.bank_url("/knowledge-base/search"),
            encode_component(query)
        );
        let response = self
            .request(reqwest::Method::GET, url, None, timeout)
            .await?;
        let response = ensure_success(response).await?;
        let value: SearchResponse = response.json().await.context("page search response")?;
        Ok(value
            .results
            .into_iter()
            .map(|hit| PageHit {
                id: hit.id,
                name: hit.name,
                snippet: hit.snippet.unwrap_or_default(),
            })
            .collect())
    }

    /// The second fallback tier: consolidated observations, no synthesis.
    pub(crate) async fn recall(
        &self,
        query: &str,
        max_tokens: u32,
        timeout: Duration,
    ) -> Result<Vec<String>> {
        let body = serde_json::json!({
            "query": query,
            "types": ["observation"],
            "budget": "low",
            "max_tokens": max_tokens,
            "include": { "entities": null },
        });
        let response = self
            .request(
                reqwest::Method::POST,
                self.bank_url("/memories/recall"),
                Some(body),
                timeout,
            )
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        let response = ensure_success(response).await?;
        let value: RecallResponse = response.json().await.context("recall response")?;
        Ok(value
            .results
            .into_iter()
            .filter_map(|item| item.text)
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
            .collect())
    }

    /// The roster's source: flatten the knowledge-base tree to page titles.
    pub(crate) async fn list_pages(&self, timeout: Duration) -> Result<Vec<PageTitle>> {
        let response = self
            .request(
                reqwest::Method::GET,
                self.bank_url("/knowledge-base/tree"),
                None,
                timeout,
            )
            .await?;
        let response = ensure_success(response).await?;
        let value: TreeResponse = response.json().await.context("knowledge tree response")?;
        let mut pages = Vec::new();
        for node in value.roots {
            walk_tree(&node, &mut pages);
        }
        Ok(pages)
    }

    /// Enqueue one session write-back. `update_mode` distinguishes the
    /// append path; the caller retries as replace when the server rejects
    /// it.
    pub(crate) async fn retain(
        &self,
        request_body: &RetainRequest,
        timeout: Duration,
    ) -> Result<(), RetainFailure> {
        let body = serde_json::to_value(request_body).map_err(|error| RetainFailure {
            status: None,
            message: error.to_string(),
        })?;
        let response = self
            .request(
                reqwest::Method::POST,
                self.bank_url("/memories"),
                Some(body),
                timeout,
            )
            .await
            .map_err(|error| RetainFailure {
                status: None,
                message: format!("{error:#}"),
            })?;
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let body = response.text().await.unwrap_or_default();
        Err(RetainFailure {
            status: Some(status.as_u16()),
            message: format!(
                "hindsight {status}: {}",
                super::truncate_chars(body.trim(), 500)
            ),
        })
    }
}

async fn ensure_success(response: reqwest::Response) -> Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().await.unwrap_or_default();
    anyhow::bail!(
        "hindsight {status}: {}",
        super::truncate_chars(body.trim(), 500)
    )
}

fn walk_tree(node: &TreeNode, pages: &mut Vec<PageTitle>) {
    if node.kind == "page" && !node.id.is_empty() && !node.name.is_empty() {
        pages.push(PageTitle {
            id: node.id.clone(),
            name: node.name.clone(),
        });
    }
    for child in node.children.as_deref().unwrap_or_default() {
        walk_tree(child, pages);
    }
}

/// Percent-encode one path segment or query value (the JS
/// `encodeURIComponent` shape: no `+` for spaces).
fn encode_component(value: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

#[derive(Debug, Deserialize)]
struct ReflectResponse {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    results: Vec<SearchHit>,
}

#[derive(Debug, Deserialize)]
struct SearchHit {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    snippet: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RecallResponse {
    #[serde(default)]
    results: Vec<RecallHit>,
}

#[derive(Debug, Deserialize)]
struct RecallHit {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TreeResponse {
    #[serde(default)]
    roots: Vec<TreeNode>,
}

#[derive(Debug, Deserialize)]
struct TreeNode {
    #[serde(default)]
    id: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    children: Option<Vec<TreeNode>>,
}
