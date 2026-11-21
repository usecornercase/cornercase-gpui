use super::super::Store;
use crate::cli::Feature;
use crate::model::bots::{self, FollowRow, Home};
use crate::platform::Os;

impl Store {
    pub(crate) fn started_by(&self, bot: Option<&str>) -> Option<String> {
        self.bots.find(bot?).map(|bot| bots::by(&bot.name))
    }

    pub(crate) fn shown_agents(&self) -> Vec<u64> {
        let (Some((bot, chat)), Some(session)) = (self.shown_chat(), self.session.as_ref()) else { return Vec::new() };
        if bot.home != Home::Here {
            return Vec::new();
        }
        bots::showable(&chat.lines, &session.boot, |pane| session.pane(pane).is_some())
    }

    pub(crate) fn follow_row(&self) -> FollowRow {
        bots::follow_row(self.releases().lacks(Feature::BotFollow), Os::CURRENT)
    }
}
