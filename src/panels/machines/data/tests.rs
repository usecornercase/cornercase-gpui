use super::*;
use crate::model::machines::{LinkState, MachineIcon};

fn draft() -> Draft {
    Draft {
        name: "vps".into(),
        icon: MachineIcon::Server,
        destination: "ana@vps.example.com".into(),
        folder: "~/code".into(),
    }
}

fn machine(name: &str) -> Machine {
    Machine { name: name.into(), ..Machine::default() }
}

#[test]
fn the_first_row_is_this_computer_here() {
    assert_eq!(here_words(None).0, "this computer");
}

#[test]
fn on_another_server_the_first_row_names_it() {
    let (name, detail) = here_words(Some("devbox"));
    assert_eq!(name, "devbox");
    assert!(detail.contains("this window is connected to"), "{detail}");
}

#[test]
fn the_sheet_says_how_to_move_the_window_with_the_platforms_keys() {
    assert!(window_note(Os::Mac).ends_with("press ⇧⌘K."));
    assert!(window_note(Os::Linux).ends_with("press Ctrl+Alt+Shift+K."));
}

#[test]
fn adding_gives_every_value_and_puts_the_name_after_the_dashes() {
    assert_eq!(
        add(&draft()).command,
        ["machines", "add", "--json", "--icon=server", "--folder=~/code", "--", "vps", "ana@vps.example.com"]
    );
}

#[test]
fn an_icon_goes_by_its_name_with_its_space() {
    let home = Draft { icon: MachineIcon::HomeServer, ..draft() };
    assert!(add(&home).command.contains(&"--icon=home server".to_string()));
}

#[test]
fn editing_sends_only_what_changed() {
    let renamed = Draft { name: "cloud".into(), folder: "/srv/code".into(), ..draft() };
    let saving = edit(&draft(), &renamed).expect("a change");
    assert_eq!(saving.command, ["machines", "edit", "--json", "--name=cloud", "--folder=/srv/code", "--", "vps"]);
}

#[test]
fn editing_an_icon_or_a_destination_names_each() {
    let moved = Draft { icon: MachineIcon::Mini, destination: "mini".into(), ..draft() };
    let saving = edit(&draft(), &moved).expect("a change");
    assert_eq!(saving.command, ["machines", "edit", "--json", "--icon=mini", "--destination=mini", "--", "vps"]);
}

#[test]
fn editing_nothing_runs_nothing() {
    assert!(edit(&draft(), &draft()).is_none());
}

#[test]
fn a_new_name_starting_with_a_dash_cannot_be_read_as_an_option() {
    let renamed = Draft { name: "-x".into(), ..draft() };
    assert!(edit(&draft(), &renamed).expect("a change").command.contains(&"--name=-x".to_string()));
}

#[test]
fn each_row_action_names_its_command() {
    assert_eq!(Doing::Retry.command("vps"), ["machines", "retry", "--json", "--", "vps"]);
    assert_eq!(Doing::Update.command("vps"), ["machines", "update", "--json", "--", "vps"]);
    assert_eq!(Doing::Remove.command("vps"), ["machines", "remove", "--json", "--", "vps"]);
}

#[test]
fn the_status_bar_names_the_machine() {
    assert!(format!("{:?}", Doing::Update.job("vps")).contains("Updating cornercase on vps…"));
}

#[test]
fn removing_says_that_nothing_there_is_touched() {
    let ask = remove_ask("vps");
    assert_eq!(ask.title, "Remove the machine vps?");
    assert!(ask.message.contains("keeps running there"));
    assert!(ask.message.contains("nothing there is touched"));
}

#[test]
fn updating_says_that_nothing_restarts() {
    assert!(update_ask("vps").message.contains("restarts nothing"));
    assert!(update_ask("vps").message.contains("installs the latest release"));
}

#[test]
fn a_second_entry_with_a_name_is_shadowed_by_the_first() {
    let machines = [machine("vps"), machine("VPS"), machine("mini")];
    let shadowed: Vec<bool> = listed(&machines).iter().map(|row| row.shadowed).collect();
    assert_eq!(shadowed, [false, true, false]);
}

#[test]
fn the_edit_form_goes_under_the_first_entry_of_a_name() {
    let machines = [machine("vps"), machine("vps")];
    let edited: Vec<bool> = listed(&machines).iter().map(|row| edits(*row, "vps")).collect();
    assert_eq!(edited, [true, false]);
}

#[test]
fn retrying_stops_showing_once_the_link_is_up() {
    let down = Machine { state: LinkState::Unreachable, ..machine("vps") };
    let up = Machine { state: LinkState::Linked, ..machine("vps") };
    let (down, up) = (std::slice::from_ref(&down), std::slice::from_ref(&up));
    assert!(shows_doing(Doing::Retry, listed(down)[0]));
    assert!(!shows_doing(Doing::Retry, listed(up)[0]));
}

#[test]
fn a_shadowed_entry_shows_nothing_running_for_the_first_of_its_name() {
    let machines = [machine("vps"), machine("vps")];
    let rows = listed(&machines);
    assert!(shows_doing(Doing::Remove, rows[0]));
    assert!(!shows_doing(Doing::Remove, rows[1]));
}

#[test]
fn what_ssh_said_shows_under_the_row() {
    let down = Machine { error: Some("ssh: refused".into()), state: LinkState::Unreachable, ..machine("down") };
    let rows = listed(std::slice::from_ref(&down));
    assert_eq!(notes(rows[0]), [Note::Error("ssh: refused".into())]);
}

#[test]
fn a_problem_says_it_comes_from_the_file() {
    let wrong = Machine { problem: Some("the name is required".into()), ..machine("") };
    let rows = listed(std::slice::from_ref(&wrong));
    assert_eq!(notes(rows[0]), [Note::Error("In config.json: the name is required".into())]);
}

#[test]
fn a_shadowed_entry_says_only_the_file_can_fix_it() {
    let machines = [machine("vps"), Machine { problem: Some("taken".into()), ..machine("vps") }];
    let rows = listed(&machines);
    let shown = notes(rows[1]);
    assert!(matches!(&shown[..], [Note::Error(text)] if text.contains("only be fixed in the file")), "{shown:?}");
}

#[test]
fn a_server_behind_its_install_is_a_hint() {
    let behind = Machine {
        state: LinkState::Linked,
        version: Some("0.13.1".into()),
        server_version: Some("0.13.0".into()),
        ..machine("vps")
    };
    let rows = listed(std::slice::from_ref(&behind));
    assert_eq!(notes(rows[0]), [Note::Hint("Its server runs 0.13.0 until it restarts there".into())]);
}
