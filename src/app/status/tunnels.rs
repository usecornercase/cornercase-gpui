use gpui::{ClipboardItem, Context, Pixels, Point, Window};

use crate::app::Root;
use crate::cli::Feature;
use crate::context_menu::{self, Entry};
use crate::dialogs;
use crate::keymap::{Command, keys_of};
use crate::model::Owner;
use crate::model::machines::listed;
use crate::model::tunnels::{Toward, others_note};
use crate::panels::{PanelKind, on_machine};
use crate::platform::Os;
use crate::store::ToastKind;
use crate::store::remote_url;
use crate::store::{Reach, Tail, TunnelRow, TunnelSection, Whose, heading, none_note, section_tail};
use crate::store::{Store, address_there, copied_words};

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
        this.store.update(cx, |s, cx| s.try_tunnel_again(&toward, port, cx));
    })
}

fn reverse_actions(toward: &Toward, there: Option<u16>, name: &str) -> Vec<Entry> {
    let close = toward.clone();
    let mut entries = Vec::new();
    if let Some(there) = there {
        let (address, label, copied) =
            (address_there(there), format!("Copy its address on {name}"), copied_words(name));
        entries.push(context_menu::note(format!("Every user and program on {name} can drive it")));
        entries.push(context_menu::item(label, move |this, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(address.clone()));
            this.store.update(cx, |s, cx| s.toast(copied.clone(), ToastKind::Info, cx));
        }));
    }
    entries.push(context_menu::item("Close", move |this, _, cx| {
        this.store.update(cx, |s, cx| s.close_reverse(&close, cx));
    }));
    entries
}

fn actions(row: &TunnelRow, shown: Option<Owner>, name: &str) -> Vec<Entry> {
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
        Whose::Reverse { there } => reverse_actions(toward, *there, name),
        Whose::Window | Whose::Program | Whose::ClosedElsewhere => closing(toward, port),
    }
}

fn server_entries(host: &str, tunnels: &[TunnelRow], notes: Vec<String>) -> Vec<Entry> {
    let mut entries = vec![Entry::Heading(heading(host).into())];
    if tunnels.is_empty() {
        entries.push(context_menu::note(none_note(keys_of(Command::Panel(PanelKind::Ports), Os::CURRENT).as_deref())));
    }
    entries.extend(tunnels.iter().map(|row| context_menu::submenu(row.text(), actions(row, None, host))));
    entries.extend(notes.into_iter().map(context_menu::note));
    entries
}

fn tail_entry(tail: Tail) -> Entry {
    match tail {
        Tail::Note(text) => context_menu::note(text),
        Tail::Update(update) => context_menu::item(update.label(), move |_, window, cx| {
            on_machine::ask_update_there(&update, window, cx);
        }),
        Tail::Updating(label) => context_menu::resting(label),
    }
}

fn section_row(section: &TunnelSection, row: &TunnelRow, shown: Option<Owner>) -> Entry {
    if section.rests(row) {
        context_menu::resting(row.text())
    } else {
        context_menu::submenu(row.text(), actions(row, shown, &section.name))
    }
}

fn machine_section(section: &TunnelSection, store: &Store) -> Vec<Entry> {
    let name = &section.name;
    let mut entries = vec![Entry::Heading(heading(name).into())];
    let shown = store.owner_on(name);
    entries.extend(section.rows.iter().map(|row| section_row(section, row, shown.clone())));
    entries.extend(section.last_sent.iter().cloned().map(context_menu::note));
    let reach = store.reach_of(name);
    let note = reach.as_ref().and_then(Reach::older);
    entries.extend(section_tail(note, &store.update_offer(note)).into_iter().map(tail_entry));
    entries
}

fn machine_entries(sections: &[TunnelSection], store: &Store) -> Vec<Entry> {
    let mut entries: Vec<Entry> = sections.iter().flat_map(|section| machine_section(section, store)).collect();
    let names: Vec<String> = sections.iter().map(|section| section.name.clone()).collect();
    let towards: Vec<Toward> = names.iter().cloned().map(Toward::Machine).collect();
    entries.extend(store.orders_notes(&towards).into_iter().map(context_menu::note));
    let gate = store.server_has(Feature::MachineTunnels);
    entries.extend(others_note(gate, &listed(&names)).map(context_menu::note));
    entries
}

impl Root {
    pub(crate) fn open_machine_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let mut entries = match &self.host {
            Some(host) => server_entries(host, &store.tunnel_rows(), store.orders_notes(&[Toward::Server])),
            None => machine_entries(&store.machine_tunnel_sections(), store),
        };
        entries.push(Entry::Separator);
        entries.push(context_menu::item("Connect to another server…", |_, window, cx| dialogs::connect(window, cx)));
        self.open_menu_above(at, entries, window, cx);
    }
}
