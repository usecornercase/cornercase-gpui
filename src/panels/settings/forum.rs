use serde::Deserialize;
use serde_json::Value;

pub(super) const SETTING: &str = "discord_forum";
pub(super) const BOT: &str = "discord";
pub(super) const PICKING: &str = "Choosing among several servers still takes cornercase's own window, Settings › \
                                  Remote: needs usecornercase/cornercase-terminal#668.";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Forum {
    on: bool,
    ready: bool,
    guild: Option<String>,
    channel: Option<String>,
    pending: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    Off,
    Waiting,
    Ready,
}

pub(super) fn of_app(app: Option<&Value>) -> Option<Forum> {
    let away = app?.get("away")?.as_array()?;
    let discord = away.iter().find(|row| row.get("platform").and_then(Value::as_str) == Some("discord"))?;
    serde_json::from_value(discord.get("forum")?.clone()).ok()
}

impl Forum {
    pub(super) fn stage(&self) -> Stage {
        match (self.on, self.ready) {
            (false, _) => Stage::Off,
            (true, false) => Stage::Waiting,
            (true, true) => Stage::Ready,
        }
    }

    pub(super) fn words(&self) -> String {
        match (self.stage(), &self.pending) {
            (Stage::Off, _) => "Forum mode off: the forum is kept for when it comes back".to_string(),
            (Stage::Ready, _) => "Forum ready: each workspace gets its own post".to_string(),
            (Stage::Waiting, Some(pending)) => format!("Forum not ready: {pending}"),
            (Stage::Waiting, None) => "Forum not ready: setting it up".to_string(),
        }
    }

    pub(super) fn address(&self) -> Option<String> {
        let (guild, channel) = (self.guild.as_ref()?, self.channel.as_ref()?);
        (self.stage() == Stage::Ready).then(|| format!("https://discord.com/channels/{guild}/{channel}"))
    }
}

#[cfg(test)]
mod tests;
