//! Completions and hover help in the editor, without a language server: each app gives a
//! [`Vocabulary`] (its own API, `dspk.*`, `lucem.op`, `garnish.ops.*`, and the libraries its scripts
//! use, `np.*`), and the script's own names (functions, parameters, imports, variables) are added
//! as it is typed. Instant on the desktop and in a browser alike.
//!
//! Generalised from lucem's Code workspace, which had it for `lucem`, NumPy and `scipy.ndimage`;
//! lucem now passes those as its vocabulary.

use std::collections::BTreeSet;
use std::rc::Rc;

use anyhow::Result;
use gpui_kit::component::input::{CompletionProvider, HoverProvider, Rope};
use gpui_kit::{App, Entity, Task, Window};
use kitchen_ui::widgets::CodeView;
use lsp_types::{CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, Documentation, Hover, HoverContents, MarkupContent, MarkupKind};

/// What a name is, for its icon in the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Module,
    Function,
    Method,
    Class,
    Variable,
    Constant,
    Keyword,
    /// A whole construct (a decorator with its arguments).
    Snippet,
}

impl EntryKind {
    fn lsp(self) -> CompletionItemKind {
        match self {
            EntryKind::Module => CompletionItemKind::MODULE,
            EntryKind::Function => CompletionItemKind::FUNCTION,
            EntryKind::Method => CompletionItemKind::METHOD,
            EntryKind::Class => CompletionItemKind::CLASS,
            EntryKind::Variable => CompletionItemKind::VARIABLE,
            EntryKind::Constant => CompletionItemKind::CONSTANT,
            EntryKind::Keyword => CompletionItemKind::KEYWORD,
            EntryKind::Snippet => CompletionItemKind::SNIPPET,
        }
    }
}

/// One name offered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub label: String,
    pub kind: EntryKind,
    /// One line beside it (`np.clip(a, a_min, a_max)`).
    pub detail: String,
    /// Markdown shown with it and on hover.
    pub doc: String,
    /// What is inserted, when not the label (`clip(`).
    pub insert: Option<String>,
}

impl Entry {
    pub fn new(label: &str, kind: EntryKind, detail: &str, doc: &str) -> Self {
        Self { label: label.into(), kind, detail: detail.into(), doc: doc.into(), insert: None }
    }

    /// Inserts `text` instead of the label.
    pub fn inserting(mut self, text: &str) -> Self {
        self.insert = Some(text.into());
        self
    }

    fn item(&self) -> CompletionItem {
        CompletionItem {
            label: self.label.clone(),
            kind: Some(self.kind.lsp()),
            detail: (!self.detail.is_empty()).then(|| self.detail.clone()),
            documentation: (!self.doc.is_empty()).then(|| Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown, value: self.doc.clone() })),
            insert_text: self.insert.clone(),
            ..Default::default()
        }
    }
}

/// What an app's scripts can name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Vocabulary {
    /// Offered at the start of a name (modules, functions, keywords).
    pub top: Vec<Entry>,
    /// Offered after `qualifier.`: each list with the qualifiers that lead to it
    /// (`["np", "numpy"]`).
    pub members: Vec<(Vec<String>, Vec<Entry>)>,
    /// Offered after `@`.
    pub decorators: Vec<Entry>,
    /// Offered after any other `name.` (an array's methods, a table's).
    pub any_member: Vec<Entry>,
}

impl Vocabulary {
    /// Python's keywords and the builtins a script uses most.
    pub fn python() -> Self {
        let kw = |k: &str, doc: &str| Entry::new(k, EntryKind::Keyword, "keyword", doc);
        let builtin = |b: &str, doc: &str| Entry::new(b, EntryKind::Class, "builtin", doc);
        Self {
            top: vec![
                kw("def", "Defines a function."),
                kw("return", "Returns from a function."),
                kw("import", "Imports a module."),
                kw("from", "Imports names from a module."),
                kw("for", "Loops over items."),
                kw("while", "Loops while a condition holds."),
                kw("if", "Runs a block when a condition holds."),
                kw("with", "Uses a context manager."),
                kw("class", "Defines a class."),
                builtin("float", "Floating-point number."),
                builtin("int", "Integer."),
                builtin("bool", "True or False."),
                builtin("str", "Text."),
                builtin("list", "A list."),
                builtin("dict", "A mapping."),
                Entry::new("print", EntryKind::Function, "print(*values)", "Writes to the run panel; the last line, when JSON, is the script's result.").inserting("print("),
                Entry::new("len", EntryKind::Function, "len(x)", "The number of items.").inserting("len("),
                Entry::new("range", EntryKind::Function, "range(start, stop, step)", "Numbers from start up to stop.").inserting("range("),
            ],
            ..Default::default()
        }
    }

    /// `other`'s names added to these.
    pub fn with(mut self, other: Vocabulary) -> Self {
        self.top.extend(other.top);
        self.members.extend(other.members);
        self.decorators.extend(other.decorators);
        self.any_member.extend(other.any_member);
        self
    }

    /// The names offered at `offset` of `src`, the script's own names included.
    pub fn complete(&self, src: &str, offset: usize) -> Vec<Entry> {
        let (qualifier, prefix) = prefix_context(src, offset);
        let mut out: Vec<Entry> = match qualifier {
            "" => {
                let mut list = doc_symbols(src);
                list.extend(self.top.iter().cloned());
                list
            }
            "@" => self.decorators.clone(),
            q => self.members.iter().find(|(names, _)| names.iter().any(|n| n == q)).map_or_else(|| self.any_member.clone(), |(_, list)| list.clone()),
        };
        let prefix = prefix.to_ascii_lowercase();
        if !prefix.is_empty() {
            out.retain(|e| e.label.to_ascii_lowercase().starts_with(&prefix));
        }
        out
    }

    /// The help of the name at `offset` (`np.clip`, `clip`, `@lucem.op`).
    pub fn hover(&self, src: &str, offset: usize) -> Option<String> {
        let word = word_at(src, offset);
        let (qualifier, name) = word.rsplit_once('.').map_or(("", word.as_str()), |(q, n)| (q, n));
        let in_members = || {
            self.members
                .iter()
                .filter(|(names, _)| qualifier.is_empty() || names.iter().any(|n| n == qualifier))
                .flat_map(|(_, list)| list)
                .chain(&self.any_member)
                .find(|e| e.label == name)
        };
        let found = self.decorators.iter().find(|e| e.label == word).or_else(|| self.top.iter().find(|e| e.label == word)).or_else(in_members);
        found.filter(|e| !e.doc.is_empty()).map(|e| format!("**`{}`**\n\n{}", if e.detail.is_empty() { &e.label } else { &e.detail }, e.doc))
    }
}

/// Gives `view`'s editor completions and hover help from `vocabulary`.
pub fn attach(view: &Entity<CodeView>, vocabulary: Rc<Vocabulary>, cx: &mut App) {
    let editor = view.read(cx).editor().clone();
    let provider = Rc::new(Provider(vocabulary));
    editor.update(cx, |ed, _| {
        let lsp = ed.lsp_mut();
        lsp.completion_provider = Some(provider.clone());
        lsp.hover_provider = Some(provider);
    });
}

struct Provider(Rc<Vocabulary>);

impl CompletionProvider for Provider {
    fn completions(&self, text: &Rope, offset: usize, _trigger: CompletionContext, _window: &mut Window, _cx: &mut App) -> Task<Result<CompletionResponse>> {
        let items = self.0.complete(&text.to_string(), offset).iter().map(Entry::item).collect();
        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut App) -> bool {
        matches!(new_text, "." | "@") || new_text.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    }
}

impl HoverProvider for Provider {
    fn hover(&self, text: &Rope, offset: usize, _window: &mut Window, _cx: &mut App) -> Task<Result<Option<Hover>>> {
        let hover = self.0.hover(&text.to_string(), offset).map(|value| Hover { contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value }), range: None });
        Task::ready(Ok(hover))
    }
}

/// The script's own names: functions and their parameters, imports, assigned variables.
fn doc_symbols(src: &str) -> Vec<Entry> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut add = |name: &str, kind: EntryKind, detail: &str, out: &mut Vec<Entry>| {
        if is_ident(name) && seen.insert(name.to_string()) {
            out.push(Entry::new(name, kind, detail, ""));
        }
    };
    for line in src.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("def ").or_else(|| t.strip_prefix("fn ")).or_else(|| t.strip_prefix("pub fn ")) {
            if let Some(name) = rest.split(['(', '<']).next() {
                add(name.trim(), EntryKind::Function, "in this script", &mut out);
            }
            if let (Some(open), Some(close)) = (rest.find('('), rest.rfind(')')) {
                for arg in rest[open + 1..close].split(',') {
                    let param = arg.split([':', '=']).next().unwrap_or(arg).trim().trim_start_matches("mut ");
                    add(param, EntryKind::Variable, "parameter", &mut out);
                }
            }
        } else if t.starts_with("import ") || t.starts_with("from ") {
            if let Some((_, alias)) = t.rsplit_once(" as ") {
                add(alias.trim(), EntryKind::Module, "import", &mut out);
            } else if let Some((_, names)) = t.split_once(" import ") {
                for name in names.split(',') {
                    add(name.trim(), EntryKind::Function, "import", &mut out);
                }
            } else if let Some(module) = t.strip_prefix("import ") {
                for name in module.split(',') {
                    add(name.trim().split('.').next().unwrap_or(""), EntryKind::Module, "import", &mut out);
                }
            }
        } else if let Some(rest) = t.strip_prefix("let ") {
            let name = rest.trim_start_matches("mut ").split([':', '=', ' ']).next().unwrap_or("");
            add(name, EntryKind::Variable, "local", &mut out);
        } else if let Some((lhs, _)) = t.split_once('=')
            && !lhs.contains(['(', '[', '!', '<', '>'])
            && !t.starts_with(['#', '@', '/'])
        {
            add(lhs.split(':').next().unwrap_or(lhs).trim(), EntryKind::Variable, "local", &mut out);
        }
    }
    out
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The qualifier before the name being typed (`np` in `np.cl`, `@` after `@`) and the name so far.
fn prefix_context(src: &str, offset: usize) -> (&str, &str) {
    let before = &src[..floor_char(src, offset)];
    let line = &before[before.rfind('\n').map_or(0, |i| i + 1)..];
    let start = line.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).map_or(0, |i| i + 1);
    let (head, prefix) = line.split_at(start);
    if let Some(before_dot) = head.strip_suffix('.') {
        let q = before_dot.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.')).map_or(0, |i| i + 1);
        return (&before_dot[q..], prefix);
    }
    if head.ends_with('@') {
        return ("@", prefix);
    }
    ("", prefix)
}

/// The dotted name around `offset`.
fn word_at(src: &str, offset: usize) -> String {
    let offset = floor_char(src, offset);
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.';
    let start = src[..offset].rfind(|c: char| !is_word(c)).map_or(0, |i| i + 1);
    let end = src[offset..].find(|c: char| !is_word(c)).map_or(src.len(), |i| offset + i);
    src[start..end].trim_matches('.').to_string()
}

/// `offset` moved back to a character boundary (and into the text).
fn floor_char(src: &str, offset: usize) -> usize {
    let mut o = offset.min(src.len());
    while !src.is_char_boundary(o) {
        o -= 1;
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numpy() -> Vocabulary {
        Vocabulary {
            members: vec![(
                vec!["np".into(), "numpy".into()],
                vec![
                    Entry::new("clip", EntryKind::Function, "np.clip(a, a_min, a_max)", "Clamps values.").inserting("clip("),
                    Entry::new("mean", EntryKind::Function, "np.mean(a)", "The mean."),
                ],
            )],
            decorators: vec![Entry::new("dspk.op", EntryKind::Snippet, "@dspk.op(family=\"Filter\")", "A pipeline operation.")],
            ..Default::default()
        }
    }

    #[test]
    fn completes_members_decorators_and_the_scripts_names() {
        let v = Vocabulary::python().with(numpy());
        let src = "import numpy as np\ndef spikes(trace, threshold=4.5):\n    peaks = np.cl";
        let labels = |es: Vec<Entry>| es.into_iter().map(|e| e.label).collect::<Vec<_>>();
        assert_eq!(labels(v.complete(src, src.len())), ["clip"]);
        let src2 = "import numpy as np\ndef spikes(trace, threshold=4.5):\n    th";
        assert_eq!(labels(v.complete(src2, src2.len())), ["threshold"]);
        let src3 = "@ds";
        assert_eq!(labels(v.complete(src3, src3.len())), ["dspk.op"]);
        assert!(v.hover("x = np.clip(a, 0, 1)", 7).unwrap().contains("Clamps values"));
        assert!(v.hover("@dspk.op", 3).unwrap().contains("pipeline operation"));
    }
}
