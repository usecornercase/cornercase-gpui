use gpui::{ClipboardItem, Context, Pixels, Point, Window};

use crate::app::Root;
use crate::context_menu::{self, Entry};
use crate::dialogs;
use crate::keymap::{Command, keys_of};
use crate::model::Owner;
use crate::model::machines::listed;
use crate::model::tunnels::Toward;
use crate::panels::PanelKind;
use crate::platform::Os;
use crate::store::ToastKind;
use crate::store::remote_url;
use crate::store::{Asked, Store};
use crate::store::{TunnelRow, Whose, asks_note, heading, none_note, others_note};

fn closing(toward: &Toward, port: u16) -> Vec<Entry> {
    let (close, forget) = (toward.clone(), toward.clone());
    vec![
        context_menu::item("Close", move |this, _, cx| {
            this.store.update(cx, |s, cx| s.close_tunnel(&close, port, false, cx));
        }),
        context_menu::item("Forget", move |this, _, cx| {
            this.store.update(cx, |s, cx| s.close_tunnel(&forget, port, true, cx));
        }),
    ]
}

fn copy_item(local: u16) -> Entry {
    let url = format!("http://localhost:{local}");
    context_menu::item(format!("Copy {url}"), move |this, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
        this.store.update(cx, |s, cx| s.toast(format!("Copied {url}"), ToastKind::Info, cx));
    })
}

pub(crate) fn open_label(toward: &Toward, shown: Option<&Owner>) -> &'static str {
    match (toward, shown) {
        (Toward::Machine(_), None) => "Open in the system browser",
        _ => "Open in the Browser",
    }
}

fn open_item(toward: &Toward, port: u16, local: u16, shown: Option<Owner>) -> Entry {
    let label = open_label(toward, shown.as_ref());
    match (toward, shown) {
        (Toward::Machine(_), None) => {
            let url = format!("http://localhost:{local}");
            context_menu::item(label, move |_, _, cx| cx.open_url(&url))
        }
        (Toward::Machine(_), Some(owner)) => context_menu::item(label, move |this, _, cx| {
            let owner = owner.clone();
            this.store.update(cx, |s, cx| s.open_in_browser(remote_url(port), owner, cx));
        }),
        (Toward::Server, _) => context_menu::item(label, move |this, _, cx| {
            this.store.update(cx, |s, cx| {
                let owner = s.local_owner();
                s.open_in_browser(remote_url(port), owner, cx);
            });
        }),
    }
}

fn try_again(toward: &Toward, port: u16) -> Entry {
    let toward = toward.clone();
    context_menu::item("Try again", move |this, _, cx| {
        this.store.update(cx, |s, cx| {
            s.drop_down_tunnel(&toward, port);
            s.hold_tunnel(&toward, port, Asked::Click, cx);
        });
    })
}

fn actions(row: &TunnelRow, shown: Option<Owner>) -> Vec<Entry> {
    let (port, toward) = (row.port, &row.toward);
    match &row.whose {
        Whose::Up { local } => {
            let mut entries = vec![open_item(toward, port, *local, shown), copy_item(*local)];
            entries.extend(closing(toward, port));
            entries
        }
        Whose::Opening => {
            let toward = toward.clone();
            vec![context_menu::item("Close", move |this, _, cx| {
                this.store.update(cx, |s, cx| s.close_tunnel(&toward, port, false, cx));
            })]
        }
        Whose::Down { why } => {
            let mut entries: Vec<Entry> =
                (!why.is_empty()).then(|| context_menu::note(why.clone())).into_iter().collect();
            entries.push(try_again(toward, port));
            entries.extend(closing(toward, port));
            entries
        }
        Whose::Window | Whose::Program | Whose::ClosedElsewhere => closing(toward, port),
    }
}

fn server_entries(host: &str, tunnels: &[TunnelRow]) -> Vec<Entry> {
    let mut entries = vec![Entry::Heading(heading(host).into())];
    if tunnels.is_empty() {
        entries.push(context_menu::note(none_note(keys_of(Command::Panel(PanelKind::Ports), Os::CURRENT).as_deref())));
    }
    entries.extend(tunnels.iter().map(|row| context_menu::submenu(row.text(), actions(row, None))));
    entries.push(context_menu::note(asks_note(host)));
    entries
}

fn machine_entries(sections: &[(String, Vec<TunnelRow>)], store: &Store) -> Vec<Entry> {
    let mut entries = Vec::new();
    for (name, rows) in sections {
        entries.push(Entry::Heading(heading(name).into()));
        let shown = store.owner_on(name);
        entries.extend(rows.iter().map(|row| context_menu::submenu(row.text(), actions(row, shown.clone()))));
    }
    let names: Vec<String> = sections.iter().map(|(name, _)| name.clone()).collect();
    entries.push(context_menu::note(asks_note(&listed(&names))));
    entries.push(context_menu::note(others_note(&listed(&names))));
    entries
}

impl Root {
    pub(crate) fn open_machine_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let mut entries = match &self.host {
            Some(host) => server_entries(host, &store.tunnel_rows()),
            None => machine_entries(&store.machine_tunnel_sections(), store),
        };
        entries.push(Entry::Separator);
        entries.push(context_menu::item("Connect to another server…", |_, window, cx| dialogs::connect(window, cx)));
        self.open_menu_above(at, entries, window, cx);
    }
}
