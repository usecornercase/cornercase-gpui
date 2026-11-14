use std::{collections::HashMap, fs, path::Path};

const RAIL: &str = "src/sidebar/rail.rs";
const GEAR: &str = "src/sidebar/rail/gear.rs";
const PANELS: &str = "src/panels/mod.rs";
const ICONS: &str = "src/ui/icons.rs";
const PAGE_RAIL: &str = "site/src/components/stage/Rail.astro";
const STAGE: &str = "site/src/components/stage";

const RULE: &str = "|";
const GAP: &str = "…";

const TEACH: &str = "tests/website.rs reads this to compare the page's window with the app's: if what it draws is the same, teach the test the new shape (design/website.md, Kept in step with the app)";
const KNOWN_WORDS: [&str; 6] = ["into_iter", "chain", "map", "collect", "then_some", "flow"];

type Found<T> = Result<T, String>;

fn unread(file: &str, what: &str) -> String {
    format!("{file}: {what}; {TEACH}")
}

fn read(path: &str) -> String {
    let full = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    let text = fs::read_to_string(full).unwrap_or_else(|why| panic!("cannot read {path}: {why}"));
    assert!(!text.contains('\r'), "{path} must use \\n line endings (.gitattributes)");
    text
}

fn block<'a>(text: &'a str, file: &str, start: &str) -> Found<&'a str> {
    let from = text.find(start).ok_or_else(|| unread(file, &format!("cannot find `{start}`")))?;
    let rest = &text[from + start.len()..];
    let open = rest.find('{').ok_or_else(|| unread(file, &format!("`{start}` has no body")))?;
    let mut depth = 0;
    let close = rest[open..]
        .char_indices()
        .find(|&(_, c)| {
            depth += i32::from(c == '{') - i32::from(c == '}');
            depth == 0
        })
        .map(|(at, _)| open + at)
        .ok_or_else(|| unread(file, &format!("`{start}` never closes")))?;
    Ok(&rest[open + 1..close])
}

fn declared<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let rest = &text[text.find(&format!("const {name}:"))?..];
    let rest = &rest[rest.find('=')?..];
    Some(&rest[..rest.find(';')?])
}

fn words(code: &str) -> Vec<&str> {
    code.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == ':'))
        .map(|word| word.trim_matches(':'))
        .filter(|word| !word.is_empty())
        .collect()
}

fn arms(code: &str, prefix: &str, file: &str) -> Found<HashMap<String, String>> {
    let mut found = HashMap::new();
    for (left, right) in code.lines().filter_map(|line| line.split_once("=>")) {
        let value = right.trim().trim_end_matches(',').to_string();
        for name in words(left).into_iter().filter_map(|word| word.strip_prefix(prefix)) {
            if let Some(old) = found.insert(name.to_string(), value.clone()) {
                return Err(unread(file, &format!("{prefix}{name} has two values: {old} and {value}")));
            }
        }
    }
    Ok(found)
}

fn quoted(text: &str) -> Option<&str> {
    let open = text.find(['"', '\''])?;
    let quote = text[open..].chars().next()?;
    let rest = &text[open + 1..];
    Some(&rest[..rest.find(quote)?])
}

fn is_const(word: &str) -> bool {
    word.len() >= 2 && word.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

#[derive(Clone, Copy)]
enum Part {
    Column,
    Help,
    Gear,
}

fn part(child: &str) -> Found<Option<Part>> {
    match child {
        "card" => Ok(None),
        "self.rail_panels" => Ok(Some(Part::Column)),
        "tours_button" => Ok(Some(Part::Help)),
        "gear" => Ok(Some(Part::Gear)),
        _ => Err(unread(RAIL, &format!("render_rail draws {child}, which the test does not know"))),
    }
}

struct App {
    rail: String,
    gear: String,
    panels: String,
    icons: String,
}

impl App {
    fn read() -> Self {
        Self { rail: read(RAIL), gear: read(GEAR), panels: read(PANELS), icons: read(ICONS) }
    }

    fn icon_file(&self, icon: &str) -> Found<String> {
        let files = arms(&self.icons, "Icon::", ICONS)?;
        files.get(icon).and_then(|value| quoted(value)).map(str::to_string).ok_or_else(|| {
            unread(ICONS, &format!("Icon::{icon} has no file (no `Icon::{icon} => asset!(\"…\")` line)"))
        })
    }

    fn panel_icon(&self, panel: &str) -> Found<String> {
        let body = block(&self.panels, PANELS, "fn icon(self)")?;
        let icons = arms(body, "Self::", PANELS)?;
        let value =
            icons.get(panel).ok_or_else(|| unread(PANELS, &format!("PanelKind::icon gives {panel} no icon")))?;
        let icon = value.strip_prefix("Icon::").ok_or_else(|| {
            unread(PANELS, &format!("PanelKind::icon gives {panel} `{value}`, which the test cannot read"))
        })?;
        self.icon_file(icon)
    }

    fn panel_title(&self, panel: &str) -> Found<String> {
        let body = block(&self.panels, PANELS, "fn title(self)")?;
        let titles = arms(body, "Self::", PANELS)?;
        let title = titles.get(panel).and_then(|value| quoted(value));
        title.map(str::to_string).ok_or_else(|| unread(PANELS, &format!("PanelKind::title gives {panel} no title")))
    }

    fn listed(&self, name: &str) -> Found<Vec<String>> {
        let panels: Vec<&str> = declared(&self.rail, name)
            .map(words)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|word| word.strip_prefix("PanelKind::"))
            .collect();
        if panels.is_empty() {
            return Err(unread(RAIL, &format!("column reads {name}, which is no list of PanelKind there")));
        }
        panels.into_iter().map(|panel| self.panel_icon(panel)).collect()
    }

    fn word_icons(&self, word: &str) -> Found<Vec<String>> {
        if let Some(panel) = word.strip_prefix("PanelKind::") {
            return Ok(vec![self.panel_icon(panel)?]);
        }
        match word {
            "Slot::Divider" => Ok(vec![RULE.to_string()]),
            "Slot::Plugins" | "Slot::Panel" => Ok(vec![]),
            _ if word.starts_with("Slot::") => {
                Err(unread(RAIL, &format!("column draws {word}, which the test does not know")))
            }
            _ if is_const(word) => self.listed(word),
            _ if KNOWN_WORDS.contains(&word) => Ok(vec![]),
            _ => Err(unread(RAIL, &format!("column calls {word}, which the test does not know"))),
        }
    }

    fn column(&self) -> Found<Vec<String>> {
        let body = block(&self.rail, RAIL, "fn column(")?;
        let icons: Found<Vec<Vec<String>>> = words(body).into_iter().map(|word| self.word_icons(word)).collect();
        Ok(icons?.concat())
    }

    fn only_icon(&self, text: &str, file: &str, name: &str) -> Found<String> {
        let body = block(text, file, &format!("fn {name}("))?;
        let icons: Vec<&str> = words(body).into_iter().filter_map(|word| word.strip_prefix("Icon::")).collect();
        match icons.as_slice() {
            [one] => self.icon_file(one),
            _ => Err(unread(file, &format!("{name} draws {} icons, the test reads one", icons.len()))),
        }
    }

    fn part_icons(&self, part: Part) -> Found<Vec<String>> {
        match part {
            Part::Column => Ok([self.column()?, vec![GAP.to_string()]].concat()),
            Part::Help => Ok(vec![self.only_icon(&self.rail, RAIL, "tours_button")?]),
            Part::Gear => Ok(vec![self.only_icon(&self.gear, GEAR, "gear_button")?]),
        }
    }

    fn rail(&self) -> Found<Vec<String>> {
        let body: String = block(&self.rail, RAIL, "fn render_rail(")?.chars().filter(|c| !c.is_whitespace()).collect();
        if body.contains(".children(") {
            return Err(unread(RAIL, "render_rail draws .children(, which the test does not know"));
        }
        let mut sequence = Vec::new();
        for piece in body.split(".child(").skip(1) {
            let end = piece.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':')));
            if let Some(part) = part(&piece[..end.unwrap_or(piece.len())])? {
                sequence.extend(self.part_icons(part)?);
            }
        }
        self.drawn_as_read()?;
        Ok(sequence)
    }

    fn drawn_as_read(&self) -> Found<()> {
        let panels = block(&self.rail, RAIL, "fn rail_panels(")?;
        let compact: String = panels.chars().filter(|c| !c.is_whitespace()).collect();
        let piped = compact.contains("column(flow).into_iter().flat_map(") && compact.contains("}).collect()");
        if !piped || panels.contains(".child(") || panels.matches(".children(").count() != 1 {
            return Err(unread(RAIL, "rail_panels draws something besides column's slots"));
        }
        let button = block(&self.rail, RAIL, "fn rail_button(")?;
        if !button.contains("kind.icon()") || words(button).iter().any(|word| word.starts_with("Icon::")) {
            return Err(unread(RAIL, "rail_button draws a panel's icon other than kind.icon()"));
        }
        Ok(())
    }

    fn check_head(&self, file: &str, astro: &str) -> Found<()> {
        let (icon, title) = page_head(astro, file)?;
        let name = file.rsplit('/').next().unwrap_or(file);
        let panel = name.strip_prefix("Dock").and_then(|rest| rest.strip_suffix(".astro")).unwrap_or(name);
        let expected = self.panel_title(panel)?;
        if title != expected {
            return Err(format!(
                "{file}: its head says {title}; the app's {panel} panel is titled {expected} ({PANELS} PanelKind::title): change the page's title to its panel's"
            ));
        }
        let drawn = self.panel_icon(panel)?;
        if icon == drawn {
            return Ok(());
        }
        Err(format!(
            "{file}: its head draws {icon} for {title}; the app draws {drawn} ({PANELS} PanelKind::icon, as src/app.rs panel_title draws it): change the page's icon"
        ))
    }
}

fn frontmatter(astro: &str) -> Found<(&str, &str)> {
    astro
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---"))
        .ok_or_else(|| unread(PAGE_RAIL, "has no frontmatter"))
}

fn strings(text: &str) -> Vec<String> {
    text.split(['\'', '"']).skip(1).step_by(2).map(str::to_string).collect()
}

fn list(name: &str, rest: &str) -> Option<Found<(String, Vec<String>)>> {
    let body = rest.trim_start().strip_prefix('[')?.split_once(']')?.0;
    let loose = body.split(['\'', '"']).step_by(2).any(|piece| !piece.chars().all(|c| c.is_whitespace() || c == ','));
    Some(if loose {
        Err(unread(PAGE_RAIL, &format!("{} holds more than quoted names", name.trim())))
    } else {
        Ok((name.trim().to_string(), strings(body)))
    })
}

fn lists(front: &str) -> Found<HashMap<String, Vec<String>>> {
    front
        .split("const ")
        .skip(1)
        .filter_map(|declaration| {
            let (name, rest) = declaration.split_once('=')?;
            list(name, rest)
        })
        .collect()
}

fn occurrences(text: &str, needle: &str) -> Vec<usize> {
    text.match_indices(needle).map(|(at, _)| at).collect()
}

fn trailing_name(text: &str) -> &str {
    let start = text.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).map_or(0, |at| at + 1);
    &text[start..]
}

fn callback_draws_its_value(template: &str, at: usize) -> Found<()> {
    let line = template[at..].lines().next().unwrap_or_default();
    let param: String = line[".map(".len()..]
        .trim_start_matches('(')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    let draws = format!("<Icon name={{{param}}}");
    if param.is_empty() || line.matches("<Icon").count() != 1 || !line.contains(&draws) {
        return Err(unread(PAGE_RAIL, &format!("a map callback does not draw `{draws}` once, on its own line")));
    }
    Ok(())
}

fn page_rail(astro: &str) -> Found<Vec<String>> {
    let (front, template) = frontmatter(astro)?;
    let lists = lists(front)?;
    let mut marks: Vec<(usize, Vec<String>)> = Vec::new();
    for at in occurrences(template, ".map(") {
        let name = trailing_name(&template[..at]);
        let items = lists
            .get(name)
            .ok_or_else(|| unread(PAGE_RAIL, &format!("maps {name}, which its frontmatter does not declare")))?;
        callback_draws_its_value(template, at)?;
        marks.push((at, items.clone()));
    }
    for (needle, mark) in [("w-rail__rule", RULE), ("w-rail__gap", GAP)] {
        marks.extend(occurrences(template, needle).into_iter().map(|at| (at, vec![mark.to_string()])));
    }
    for at in occurrences(template, "name=\"") {
        let name = template[at + 6..].split('"').next().unwrap_or_default();
        marks.push((at, vec![name.to_string()]));
    }
    let icons = occurrences(template, "<Icon").len();
    let known = occurrences(template, ".map(").len() + occurrences(template, "name=\"").len();
    if icons != known {
        return Err(unread(PAGE_RAIL, &format!("has {icons} <Icon, but its maps and name=\"…\" account for {known}")));
    }
    marks.sort_by_key(|&(at, _)| at);
    Ok(marks.into_iter().flat_map(|(_, items)| items).collect())
}

fn page_head(astro: &str, file: &str) -> Found<(String, String)> {
    let missing = || unread(file, "no w-dockhead with an icon and a <b> title");
    let (_, after) = astro.split_once("class=\"w-dockhead\"").ok_or_else(missing)?;
    let icon = after.split_once("name=\"").and_then(|(_, rest)| rest.split_once('"')).ok_or_else(missing)?.0;
    let title = after.split_once("<b>").and_then(|(_, rest)| rest.split_once("</b>")).ok_or_else(missing)?.0;
    Ok((icon.to_string(), title.to_string()))
}

fn dock_pages() -> Vec<(String, String)> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join(STAGE);
    let entries = fs::read_dir(folder).unwrap_or_else(|why| panic!("cannot read {STAGE}: {why}"));
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.starts_with("Dock") && name.to_lowercase().ends_with(".astro"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let path = format!("{STAGE}/{name}");
            let text = read(&path);
            (path, text)
        })
        .collect()
}

fn only(side: &str, mine: &[String], other: &[String]) -> Option<String> {
    let extra: Vec<&str> = mine
        .iter()
        .filter(|item| ![RULE, GAP].contains(&item.as_str()) && !other.contains(item))
        .map(String::as_str)
        .collect();
    (!extra.is_empty()).then(|| format!("  only the {side} draws: {}", extra.join(" ")))
}

fn differ(app: &[String], page: &[String]) -> String {
    let at = app.iter().zip(page).position(|(a, p)| a != p).unwrap_or(app.len().min(page.len()));
    let item = |list: &[String]| list.get(at).map_or("nothing", String::as_str).to_string();
    let mut lines = vec![
        "the page's rail is not the app's (a flow's workspace, no plugin's button)".to_string(),
        format!("  app:  {}", app.join(" ")),
        format!("  page: {}", page.join(" ")),
    ];
    lines.extend(only("app", app, page));
    lines.extend(only("page", page, app));
    lines.push(format!("  first difference, at {}: the app draws {}, the page {}", at + 1, item(app), item(page)));
    lines.push(format!(
        "make {PAGE_RAIL} draw the app's rail (its docked, sheets and bottom lists; | is its w-rail__rule, … its w-rail__gap, design/website.md); the story lights a docked icon by its class js-rail-<name> in site/src/client/stage/index.ts and site/src/styles/stage.css, so rename those with it"
    ));
    lines.join("\n")
}

fn app_with(rail: &str, icons: &str) -> App {
    App { rail: rail.to_string(), gear: String::new(), panels: String::new(), icons: icons.to_string() }
}

#[test]
fn the_pages_rail_is_the_apps_in_a_flows_workspace_with_no_plugin() {
    let app = App::read().rail().unwrap_or_else(|why| panic!("{why}"));
    let page = page_rail(&read(PAGE_RAIL)).unwrap_or_else(|why| panic!("{why}"));
    assert!(app.iter().any(|item| ![RULE, GAP].contains(&item.as_str())), "{RAIL}: the rail draws no icon");
    assert!(app == page, "{}", differ(&app, &page));
}

#[test]
fn each_dock_page_heads_with_its_panels_icon_and_title() {
    let app = App::read();
    let pages = dock_pages();
    assert!(!pages.is_empty(), "{}", unread(STAGE, "found no Dock*.astro"));
    let failures: Vec<String> = pages.iter().filter_map(|(file, text)| app.check_head(file, text).err()).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_list_the_column_reads_but_the_rail_does_not_declare_fails() {
    let app =
        app_with("fn column(flow: bool) -> Vec<Slot> {\n    EXTRA.into_iter().map(Slot::Panel).collect()\n}\n", "");
    let why = app.column().expect_err("an undeclared list is an error");
    assert!(why.contains("column reads EXTRA"), "{why}");
}

#[test]
fn a_slot_this_test_does_not_know_fails() {
    let app = app_with("fn column(flow: bool) -> Vec<Slot> {\n    Slot::Spacer\n}\n", "");
    let why = app.column().expect_err("an unknown slot is an error");
    assert!(why.contains("Slot::Spacer"), "{why}");
}

#[test]
fn a_child_of_the_rail_this_test_does_not_know_fails() {
    let app = app_with("fn render_rail(&self) {\n    card().child(keys_button())\n}\n", "");
    let why = app.rail().expect_err("an unknown child is an error");
    assert!(why.contains("keys_button"), "{why}");
}

#[test]
fn a_children_call_in_the_rail_fails() {
    let app = app_with("fn render_rail(&self) {\n    card().child(gear).children(extra())\n}\n", "");
    let why = app.rail().expect_err("an unread call is an error");
    assert!(why.contains(".children("), "{why}");
}

#[test]
fn a_call_in_the_column_this_test_does_not_know_fails() {
    let app = app_with("fn column(flow: bool) -> Vec<Slot> {\n    machines(flow)\n}\n", "");
    let why = app.column().expect_err("an unknown call is an error");
    assert!(why.contains("column calls machines"), "{why}");
}

const SLOTS: &str = "let slots = column(flow)\n        .into_iter()\n        .flat_map(|slot| match slot {\n            Slot::Divider => vec![rule()],\n        })\n        .collect();";

fn rail_drawn(panels: &str, button: &str) -> String {
    let rail = format!(
        "fn render_rail(&self) {{\n    card()\n}}\nfn rail_panels() {{\n    {panels}\n}}\nfn rail_button() {{\n    {button}\n}}\n"
    );
    app_with(&rail, "").rail().expect_err("the rail is not drawn as the test reads it")
}

#[test]
fn a_rail_panels_that_draws_a_child_besides_the_column_fails() {
    let why = rail_drawn(&format!("{SLOTS}\ndiv().children(slots).child(machines_button())"), "kind.icon()");
    assert!(why.contains("rail_panels draws something besides column's slots"), "{why}");
}

#[test]
fn a_rail_panels_that_draws_a_second_list_of_children_fails() {
    let why = rail_drawn(&format!("{SLOTS}\ndiv().children(slots).children(extra())"), "kind.icon()");
    assert!(why.contains("rail_panels draws something besides column's slots"), "{why}");
}

#[test]
fn a_rail_panels_that_reorders_the_columns_slots_fails() {
    let before = "let slots = column(flow).into_iter().rev().flat_map(|slot| match slot {\n    Slot::Divider => vec![rule()],\n}).collect();\ndiv().children(slots)";
    let after = "let slots = column(flow).into_iter().flat_map(|slot| match slot {\n    Slot::Divider => vec![rule()],\n}).rev().collect();\ndiv().children(slots)";
    for panels in [before, after] {
        let why = rail_drawn(panels, "kind.icon()");
        assert!(why.contains("rail_panels draws something besides column's slots"), "{why}");
    }
}

#[test]
fn a_rail_button_that_draws_another_icon_than_its_panels_fails() {
    let why = rail_drawn(&format!("{SLOTS}\ndiv().children(slots)"), "Icon::Repeat");
    assert!(why.contains("rail_button draws a panel's icon other than kind.icon()"), "{why}");
}

#[test]
fn an_icon_with_no_file_fails() {
    let app = app_with("", "Icon::Search => asset!(\"search\"),\n");
    let why = app.icon_file("Help").expect_err("an icon with no arm is an error");
    assert!(why.contains("Icon::Help has no file"), "{why}");
}

#[test]
fn an_icon_the_page_names_outside_its_lists_counts_where_it_sits() {
    let astro = "---\nconst docked = ['a', 'b'];\nconst sheets = [\"c\"];\n---\n<nav>\n{docked.map((name) => <Icon name={name} />)}\n<span class=\"w-rail__rule\"></span>\n<Icon name=\"keyboard\" size={18} />\n{sheets.map((name) => <Icon name={name} />)}\n<span class=\"w-rail__gap\"></span>\n</nav>\n";
    let page = page_rail(astro).expect("the short page reads");
    assert_eq!(page, ["a", "b", RULE, "keyboard", "c", GAP]);
}

#[test]
fn an_icon_the_page_draws_in_a_form_this_test_does_not_read_fails() {
    let astro = "---\nconst docked = ['a'];\n---\n<nav>\n{docked.map((name) => <Icon name={name} />)}\n<Icon name={'keyboard'} size={18} />\n</nav>\n";
    let why = page_rail(astro).expect_err("an unread icon is an error");
    assert!(why.contains("has 2 <Icon"), "{why}");
}

#[test]
fn a_list_the_page_builds_from_more_than_quoted_names_fails() {
    let astro =
        "---\nconst docked = [...extra, 'a'];\n---\n<nav>\n{docked.map((name) => <Icon name={name} />)}\n</nav>\n";
    let why = page_rail(astro).expect_err("a spread is an error");
    assert!(why.contains("docked holds more than quoted names"), "{why}");
}

#[test]
fn a_map_callback_that_draws_another_icon_than_its_value_fails() {
    let astro = "---\nconst docked = ['a'];\n---\n<nav>\n{docked.map((name) => <Icon name={'settings'} />)}\n</nav>\n";
    let why = page_rail(astro).expect_err("a callback that ignores its value is an error");
    assert!(why.contains("a map callback does not draw `<Icon name={name}`"), "{why}");
}

#[test]
fn a_dock_page_headed_with_another_panels_heading_fails() {
    let panels = "fn icon(self) -> Icon {\n    Self::Flow => Icon::Waypoints,\n    Self::Browser => Icon::Globe,\n}\nfn title(self) -> &'static str {\n    Self::Flow => \"Flow\",\n    Self::Browser => \"Browser\",\n}\n";
    let icons = "Icon::Waypoints => asset!(\"waypoints\"),\nIcon::Globe => asset!(\"globe\"),\n";
    let app = App { rail: String::new(), gear: String::new(), panels: panels.to_string(), icons: icons.to_string() };
    let astro = "<header class=\"w-dockhead\"><Icon name=\"globe\" size={16} /><b>Browser</b></header>";
    let why = app.check_head("site/src/components/stage/DockFlow.astro", astro).expect_err("another panel's head");
    assert!(why.contains("the app's Flow panel is titled Flow"), "{why}");
}
