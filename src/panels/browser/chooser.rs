use std::path::PathBuf;

use gpui::{Context, PathPromptOptions};
use serde_json::{Value, json};

use super::cdp::Event;
use super::{Browser, say};

pub(crate) fn chooser(params: &Value) -> Option<(i64, bool)> {
    let node = params["backendNodeId"].as_i64()?;
    Some((node, params["mode"].as_str() == Some("selectMultiple")))
}

impl Browser {
    pub(super) fn choose_files(event: &Event, cx: &mut Context<Self>) {
        let (Some(session), Some((node, multiple))) = (event.session.clone(), chooser(&event.params)) else { return };
        let paths = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple, prompt: None });
        cx.spawn(async move |this, cx| {
            let chosen = paths.await.map_err(anyhow::Error::from).and_then(|chosen| chosen);
            let _ = this.update(cx, |this, cx| this.chosen(&session, node, chosen, cx));
        })
        .detach();
    }

    fn chosen(&self, session: &str, node: i64, chosen: anyhow::Result<Option<Vec<PathBuf>>>, cx: &mut Context<Self>) {
        let paths = match chosen {
            Ok(Some(paths)) => paths,
            Ok(None) => return,
            Err(e) => return say(format!("The file picker could not open: {e}"), cx),
        };
        let files: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        let params = json!({ "files": files, "backendNodeId": node });
        self.call((Some(session), "DOM.setFileInputFiles", params), cx, |_, result, cx| {
            if let Err(e) = result {
                say(format!("The page did not take the files: {e}"), cx);
            }
        });
    }
}

#[cfg(test)]
mod tests;
