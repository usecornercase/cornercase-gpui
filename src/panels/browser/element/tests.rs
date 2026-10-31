use serde_json::json;

use super::*;

fn button() -> Element {
    Element::parse(json!({
        "url": "http://localhost:5173/login",
        "tag": "button",
        "selector": "form#login > button.primary",
        "id": "",
        "classes": ["primary"],
        "text": "Sign   in",
        "attributes": [["type", "submit"], ["aria-label", "Sign in"]],
        "html": "<button class=\"primary\" type=\"submit\">\n  Sign in\n</button>",
        "rect": { "x": 40, "y": 200, "width": 120.4, "height": 36 },
        "page": { "x": 0, "y": 0, "width": 800, "height": 600 },
        "styles": [["color", "rgb(255, 255, 255)"], ["display", "inline-flex"]],
        "component": {
            "framework": "React",
            "name": "LoginButton",
            "file": "src/components/LoginButton.tsx",
            "line": 42,
            "owners": ["App", "LoginForm", "LoginButton"]
        }
    }))
    .expect("the element parses")
}

#[test]
fn an_answer_without_a_tag_is_no_element() {
    assert_eq!(Element::parse(json!({ "selector": "div" })), None);
    assert_eq!(Element::parse(json!(null)), None);
}

#[test]
fn the_heading_names_the_tag_and_its_selector() {
    assert_eq!(button().heading(), "<button> form#login > button.primary");
}

#[test]
fn the_detail_prefers_the_component_and_its_source() {
    assert_eq!(button().detail().as_deref(), Some("LoginButton · src/components/LoginButton.tsx:42"));
}

#[test]
fn without_a_component_the_detail_quotes_the_text() {
    let element = Element { component: None, ..button() };
    assert_eq!(element.detail().as_deref(), Some("“Sign in”"));
}

#[test]
fn a_place_read_from_a_stack_is_only_near() {
    let component =
        Component { file: Some("src/App.tsx".into()), line: Some(9), approximate: true, ..Component::default() };
    assert_eq!(component.place().as_deref(), Some("near src/App.tsx:9"));
}

#[test]
fn the_brief_carries_what_an_agent_needs_to_find_the_element() {
    let brief = button().brief(true);
    let lines: Vec<&str> = brief.lines().collect();
    assert_eq!(lines[0], "I picked this element in the browser (the image is a crop of it):");
    assert_eq!(lines[1], "Page: http://localhost:5173/login");
    assert_eq!(lines[2], "Element: <button> form#login > button.primary");
    assert_eq!(
        lines[3],
        "Component: LoginButton, src/components/LoginButton.tsx:42 (React), inside App > LoginForm > LoginButton"
    );
    assert_eq!(lines[4], "Text: \"Sign in\"");
    assert_eq!(lines[5], "Attributes: type=\"submit\", aria-label=\"Sign in\"");
    assert_eq!(lines[6], "Styles: color rgb(255, 255, 255); display inline-flex; size 120×36");
    assert_eq!(lines[7], "HTML: <button class=\"primary\" type=\"submit\"> Sign in </button>");
}

#[test]
fn without_a_crop_the_brief_does_not_mention_an_image() {
    assert!(!button().brief(false).contains("image"));
}

#[test]
fn a_huge_element_keeps_the_brief_bounded() {
    let element = Element { html: "<div>".repeat(5000), text: "word ".repeat(5000), ..button() };
    let brief = element.brief(false);
    assert!(brief.chars().count() <= 4000);
    assert!(brief.contains("Text: \"word word"));
}

#[test]
fn a_cut_text_ends_in_an_ellipsis() {
    assert_eq!(bounded("one two three", 8), "one two…");
    assert_eq!(bounded("short", 8), "short");
}
