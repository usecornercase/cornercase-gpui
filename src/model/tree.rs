use std::collections::HashMap;
use std::hash::Hash;
use std::mem;

#[derive(Debug, Clone, Copy)]
pub(super) struct Item<K> {
    pub key: K,
    pub parent: Option<K>,
    pub folded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Node {
    pub at: usize,
    pub depth: usize,
    pub children: usize,
    pub hidden: Vec<usize>,
}

struct Walk {
    kids: Vec<Vec<usize>>,
    folded: Vec<bool>,
    visited: Vec<bool>,
    nodes: Vec<Node>,
}

impl Walk {
    fn first_visit(&mut self, at: usize) -> bool {
        !mem::replace(&mut self.visited[at], true)
    }

    fn visit(&mut self, at: usize, depth: usize) {
        if !self.first_visit(at) {
            return;
        }
        let kids: Vec<usize> = self.kids[at].iter().copied().filter(|kid| !self.visited[*kid]).collect();
        let node = self.nodes.len();
        self.nodes.push(Node { at, depth, children: kids.len(), hidden: Vec::new() });
        if self.folded[at] {
            self.nodes[node].hidden = self.hide(kids);
            return;
        }
        for kid in kids {
            self.visit(kid, depth + 1);
        }
    }

    fn hide(&mut self, kids: Vec<usize>) -> Vec<usize> {
        let mut hidden = Vec::new();
        let mut waiting = kids;
        while let Some(at) = waiting.pop() {
            if self.first_visit(at) {
                hidden.push(at);
                waiting.extend(self.kids[at].iter().copied());
            }
        }
        hidden
    }
}

pub(super) fn order<K: Copy + Eq + Hash>(items: &[Item<K>]) -> Vec<Node> {
    let known: HashMap<K, usize> = items.iter().enumerate().rev().map(|(at, item)| (item.key, at)).collect();
    let mut kids = vec![Vec::new(); items.len()];
    let mut roots = Vec::new();
    for (at, item) in items.iter().enumerate() {
        match item.parent.filter(|parent| *parent != item.key).and_then(|parent| known.get(&parent)) {
            Some(parent) => kids[*parent].push(at),
            None => roots.push(at),
        }
    }
    let folded = items.iter().map(|item| item.folded).collect();
    let mut walk = Walk { kids, folded, visited: vec![false; items.len()], nodes: Vec::new() };
    for at in roots.into_iter().chain(0..items.len()) {
        walk.visit(at, 0);
    }
    walk.nodes
}

#[cfg(test)]
mod tests;
