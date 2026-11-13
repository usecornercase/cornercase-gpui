#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Ask {
    pub(super) seq: u64,
    pub(super) talk: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Keep {
    pub(super) facts: bool,
    pub(super) talk: bool,
}

#[derive(Debug)]
pub(super) struct Reads {
    base: u64,
    asked_at: Option<i64>,
    talk: bool,
    next: u64,
    facts: u64,
    told: u64,
}

impl Reads {
    pub(super) fn new(base: u64, feed_at: Option<i64>) -> (Self, Ask) {
        let mut reads = Self { base, asked_at: feed_at, talk: false, next: base, facts: base, told: base };
        let ask = reads.ask();
        (reads, ask)
    }

    fn ask(&mut self) -> Ask {
        self.next += 1;
        Ask { seq: self.next, talk: self.talk }
    }

    pub(super) fn moved(&mut self, feed_at: Option<i64>) -> Option<Ask> {
        if feed_at == self.asked_at {
            return None;
        }
        self.asked_at = feed_at;
        Some(self.ask())
    }

    pub(super) fn again(&mut self) -> Ask {
        self.ask()
    }

    pub(super) fn wants_talk(&mut self) -> Option<Ask> {
        if self.talk {
            return None;
        }
        self.talk = true;
        Some(self.ask())
    }

    pub(super) fn answered(&mut self, ask: Ask) -> Keep {
        if ask.seq <= self.base {
            return Keep { facts: false, talk: false };
        }
        let facts = ask.seq > self.facts;
        let talk = ask.talk && ask.seq > self.told;
        if facts {
            self.facts = ask.seq;
        }
        if talk {
            self.told = ask.seq;
        }
        Keep { facts, talk }
    }
}

#[derive(Debug, Default)]
pub(super) struct Counter(u64);

impl Counter {
    pub(super) fn base(&self) -> u64 {
        self.0
    }

    pub(super) fn seen(&mut self, ask: Ask) {
        self.0 = self.0.max(ask.seq);
    }
}

pub(super) fn refreshing_keys(id: u64, talk_shown: bool) -> Vec<String> {
    let mut keys = vec![job_key(id, false)];
    if talk_shown {
        keys.push(job_key(id, true));
    }
    keys
}

pub(super) fn show_args(id: u64, talk: bool) -> Vec<String> {
    let mut args: Vec<String> = ["queue", "show"].map(String::from).into();
    args.push(id.to_string());
    args.push("--json".into());
    if talk {
        args.push("--conversation".into());
    }
    args
}

pub(super) fn job_key(id: u64, talk: bool) -> String {
    if talk { format!("queue-show:{id}:talk") } else { format!("queue-show:{id}") }
}

pub(super) fn job_words(label: &str, talk: bool) -> String {
    if talk {
        format!("Reading the conversation of “{label}”…")
    } else {
        format!("Reading what “{label}” did…")
    }
}

pub(super) fn failed(talk: bool, reason: &str) -> String {
    let what = if talk { "the conversation" } else { "what it did" };
    format!("Could not read {what}: {reason}")
}

#[cfg(test)]
mod tests;
