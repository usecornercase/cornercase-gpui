use super::*;

fn machine(name: &str, destination: &str) -> Machine {
    Machine { name: name.into(), destination: destination.into(), ..Machine::default() }
}

fn draft(name: &str, destination: &str, folder: &str) -> Draft {
    Draft { name: name.into(), icon: MachineIcon::Server, destination: destination.into(), folder: folder.into() }
}

fn name_refused(name: &str, machines: &[Machine]) -> Option<String> {
    let others: Vec<&Machine> = machines.iter().collect();
    refusals(&draft(name, "host", "~/code"), &others, true).name
}

fn destination_refused(destination: &str, machines: &[Machine]) -> Option<String> {
    let others: Vec<&Machine> = machines.iter().collect();
    refusals(&draft("vps", destination, "~/code"), &others, true).destination
}

fn folder_refused(folder: &str) -> Option<String> {
    refusals(&draft("vps", "host", folder), &[], true).folder
}

#[test]
fn a_new_machine_starts_with_the_first_icon_and_the_usual_folder() {
    let new = Draft::new();
    assert_eq!((new.name.as_str(), new.icon, new.folder.as_str()), ("", MachineIcon::Server, "~/code"));
}

#[test]
fn the_edit_form_is_filled_from_the_machine() {
    let listed = Machine {
        name: "vps".into(),
        icon: MachineIcon::Mini,
        destination: "d".into(),
        folder: "/f".into(),
        ..Machine::default()
    };
    assert_eq!(
        Draft::of(&listed),
        Draft { name: "vps".into(), icon: MachineIcon::Mini, destination: "d".into(), folder: "/f".into() }
    );
}

#[test]
fn a_good_name_is_not_refused() {
    for name in ["vps", "mini-2", "a.b_c", "12345678901234567890"] {
        assert_eq!(name_refused(name, &[]), None, "{name}");
    }
}

#[test]
fn a_name_takes_only_letters_digits_and_three_marks() {
    for name in ["my vps", "vps!", "máquina", "123456789012345678901"] {
        assert!(name_refused(name, &[]).expect(name).contains("at most 20"), "{name}");
    }
}

#[test]
fn all_is_not_a_name_in_any_case() {
    for name in ["all", "ALL", "All"] {
        assert!(name_refused(name, &[]).expect(name).contains("kept for every machine"), "{name}");
    }
}

#[test]
fn a_name_is_unique_ignoring_case() {
    let refusal = name_refused("VPS", &[machine("vps", "other")]).expect("refused");
    assert_eq!(refusal, "Another machine is already called `vps`");
}

#[test]
fn a_machine_keeps_its_own_name_when_edited() {
    let machines = [machine("vps", "one"), machine("mini", "two")];
    let others = others(&machines, Some("VPS"));
    assert_eq!(others.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["mini"]);
}

#[test]
fn a_new_machine_is_checked_against_them_all() {
    let machines = [machine("vps", "one"), machine("mini", "two")];
    assert_eq!(others(&machines, None).len(), 2);
}

#[test]
fn a_destination_cannot_start_with_a_dash() {
    assert!(destination_refused("-oProxy", &[]).expect("refused").contains("start with"));
}

#[test]
fn a_destination_holds_no_spaces() {
    assert!(destination_refused("ana@vps example", &[]).expect("refused").contains("spaces"));
}

#[test]
fn a_destination_has_a_limit() {
    assert!(destination_refused(&"a".repeat(256), &[]).expect("refused").contains("255"));
    assert_eq!(destination_refused(&"a".repeat(255), &[]), None);
}

#[test]
fn a_destination_is_unique() {
    let refusal = destination_refused("one", &[machine("mini", "one")]).expect("refused");
    assert_eq!(refusal, "The machine `mini` already uses this destination");
}

#[test]
fn a_folder_is_absolute_or_from_home() {
    for folder in ["/srv/code", "~", "~/code"] {
        assert_eq!(folder_refused(folder), None, "{folder}");
    }
    for folder in ["code", "~code", "./code"] {
        assert!(folder_refused(folder).expect(folder).contains("absolute"), "{folder}");
    }
}

#[test]
fn an_empty_field_waits_until_the_form_is_sent() {
    let empty = draft("", "", "");
    assert_eq!(refusals(&empty, &[], false), Refusals::default());
    let sent = refusals(&empty, &[], true);
    assert_eq!(sent.name.as_deref(), Some("The name is required"));
    assert_eq!(sent.destination.as_deref(), Some("The destination is required"));
    assert_eq!(sent.folder.as_deref(), Some("The folder is required"));
    assert!(sent.any());
}

#[test]
fn a_wrong_value_is_said_at_once() {
    assert!(refusals(&draft("all", "", ""), &[], false).name.is_some());
}

#[test]
fn hosts_offered_narrow_as_one_types() {
    let hosts = ["devbox".to_string(), "mini".to_string()];
    assert_eq!(hosts_offered(&hosts, "", &[]), hosts);
    assert_eq!(hosts_offered(&hosts, "DEV", &[]), ["devbox"]);
}

#[test]
fn a_host_already_typed_is_no_longer_offered() {
    assert_eq!(hosts_offered(&["devbox".to_string()], "devbox", &[]), Vec::<String>::new());
}

#[test]
fn a_host_another_machine_uses_is_not_offered() {
    let used = machine("box", "devbox");
    assert_eq!(hosts_offered(&["devbox".to_string()], "", &[&used]), Vec::<String>::new());
}

#[test]
fn only_a_few_hosts_are_offered_at_once() {
    let hosts: Vec<String> = (0..20).map(|n| format!("host{n}")).collect();
    assert_eq!(hosts_offered(&hosts, "", &[]).len(), 8);
}

#[test]
fn the_note_says_what_is_needed_with_this_servers_version() {
    let note = setup_note(Some("0.13.1"), "", None);
    assert_eq!(
        note,
        "It must run Linux or macOS. Install cornercase 0.13.1 or newer there, and connect once by hand with \
         `ssh you@host true` so the key and the host are known, since the link never asks."
    );
}

#[test]
fn the_note_names_the_oldest_version_until_it_knows_this_servers() {
    let oldest = format!("cornercase {} or newer", cli::MINIMUM);
    assert!(setup_note(None, "", None).contains(&oldest));
    assert!(setup_note(Some(""), "", None).contains(&oldest));
}

#[test]
fn the_empty_page_says_what_a_machine_needs() {
    let needs = empty_needs(Some("0.13.1"));
    assert!(needs.contains("Linux or macOS"), "{needs}");
    assert!(needs.contains("cornercase 0.13.1 or newer"), "{needs}");
}

#[test]
fn the_note_follows_the_destination_typed() {
    assert!(setup_note(None, "ana@vps.example.com", None).contains("`ssh ana@vps.example.com true`"));
}

#[test]
fn on_another_server_the_connection_is_made_from_there() {
    let note = setup_note(None, "mini", Some("devbox"));
    assert!(note.contains("by hand from devbox with `ssh mini true`"), "{note}");
}

#[test]
fn the_destinations_rule_names_whose_ssh_config_counts() {
    assert!(destination_rule(None).contains("your ~/.ssh/config"));
    assert!(destination_rule(Some("devbox")).contains("on devbox"));
}
