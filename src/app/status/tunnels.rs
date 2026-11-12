use gpui::{ClipboardItem, Context, Pixels, Point, Window};

use crate::app::Root;
use crate::context_menu::{self, Entry};
use crate::dialogs;
use crate::store::ToastKind;
use crate::store::tunnels::held::Asked;
use crate::store::tunnels::remote_url;
use crate::store::tunnels::rows::{self, TunnelRow, Whose};

fn closing(port: u16) -> Vec<Entry> {
    vec![
        context_menu::item("Close", move |this, _, cx| this.store.update(cx, |s, cx| s.close_tunnel(port, false, cx))),
        context_menu::item("Forget", move |this, _, cx| this.store.update(cx, |s, cx| s.close_tunnel(port, true, cx))),
    ]
}

fn copy_item(local: u16) -> Entry {
    let url = format!("http://localhost:{local}");
    context_menu::item(format!("Copy {url}"), move |this, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
        this.store.update(cx, |s, cx| s.toast(format!("Copied {url}"), ToastKind::Info, cx));
    })
}

fn actions(row: &TunnelRow) -> Vec<Entry> {
    let port = row.port;
    match &row.whose {
        Whose::Up { local } => {
            let open = context_menu::item("Open in the Browser", move |this, _, cx| {
                this.store.update(cx, |s, cx| s.open_in_browser(remote_url(port), None, cx));
            });
            let mut entries = vec![open, copy_item(*local)];
            entries.extend(closing(port));
            entries
        }
        Whose::Opening => vec![context_menu::item("Close", move |this, _, cx| {
            this.store.update(cx, |s, cx| s.close_tunnel(port, false, cx));
        })],
        Whose::Down { .. } => {
            let again = context_menu::item("Try again", move |this, _, cx| {
                this.store.update(cx, |s, cx| {
                    s.drop_down_tunnel(port);
                    s.hold_tunnel(port, Asked::Click, cx);
                });
            });
            let mut entries = vec![again];
            entries.extend(closing(port));
            entries
        }
        Whose::Window | Whose::Program | Whose::ClosedElsewhere => closing(port),
    }
}

impl Root {
    pub(crate) fn open_machine_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let tunnels = store.tunnel_rows();
        let host = self.host.clone().unwrap_or_default();
        let mut entries = vec![Entry::Heading(format!("Tunnels to {host}").into())];
        if tunnels.is_empty() {
            entries.push(context_menu::note(rows::none_note()));
        }
        entries.extend(tunnels.iter().map(|row| context_menu::submenu(row.text(), actions(row))));
        entries.push(context_menu::note(rows::asks_note(&host)));
        entries.push(Entry::Separator);
        entries.push(context_menu::item("Connect to another server…", |_, window, cx| dialogs::connect(window, cx)));
        self.open_menu(context_menu::above(at, &entries), entries, window, cx);
    }
}
