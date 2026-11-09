use std::cell::Cell;
use std::time::Duration;

use gpui::{AsyncApp, Context, Task, WeakEntity};

const COUNTS_EVERY: Duration = Duration::from_secs(1);

pub(crate) type Access<V> = fn(&mut V) -> &mut Counting;

#[derive(Default)]
pub(crate) struct Counting {
    drawn: Cell<bool>,
    next: Option<Task<()>>,
}

impl Counting {
    pub(crate) fn drawn(&self) {
        self.drawn.set(true);
    }

    pub(crate) fn keep<V: 'static>(&mut self, access: Access<V>, cx: &Context<V>) {
        if !self.drawn.take() {
            self.next = None;
        } else if self.next.is_none() {
            self.next = Some(cx.spawn(async move |this, cx| next_count(this, access, cx).await));
        }
    }
}

async fn next_count<V: 'static>(this: WeakEntity<V>, access: Access<V>, cx: &mut AsyncApp) {
    cx.background_executor().timer(COUNTS_EVERY).await;
    let _ = this.update(cx, |this, cx| {
        access(this).next = None;
        cx.notify();
    });
}
