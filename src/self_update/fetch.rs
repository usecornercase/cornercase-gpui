use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use serde_json::Value;
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

pub(crate) const NOTES_LIMIT: u64 = 1024 * 1024;
pub(crate) const ARCHIVE_LIMIT: u64 = 256 * 1024 * 1024;

const ANSWER_LIMIT: u64 = 4 * 1024 * 1024;
const CHECK_WAIT: Duration = Duration::from_secs(20);
const DOWNLOAD_WAIT: Duration = Duration::from_secs(300);
const LIMIT_REACHED: &str = "GitHub's limit of 60 checks an hour was reached; the next check is in an hour";

pub(crate) fn secure(asked: &str) -> bool {
    !asked.starts_with("http://")
}

fn agent(wait: Duration, https_only: bool) -> Agent {
    let tls = TlsConfig::builder()
        .provider(TlsProvider::Rustls)
        .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    Agent::config_builder()
        .tls_config(tls)
        .https_only(https_only)
        .http_status_as_error(false)
        .timeout_global(Some(wait))
        .user_agent(concat!("cornercase-gpui/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

pub(crate) fn read_status(status: u16) -> Result<bool, String> {
    match status {
        200..=299 => Ok(true),
        404 => Ok(false),
        403 | 429 => Err(LIMIT_REACHED.to_string()),
        other => Err(format!("GitHub answered {other}")),
    }
}

fn unreachable(error: &ureq::Error, wait: Duration, what: &str) -> anyhow::Error {
    match error {
        ureq::Error::Timeout(_) => anyhow::anyhow!("{what} did not answer within {} s", wait.as_secs()),
        other => anyhow::anyhow!("could not reach {what}: {other}"),
    }
}

pub(crate) fn latest(url: &str) -> Result<Option<Value>> {
    let request = agent(CHECK_WAIT, secure(url)).get(url).header("Accept", "application/vnd.github+json");
    let mut response = request.call().map_err(|e| unreachable(&e, CHECK_WAIT, "GitHub"))?;
    if !read_status(response.status().as_u16()).map_err(anyhow::Error::msg)? {
        return Ok(None);
    }
    let body =
        response.body_mut().with_config().limit(ANSWER_LIMIT).read_to_vec().context("reading GitHub's answer")?;
    Ok(Some(serde_json::from_slice(&body).context("GitHub's answer is not JSON")?))
}

pub(crate) fn download(url: &str, limit: u64, https_only: bool) -> Result<Vec<u8>> {
    let mut response =
        agent(DOWNLOAD_WAIT, https_only).get(url).call().map_err(|e| unreachable(&e, DOWNLOAD_WAIT, "the download"))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        bail!("the download answered {status}");
    }
    response
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .with_context(|| format!("the download failed or is larger than {} MiB", limit / (1024 * 1024)))
}

#[cfg(test)]
mod tests;
