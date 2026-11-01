use gpui::px;

use super::*;
use crate::panels::issues::data::Comment;

const VIEWS: [BodyView; 2] = [BodyView::Rendered, BodyView::Markdown];

fn read(body: &str, comments: &[&str]) -> ReadIssue {
    let comments = comments.iter().map(|b| Comment { body: (*b).to_string(), ..Comment::default() }).collect();
    ReadIssue { body: body.to_string(), comments, ..ReadIssue::default() }
}

fn page(body: &str, comments: &[&str]) -> Page {
    Page::new(read(body, comments))
}

fn id() -> IssueId {
    ("github".to_string(), "#1".to_string())
}

fn page_of(reads: &HashMap<IssueId, Read>) -> Rc<Page> {
    let Some(Ok(page)) = reads.get(&id()) else { panic!("the issue has no page") };
    page.clone()
}

#[test]
fn each_block_of_the_description_is_an_item() {
    let page = page("# Steps\n\nOpen it.\n\n```\nlog\n```", &[]);
    assert_eq!(page.items(BodyView::Rendered), [Item::Heading, Item::Block(0), Item::Block(1), Item::Block(2)]);
}

#[test]
fn the_markdown_view_shows_the_description_as_one_source() {
    let page = page("# Steps\n\nOpen it.", &[]);
    assert_eq!(page.items(BodyView::Markdown), [Item::Heading, Item::Source]);
}

#[test]
fn comments_follow_their_heading_in_both_views() {
    let page = page("Text", &["one", "two"]);
    for view in VIEWS {
        assert_eq!(page.items(view)[2..], [Item::Comments, Item::Comment(0), Item::Comment(1)]);
    }
    assert_eq!(page.comments, [markdown::parse("one"), markdown::parse("two")]);
}

#[test]
fn an_empty_description_says_so_in_both_views() {
    let page = page(" \n", &[]);
    for view in VIEWS {
        assert_eq!(page.items(view), [Item::Heading, Item::Empty]);
    }
}

#[test]
fn each_view_scrolls_a_list_as_long_as_its_items() {
    let page = page("a\n\nb\n\nc", &["d"]);
    for view in VIEWS {
        assert_eq!(page.list(view).item_count(), page.items(view).len());
    }
}

#[test]
fn going_to_the_top_scrolls_both_views() {
    let page = page("a\n\nb\n\nc", &[]);
    for view in VIEWS {
        page.list(view).scroll_to(ListOffset { item_ix: 2, offset_in_item: px(5.) });
    }
    page.to_top();
    for view in VIEWS {
        let top = page.list(view).logical_scroll_top();
        assert_eq!((top.item_ix, top.offset_in_item), (0, px(0.)));
    }
}

#[test]
fn the_same_answer_keeps_the_page_and_its_scroll() {
    let mut reads = HashMap::new();
    take(&mut reads, Some(1), (1, id()), Ok(read("text", &["one"])));
    let first = page_of(&reads);
    take(&mut reads, Some(1), (1, id()), Ok(read("text", &["one"])));
    assert!(Rc::ptr_eq(&first, &page_of(&reads)));
}

#[test]
fn a_new_answer_replaces_the_page() {
    let mut reads = HashMap::new();
    take(&mut reads, Some(1), (1, id()), Ok(read("text", &[])));
    let first = page_of(&reads);
    take(&mut reads, Some(1), (1, id()), Ok(read("text", &["a new comment"])));
    assert!(!Rc::ptr_eq(&first, &page_of(&reads)));
    assert_eq!(page_of(&reads).read.comments.len(), 1);
}

#[test]
fn a_failed_answer_is_shown() {
    let mut reads = HashMap::new();
    take(&mut reads, Some(1), (1, id()), Ok(read("text", &[])));
    take(&mut reads, Some(1), (1, id()), Err("gh: not found".to_string()));
    assert!(matches!(reads.get(&id()), Some(Err(e)) if e == "gh: not found"));
}

#[test]
fn an_answer_for_a_project_no_longer_shown_is_dropped() {
    let mut reads = HashMap::new();
    take(&mut reads, Some(2), (1, id()), Ok(read("another repository's #1", &[])));
    assert!(reads.is_empty());
    take(&mut reads, None, (1, id()), Ok(read("text", &[])));
    assert!(reads.is_empty());
}
