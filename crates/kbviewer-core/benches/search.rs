//! The search path over a vault shaped like the one it serves: a few hundred notes of
//! prose with frontmatter tags, headings and wikilinks, in nested folders, about 1 MB of
//! text in all.
//!
//! The corpus is generated from a fixed seed rather than checked in, so every run measures
//! the same bytes. Each query goes through `kbviewer_core::search::search`, the call
//! `/api/search` makes, so the bench survives a rewrite of whatever sits behind it.
//!
//! Run with `make bench` (or `cargo bench -p kbviewer-core --bench search`).

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use kbviewer_core::config::RootConfig;
use kbviewer_core::index::Index;
use kbviewer_core::model::SearchHit;
use kbviewer_core::search::search;

/// The page size `/api/search` uses when the client names none.
const RESULT_LIMIT: usize = 40;
const NOTE_COUNT: usize = 400;
const PARAGRAPHS_PER_NOTE: usize = 6;
const WORDS_PER_PARAGRAPH: usize = 60;
const TAGS_PER_NOTE: usize = 2;
/// Fixed so that every run indexes the same corpus.
const CORPUS_SEED: u64 = 0x6b62_7669_6577_6572;
/// Knuth's MMIX linear congruential generator: deterministic, and cheap next to the I/O.
const LCG_MULTIPLIER: u64 = 6_364_136_223_846_793_005;
const LCG_INCREMENT: u64 = 1_442_695_040_888_963_407;
const LCG_OUTPUT_SHIFT: u32 = 33;

const FOLDERS: [&str; 5] = [
    "projects",
    "meetings",
    "reading",
    "journal/2026",
    "reference",
];
const TITLE_STEMS: [&str; 8] = [
    "Meeting Notes",
    "Release Plan",
    "Reading List",
    "Design Review",
    "Weekly Journal",
    "知识库 索引",
    "Glossary",
    "Retrospective",
];
const TAGS: [&str; 6] = ["project", "reading", "meeting", "idea", "reference", "日记"];
const WORDS: [&str; 32] = [
    "the", "index", "note", "vault", "release", "plan", "review", "latency", "budget", "folder",
    "link", "search", "server", "client", "render", "cache", "draft", "owner", "deadline",
    "meeting", "systems", "design", "知识", "笔记", "检索", "and", "of", "with", "before", "after",
    "because", "tomorrow",
];

/// Queries a person actually types: a word in many titles, a phrase, a word only the
/// bodies hold, CJK text, and a miss that must scan everything to find nothing.
const QUERIES: [&str; 5] = [
    "meeting",
    "release plan",
    "latency",
    "知识库",
    "no-such-word",
];

static VAULT: LazyLock<Index> = LazyLock::new(generated_vault);

fn main() {
    divan::main();
}

#[divan::bench(args = QUERIES)]
fn search_vault(bencher: divan::Bencher, query: &str) {
    // Built before the clock starts: writing and indexing the corpus is not the search.
    let vault = LazyLock::force(&VAULT);
    bencher.bench(|| -> Vec<SearchHit> { search(vault, query, RESULT_LIMIT) });
}

#[expect(
    clippy::expect_used,
    reason = "bench setup: without its corpus there is nothing to measure"
)]
fn generated_vault() -> Index {
    let root = std::env::temp_dir().join("kbviewer-bench-search");
    write_corpus(&root).expect("write the generated vault");
    Index::build(&vault_config(root))
}

fn vault_config(path: PathBuf) -> RootConfig {
    RootConfig {
        id: "bench".into(),
        name: "bench".into(),
        path,
        index_names: vec!["index.md".into()],
        wikilinks: Some(true),
        folder_notes: false,
        read_only: false,
    }
}

fn write_corpus(root: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(root) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let mut words = WordStream::new(CORPUS_SEED);
    for number in 0..NOTE_COUNT {
        let path = root.join(note_path(number));
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder)?;
        }
        std::fs::write(path, note_text(number, &mut words))?;
    }
    Ok(())
}

fn note_title(number: usize) -> String {
    format!("{} {number}", TITLE_STEMS[number % TITLE_STEMS.len()])
}

fn note_path(number: usize) -> String {
    format!(
        "{}/{}.md",
        FOLDERS[number % FOLDERS.len()],
        note_title(number)
    )
}

fn note_text(number: usize, words: &mut WordStream) -> String {
    let tags = words.several(&TAGS, TAGS_PER_NOTE);
    let mut text = format!(
        "---\ntags: [{}]\n---\n\n# {}\n",
        tags.join(", "),
        note_title(number)
    );
    for paragraph in 0..PARAGRAPHS_PER_NOTE {
        let linked = note_title(words.below(NOTE_COUNT));
        let prose = words.several(&WORDS, WORDS_PER_PARAGRAPH);
        text.push_str(&format!(
            "\n## Section {paragraph}\n\n{} See [[{linked}]].\n",
            prose.join(" ")
        ));
    }
    text
}

/// A deterministic stream of choices, so the corpus is the same on every run.
struct WordStream {
    state: u64,
}

impl WordStream {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn below(&mut self, bound: usize) -> usize {
        self.state = self
            .state
            .wrapping_mul(LCG_MULTIPLIER)
            .wrapping_add(LCG_INCREMENT);
        (self.state >> LCG_OUTPUT_SHIFT) as usize % bound
    }

    fn pick<'a>(&mut self, choices: &[&'a str]) -> &'a str {
        choices[self.below(choices.len())]
    }

    fn several<'a>(&mut self, choices: &[&'a str], count: usize) -> Vec<&'a str> {
        std::iter::repeat_with(|| self.pick(choices))
            .take(count)
            .collect()
    }
}
