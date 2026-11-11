//! The interface in several languages. Texts are written in French in the code, inside
//! `t!("…")`; each other language has a file in `lang/` that gives, under each French
//! line, `= its translation`. A text missing from a file shows in French.

use std::collections::HashMap;
use std::fmt::{Display, Write};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

/// `t!("Bonjour")` is the text in the chosen language; `t!("{} fichiers", n)` fills the
/// `{}` in order (or `{0}`, `{1}` by position, when a language changes the order).
#[macro_export]
macro_rules! t {
    ($s:literal) => {
        $crate::i18n::tr($s)
    };
    ($s:literal, $($a:expr),+ $(,)?) => {
        $crate::i18n::fill($crate::i18n::tr($s), &[$(&$a as &dyn std::fmt::Display),+])
    };
}

pub struct Lang {
    pub code: &'static str,
    pub name: &'static str,
    source: &'static str,
}

pub const LANGS: &[Lang] = &[
    Lang { code: "fr", name: "Français", source: "" },
    Lang { code: "en", name: "English", source: include_str!("../lang/en.txt") },
    Lang { code: "es", name: "Español", source: include_str!("../lang/es.txt") },
    Lang { code: "de", name: "Deutsch", source: include_str!("../lang/de.txt") },
    Lang { code: "it", name: "Italiano", source: include_str!("../lang/it.txt") },
    Lang { code: "pt", name: "Português (Brasil)", source: include_str!("../lang/pt.txt") },
    Lang { code: "nl", name: "Nederlands", source: include_str!("../lang/nl.txt") },
    Lang { code: "pl", name: "Polski", source: include_str!("../lang/pl.txt") },
    Lang { code: "uk", name: "Українська", source: include_str!("../lang/uk.txt") },
];

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static TABLES: OnceLock<Vec<HashMap<String, &'static str>>> = OnceLock::new();

/// `\n` in a language file stands for a line break.
fn unescape(s: &str) -> String {
    s.replace("\\n", "\n")
}

fn parse(src: &str) -> HashMap<String, &'static str> {
    let mut map = HashMap::new();
    let mut key: Option<String> = None;
    for line in src.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(v) = line.strip_prefix('=') {
            if let Some(k) = key.take() {
                let v = v.strip_prefix(' ').unwrap_or(v);
                map.insert(k, &*Box::leak(unescape(v).into_boxed_str()));
            }
        } else {
            key = Some(unescape(line));
        }
    }
    map
}

fn tables() -> &'static [HashMap<String, &'static str>] {
    TABLES.get_or_init(|| LANGS.iter().map(|l| parse(l.source)).collect())
}

/// The text in the chosen language, the French one when it has no translation.
pub fn tr(fr: &'static str) -> &'static str {
    match CURRENT.load(Ordering::Relaxed) {
        0 => fr,
        i => tables()[i].get(fr).copied().unwrap_or(fr),
    }
}

/// Puts the values in place of `{}` (in order) or `{0}`, `{1}`… (by position).
pub fn fill(pattern: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(pattern.len() + 16);
    let mut next = 0;
    let mut rest = pattern;
    while let Some(p) = rest.find('{') {
        out.push_str(&rest[..p]);
        let after = &rest[p + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[p..]);
            rest = "";
            break;
        };
        let inner = &after[..end];
        let index = if inner.is_empty() {
            next += 1;
            Some(next - 1)
        } else {
            inner.parse::<usize>().ok()
        };
        match index.and_then(|i| args.get(i)) {
            Some(a) => {
                let _ = write!(out, "{a}");
            }
            None => out.push_str(&rest[p..p + end + 2]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

pub fn current() -> usize {
    CURRENT.load(Ordering::Relaxed)
}

/// Switches to the language `code` ("en", "fr"…). Returns false when it is not offered.
pub fn set(code: &str) -> bool {
    match LANGS.iter().position(|l| l.code == code) {
        Some(i) => {
            CURRENT.store(i, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

/// The language of the system when it is one of ours, English otherwise.
pub fn system_language() -> &'static str {
    let found = crate::sys::ui_language().map(|l| l.to_lowercase());
    found.as_deref().and_then(|l| LANGS.iter().find(|x| l.starts_with(x.code))).map_or("en", |x| x.code)
}

/// `one` or `many` (both already translated), by the rule of the chosen language
/// (in French and Brazilian Portuguese, 0 and 1 are singular). Polish and Ukrainian have a
/// third form, after 2, 3, 4, 22…: their `many` is written `few|many` ("pliki|plików").
pub fn plural<'a>(n: u64, one: &'a str, many: &'a str) -> &'a str {
    plural_in(LANGS[current()].code, n, one, many)
}

fn plural_in<'a>(code: &str, n: u64, one: &'a str, many: &'a str) -> &'a str {
    let (few, many) = many.split_once('|').unwrap_or((many, many));
    let few_ok = (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100));
    match code {
        "fr" | "pt" => {
            if n <= 1 {
                one
            } else {
                many
            }
        }
        "uk" if n % 10 == 1 && n % 100 != 11 => one,
        "uk" | "pl" if few_ok => few,
        "uk" => many,
        _ if n == 1 => one,
        _ => many,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `t!("…")` text written in the code, as the program sees it.
    fn texts_in_code() -> Vec<String> {
        let mut out = Vec::new();
        let mut dirs = vec![std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))];
        while let Some(d) = dirs.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("i18n.rs") {
                    collect(&std::fs::read_to_string(&p).unwrap(), &mut out);
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    fn collect(src: &str, out: &mut Vec<String>) {
        let b = src.as_bytes();
        let mut i = 0;
        while let Some(at) = src[i..].find("t!(") {
            let start = i + at;
            i = start + 3;
            if start > 0 && (b[start - 1].is_ascii_alphanumeric() || b[start - 1] == b'_') {
                continue; // print!(, assert!(…
            }
            // the text may start on the next line
            let skip = src[i..].len() - src[i..].trim_start().len();
            if !src[i + skip..].starts_with('"') {
                continue;
            }
            i += skip + 1;
            let line_start = src[..start].rfind('\n').map_or(0, |p| p + 1);
            if src[line_start..start].trim_start().starts_with("//") {
                continue; // an example in a comment
            }
            let mut s = String::new();
            let mut chars = src[i..].char_indices();
            while let Some((k, c)) = chars.next() {
                match c {
                    '"' => {
                        i += k + 1;
                        break;
                    }
                    '\\' => match chars.next().map(|x| x.1) {
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some('"') => s.push('"'),
                        Some('\\') => s.push('\\'),
                        Some('\'') => s.push('\''),
                        Some('\n') => {
                            // a line continued: the next line's indent is dropped
                            let rest = chars.as_str();
                            let skip = rest.len() - rest.trim_start().len();
                            for _ in 0..rest[..skip].chars().count() {
                                chars.next();
                            }
                        }
                        Some(other) => s.push(other),
                        None => {}
                    },
                    c => s.push(c),
                }
            }
            out.push(s);
        }
    }

    /// The places a text asks for: `{}` count in order, `{0}` by position.
    fn holes(s: &str) -> Vec<usize> {
        let mut v = Vec::new();
        let mut next = 0;
        let mut rest = s;
        while let Some(p) = rest.find('{') {
            let after = &rest[p + 1..];
            let Some(end) = after.find('}') else { break };
            let inner = &after[..end];
            if inner.is_empty() {
                v.push(next);
                next += 1;
            } else if let Ok(i) = inner.parse() {
                v.push(i);
            }
            rest = &after[end + 1..];
        }
        v.sort();
        v.dedup();
        v
    }

    #[test]
    fn every_language_has_every_text() {
        let texts = texts_in_code();
        assert!(texts.len() > 100, "only {} texts found", texts.len());
        let mut problems = Vec::new();
        for (lang, table) in LANGS.iter().zip(tables()).skip(1) {
            for fr in &texts {
                match table.get(fr) {
                    None => problems.push(format!("{}: missing\n{}\n", lang.code, fr.replace('\n', "\\n"))),
                    Some(tr) if holes(tr) != holes(fr) => problems.push(format!("{}: wrong {{}} in {fr:?} → {tr:?}", lang.code)),
                    _ => {}
                }
            }
            for k in table.keys() {
                if !texts.contains(k) {
                    problems.push(format!("{}: no longer used: {k:?}", lang.code));
                }
            }
        }
        assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
    }

    #[test]
    fn plural_forms() {
        let p = |code, n| plural_in(code, n, "1", "few|many");
        assert_eq!([p("fr", 0), p("fr", 1), p("fr", 2)], ["1", "1", "many"]);
        assert_eq!([p("en", 0), p("en", 1), p("en", 2)], ["many", "1", "many"]);
        assert_eq!(
            [p("pl", 1), p("pl", 3), p("pl", 5), p("pl", 12), p("pl", 21), p("pl", 22)],
            ["1", "few", "many", "many", "many", "few"]
        );
        assert_eq!([p("uk", 1), p("uk", 4), p("uk", 11), p("uk", 21), p("uk", 25)], ["1", "few", "many", "1", "many"]);
        // no `|`: the one plural form serves both
        assert_eq!(plural_in("pl", 3, "plik", "pliki"), "pliki");
    }

    #[test]
    fn filling() {
        assert_eq!(fill("{} sur {}", &[&1, &"deux"]), "1 sur deux");
        assert_eq!(fill("{1}-{0}", &[&"a", &"b"]), "b-a");
        assert_eq!(fill("{x} {}", &[&3]), "{x} 3");
        assert_eq!(fill("ouvert {", &[]), "ouvert {");
        let mut v = Vec::new();
        collect("t!(\"a \\\"b\\\"\"); print!(\"no\"); t!(\"long \\\n      line\", x)", &mut v);
        assert_eq!(v, ["a \"b\"", "long line"]);
    }
}
