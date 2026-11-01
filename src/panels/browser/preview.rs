use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};
use base64::Engine as _;
use futures::future::{self, Either};
use gpui::{AsyncApp, Context, Image, ImageFormat, Task, WeakEntity};
use serde_json::{Value, json};

use super::cdp::Cdp;
use super::tunnels::{loopback_port, rewrite};
use super::{Browser, Status};
use crate::cli;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;
const STEP: Duration = Duration::from_millis(150);
const STARTING: Duration = Duration::from_secs(20);
const TUNNEL: Duration = Duration::from_secs(15);
const ANSWER: Duration = Duration::from_secs(5);
const LOADING: Duration = Duration::from_secs(8);
const SETTLE: Duration = Duration::from_millis(500);
const QUIET: &str = "window.alert = () => {}; window.confirm = () => false; window.prompt = () => null; \
                     window.print = () => {};";

type Call<'a> = (Option<&'a str>, &'a str, Value);

#[derive(Debug)]
struct Late(String);

impl std::fmt::Display for Late {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the browser did not answer {}", self.0)
    }
}

impl std::error::Error for Late {}

fn unanswered(error: anyhow::Error) -> anyhow::Error {
    if error.is::<Late>() {
        return anyhow!("The page did not answer within {} seconds.", LOADING.as_secs());
    }
    error
}

pub(crate) struct Preview {
    pub image: Arc<Image>,
    pub title: String,
}

#[derive(Default)]
pub(crate) struct Previews {
    contexts: Vec<String>,
}

impl Previews {
    pub(crate) fn owns(&self, info: &Value) -> bool {
        info["browserContextId"].as_str().is_some_and(|id| self.contexts.iter().any(|c| c == id))
    }
}

struct Job {
    cdp: Cdp,
    context: String,
    url: String,
    agent: Option<String>,
}

pub(crate) fn failure(error: &str) -> String {
    let code = error.trim().trim_start_matches("net::ERR_");
    match code {
        "CONNECTION_REFUSED" => "Nothing answers at this address.".into(),
        "NAME_NOT_RESOLVED" => "No server has this name.".into(),
        "INTERNET_DISCONNECTED" => "This Mac is offline.".into(),
        "ABORTED" => "The page did not load: it may be a download, which a preview never takes.".into(),
        _ if code.starts_with("CERT_") => "Its certificate is not valid.".into(),
        _ => format!("The page did not load: {}.", code.replace('_', " ").to_lowercase()),
    }
}

async fn pause(cx: &AsyncApp, wait: Duration) {
    cx.background_executor().timer(wait).await;
}

async fn answer_within(
    cdp: &Cdp,
    (session, method, params): Call<'_>,
    limit: Duration,
    cx: &AsyncApp,
) -> Result<Value> {
    let reply = cdp.call(session, method, params);
    match future::select(reply, cx.background_executor().timer(limit)).await {
        Either::Left((reply, _)) => reply.map_err(|_| anyhow!("the browser went away"))?,
        Either::Right(_) => Err(Late(method.to_string()).into()),
    }
}

async fn answer(cdp: &Cdp, call: Call<'_>, cx: &AsyncApp) -> Result<Value> {
    answer_within(cdp, call, ANSWER, cx).await
}

async fn evaluate(cdp: &Cdp, session: Option<&str>, expression: &str, cx: &AsyncApp) -> Result<String> {
    let params = json!({ "expression": expression, "returnByValue": true });
    let value = answer(cdp, (session, "Runtime.evaluate", params), cx).await?;
    Ok(value["result"]["value"].as_str().unwrap_or_default().to_string())
}

async fn engine(this: &WeakEntity<Browser>, cx: &mut AsyncApp) -> Result<Cdp> {
    let since = Instant::now();
    loop {
        let state = this.read_with(cx, |b, _| match &b.status {
            Status::Ready(engine) => Ok(Some(engine.cdp.clone())),
            Status::Starting => Ok(None),
            Status::Failed(why) => Err(why.clone()),
        })?;
        match state.map_err(|why| anyhow!("the Browser panel's Chrome is not running: {why}"))? {
            Some(cdp) => return Ok(cdp),
            None if since.elapsed() > STARTING => bail!("the Browser panel's Chrome did not start"),
            None => pause(cx, STEP).await,
        }
    }
}

async fn context(this: &WeakEntity<Browser>, cdp: &Cdp, cx: &mut AsyncApp) -> Result<String> {
    if let Some(id) = this.read_with(cx, |b, _| b.previews.contexts.first().cloned())? {
        return Ok(id);
    }
    let made = answer(cdp, (None, "Target.createBrowserContext", json!({ "disposeOnDetach": true })), cx).await?;
    let id = made["browserContextId"].as_str().context("Chrome made no context for previews")?.to_string();
    cdp.send(None, "Browser.setDownloadBehavior", json!({ "behavior": "deny", "browserContextId": id }));
    this.update(cx, |b, _| b.previews.contexts.push(id.clone()))?;
    Ok(id)
}

async fn reachable(this: &WeakEntity<Browser>, url: String, cx: &mut AsyncApp) -> Result<String> {
    let Some(remote) = loopback_port(&url).filter(|_| cli::host().is_some()) else { return Ok(url) };
    let since = Instant::now();
    loop {
        if let Some(local) = this.update(cx, |b, cx| b.tunnel_to(remote, cx))? {
            return Ok(rewrite(&url, local));
        }
        if since.elapsed() > TUNNEL {
            bail!("No tunnel reached port {remote} on that machine.");
        }
        pause(cx, STEP).await;
    }
}

async fn settle(cdp: &Cdp, session: Option<&str>, cx: &AsyncApp) {
    let since = Instant::now();
    while since.elapsed() < LOADING {
        if evaluate(cdp, session, "document.readyState", cx).await.is_ok_and(|state| state == "complete") {
            break;
        }
        pause(cx, STEP).await;
    }
    pause(cx, SETTLE).await;
}

async fn set_up(job: &Job, session: Option<&str>, cx: &AsyncApp) -> Result<()> {
    let metrics = json!({ "width": WIDTH, "height": HEIGHT, "deviceScaleFactor": 1, "mobile": false });
    answer(&job.cdp, (session, "Emulation.setDeviceMetricsOverride", metrics), cx).await?;
    if let Some(agent) = &job.agent {
        job.cdp.send(session, "Emulation.setUserAgentOverride", json!({ "userAgent": agent }));
    }
    answer(&job.cdp, (session, "Page.addScriptToEvaluateOnNewDocument", json!({ "source": QUIET })), cx).await?;
    Ok(())
}

async fn shoot(job: &Job, target: &str, cx: &AsyncApp) -> Result<Preview> {
    let attach = json!({ "targetId": target, "flatten": true });
    let attached = answer(&job.cdp, (None, "Target.attachToTarget", attach), cx).await?;
    let session = Some(attached["sessionId"].as_str().context("Chrome gave the preview no session")?);
    set_up(job, session, cx).await?;
    let navigate = (session, "Page.navigate", json!({ "url": job.url }));
    let went = answer_within(&job.cdp, navigate, LOADING, cx).await.map_err(unanswered)?;
    if let Some(error) = went["errorText"].as_str().filter(|e| !e.is_empty()) {
        bail!(failure(error));
    }
    settle(&job.cdp, session, cx).await;
    let title = evaluate(&job.cdp, session, "document.title", cx).await.unwrap_or_default();
    let shot =
        answer(&job.cdp, (session, "Page.captureScreenshot", json!({ "format": "jpeg", "quality": 85 })), cx).await?;
    let data = shot["data"].as_str().context("Chrome took no picture of the page")?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;
    Ok(Preview { image: Arc::new(Image::from_bytes(ImageFormat::Jpeg, bytes)), title: title.trim().to_string() })
}

async fn capture(job: Job, cx: &AsyncApp) -> Result<Preview> {
    let open = json!({ "url": "about:blank", "browserContextId": job.context });
    let made = answer(&job.cdp, (None, "Target.createTarget", open), cx).await?;
    let target = made["targetId"].as_str().context("Chrome opened no page for the preview")?.to_string();
    let shot = shoot(&job, &target, cx).await;
    job.cdp.send(None, "Target.closeTarget", json!({ "targetId": target }));
    shot
}

impl Browser {
    pub(crate) fn preview(&mut self, url: String, cx: &mut Context<Self>) -> Task<Result<Preview>> {
        let agent = self.user_agent.clone();
        cx.spawn(async move |this, cx| {
            let cdp = engine(&this, cx).await?;
            let context = context(&this, &cdp, cx).await?;
            let url = reachable(&this, url, cx).await?;
            capture(Job { cdp, context, url, agent }, cx).await
        })
    }

    pub(super) fn forget_previews(&mut self) {
        self.previews = Previews::default();
    }
}

#[cfg(test)]
mod tests;
