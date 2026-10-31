use std::collections::HashMap;
use std::rc::Rc;

use gpui::{ListOffset, ListState};

use super::data::ReadIssue;
use super::markdown::{self, Block};
use super::{BodyView, IssueId, virtual_list};

pub(super) type Read = Result<Rc<Page>, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Item {
    Heading,
    Empty,
    Block(usize),
    Source,
    Comments,
    Comment(usize),
}

struct Text {
    items: Vec<Item>,
    list: ListState,
}

impl Text {
    fn new(items: Vec<Item>) -> Self {
        let list = virtual_list(items.len());
        Self { items, list }
    }
}

pub(super) struct Page {
    pub read: ReadIssue,
    pub body: Vec<Block>,
    pub comments: Vec<Vec<Block>>,
    rendered: Text,
    markdown: Text,
}

impl Page {
    pub(super) fn new(read: ReadIssue) -> Self {
        let body = markdown::parse(&read.body);
        let comments = read.comments.iter().map(|c| markdown::parse(&c.body)).collect();
        let rendered = Text::new(items(&read, BodyView::Rendered, body.len()));
        let markdown = Text::new(items(&read, BodyView::Markdown, body.len()));
        Self { read, body, comments, rendered, markdown }
    }

    fn text(&self, view: BodyView) -> &Text {
        match view {
            BodyView::Rendered => &self.rendered,
            BodyView::Markdown => &self.markdown,
        }
    }

    pub(super) fn items(&self, view: BodyView) -> &[Item] {
        &self.text(view).items
    }

    pub(super) fn list(&self, view: BodyView) -> ListState {
        self.text(view).list.clone()
    }

    pub(super) fn to_top(&self) {
        for text in [&self.rendered, &self.markdown] {
            text.list.scroll_to(ListOffset::default());
        }
    }
}

pub(super) fn take(reads: &mut HashMap<IssueId, Read>, id: IssueId, read: Result<ReadIssue, String>) {
    if let (Some(Ok(page)), Ok(read)) = (reads.get(&id), &read)
        && page.read == *read
    {
        return;
    }
    reads.insert(id, read.map(|read| Rc::new(Page::new(read))));
}

fn items(read: &ReadIssue, view: BodyView, blocks: usize) -> Vec<Item> {
    let mut items = vec![Item::Heading];
    match view {
        _ if read.body.trim().is_empty() => items.push(Item::Empty),
        BodyView::Rendered => items.extend((0..blocks).map(Item::Block)),
        BodyView::Markdown => items.push(Item::Source),
    }
    if !read.comments.is_empty() {
        items.push(Item::Comments);
        items.extend((0..read.comments.len()).map(Item::Comment));
    }
    items
}

#[cfg(test)]
mod tests;
