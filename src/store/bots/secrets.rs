use gpui::Context;

use super::super::{Busy, Store};
use super::talk::{Call, Then};
use crate::cli::{self, Feature};
use crate::model::bots::{
    self, BotKey, SecretsSection, SecretsSeen, secret_remove_args, secret_remove_failed, secret_save_failed,
    secret_set_args,
};
use crate::platform::Os;

fn remove_key(bot: &str, name: &str) -> String {
    format!("bot-secret-remove:{bot}:{name}")
}

impl Store {
    pub(crate) fn secrets_section(&self) -> SecretsSection {
        bots::secrets_section(self.releases().lacks(Feature::BotSecrets), Os::CURRENT)
    }

    pub(crate) fn secrets_seen(&self, bot: &str) -> Option<SecretsSeen> {
        let found = self.bots.find(bot)?;
        Some(SecretsSeen {
            saved: self.bots.manage.secrets_saved(),
            count: found.secrets,
            unreadable: found.secrets_unreadable,
        })
    }

    pub(crate) fn removing_secret(&self, bot: &str, name: &str) -> bool {
        self.busy(&remove_key(bot, name))
    }

    pub(crate) fn save_secret(
        &mut self,
        (bot, name): (&str, &str),
        value: String,
        dialog: u64,
        cx: &mut Context<Self>,
    ) {
        let doing = format!("Saving a secret of {}…", self.bot_label(&BotKey::here(bot)));
        let then: Then = Box::new(move |this, result, cx| match result {
            Ok(_) => {
                this.bots.manage.secret_saved();
                this.close_started(Some(dialog), false, cx);
                cx.notify();
            }
            Err(error) => this.refused_here(dialog, secret_save_failed(&cli::reason(&error)), cx),
        });
        self.dialog_job(
            (dialog, doing),
            Call::here_with_stdin(secret_set_args(bot, name), value.into_bytes()),
            then,
            cx,
        );
    }

    pub(crate) fn remove_secret(&mut self, (bot, name): (&str, &str), form: u64, cx: &mut Context<Self>) {
        let key = remove_key(bot, name);
        if self.busy(&key) {
            return;
        }
        let bot_name = self.bot_label(&BotKey::here(bot));
        let busy = Busy::new(key, format!("Removing {name} from {bot_name}…"));
        let secret = name.to_string();
        let then: Then = Box::new(move |this, result, cx| match result {
            Ok(_) => {
                this.bots.manage.secret_saved();
                cx.notify();
            }
            Err(error) => {
                this.refused_here(form, secret_remove_failed(&secret, &bot_name, &cli::reason(&error)), cx);
            }
        });
        self.bot_job(busy, Call::here(secret_remove_args(bot, name)), then, cx);
    }
}
