//! A report to hand to someone who helps: a zip of plain text files the person can open and
//! read before passing it on. Open Recovr only writes it; it never sends anything.
//! Names that say who and where (the account, the computer, the home folder, e-mail
//! addresses, document titles in recovered file names) are masked before anything is written.

use std::io::Write;
use std::path::Path;

/// One file of the report: its name in the zip and its text.
pub struct Part {
    pub name: &'static str,
    pub text: String,
}

/// The report's file name, with the date and time (UTC) so that two reports don't collide.
pub fn file_name() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (y, m, d) = civil(secs / 86_400);
    let (h, min) = (secs % 86_400 / 3600, secs % 3600 / 60);
    format!("open-recovr-report-{y:04}-{m:02}-{d:02}-{h:02}{min:02}.zip")
}

/// Days since 1970 to (year, month, day), after H. Hinnant.
fn civil(days: u64) -> (i64, u32, u32) {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// What gets masked on this computer: the home folder, the account and the computer's name.
pub struct Mask {
    home: Option<String>,
    words: Vec<(String, &'static str)>,
}

impl Mask {
    pub fn here() -> Mask {
        let var = |k: &str| std::env::var(k).ok().filter(|v| v.trim().len() >= 2);
        let home = crate::sys::home_dir().map(|h| h.to_string_lossy().trim_end_matches(['\\', '/']).to_string());
        let mut words = Vec::new();
        for k in ["USERNAME", "USER", "LOGNAME"] {
            if let Some(v) = var(k) {
                words.push((v, "<user>"));
            }
        }
        for k in ["COMPUTERNAME", "HOSTNAME"] {
            if let Some(v) = var(k) {
                words.push((v, "<computer>"));
            }
        }
        Mask::new(home.filter(|h| h.len() > 3), words)
    }

    fn new(home: Option<String>, mut words: Vec<(String, &'static str)>) -> Mask {
        words.sort_by_key(|(w, _)| std::cmp::Reverse(w.len()));
        words.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
        Mask { home, words }
    }

    /// The text with the home folder, the names and e-mail addresses replaced.
    pub fn apply(&self, text: &str) -> String {
        // addresses first: a name inside one would otherwise break it up
        let mut out = mask_titles(&mask_emails(text));
        if let Some(home) = &self.home {
            for h in [home.clone(), home.replace('\\', "/")] {
                out = replace_ci(&out, &h, "~", false);
            }
        }
        for (w, by) in &self.words {
            out = replace_ci(&out, w, by, true);
        }
        out
    }
}

/// Replaces `what` in `text`, ignoring case; with `whole`, only where it is not part of a longer word.
fn replace_ci(text: &str, what: &str, by: &str, whole: bool) -> String {
    if what.is_empty() {
        return text.to_string();
    }
    let lower = text.to_lowercase();
    let needle = what.to_lowercase();
    // lower-casing can change lengths outside ASCII: then compare as is
    let (hay, needle) = if lower.len() == text.len() { (lower, needle) } else { (text.to_string(), what.to_string()) };
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(at) = hay[i..].find(&needle) {
        let start = i + at;
        let end = start + needle.len();
        if whole && (word(text[..start].chars().next_back()) || word(text[end..].chars().next())) {
            out.push_str(&text[i..end]);
        } else {
            out.push_str(&text[i..start]);
            out.push_str(by);
        }
        i = end;
    }
    out.push_str(&text[i..]);
    out
}

/// PhotoRec puts a document's title in the name of the file it saves (`f0007192_Lease.pdf`):
/// only the number and the extension are kept (`f0007192.pdf`).
fn mask_titles(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (n, line) in text.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        let mut rest = line;
        while let Some((head, tail)) = title_at(rest) {
            out.push_str(head);
            rest = tail;
        }
        out.push_str(rest);
    }
    out
}

/// In `line`, the text up to the first `fNNNNN` with a title and the line after that title.
fn title_at(line: &str) -> Option<(&str, &str)> {
    let b = line.as_bytes();
    for i in 0..b.len() {
        if b[i] != b'f' || (i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) {
            continue;
        }
        let digits = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
        let under = i + 1 + digits;
        if digits < 5 || b.get(under) != Some(&b'_') {
            continue;
        }
        // a title may hold spaces: it ends at the last extension before a tab, a bracket or a
        // comma (or the end of the line); without one, all of that goes
        let end = line[under..].find(['\t', '(', ')', ',']).map_or(line.len(), |e| under + e);
        let part = &line[under..end];
        let is_ext = |d: usize| {
            let ext = part[d + 1..].split(' ').next().unwrap_or("");
            (1..=5).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric())
        };
        let ext = part.rmatch_indices('.').map(|(d, _)| d).find(|&d| is_ext(d)).map_or(end, |d| under + d);
        return Some((&line[..under], &line[ext..]));
    }
    None
}

/// Any word that looks like an e-mail address becomes `<e-mail>`.
fn mask_emails(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        let looks = word.split_once('@').is_some_and(|(a, b)| !a.is_empty() && b.contains('.') && !b.starts_with('.'));
        out.push_str(if looks { "<e-mail>" } else { word });
        word.clear();
    };
    for c in text.chars() {
        if c.is_alphanumeric() || "@._%+-".contains(c) {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// Writes the parts, masked, into a zip at `path`.
pub fn write(path: &Path, parts: &[Part], mask: &Mask) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for p in parts {
        zip.start_file(p.name, options).map_err(std::io::Error::other)?;
        zip.write_all(mask.apply(&p.text).as_bytes())?;
    }
    zip.finish().map_err(std::io::Error::other)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask() -> Mask {
        Mask::new(Some(r"C:\Users\Julie".into()), vec![("Julie".into(), "<user>"), ("DESKTOP-7QX".into(), "<computer>")])
    }

    #[test]
    fn masks_who_and_where() {
        let m = mask();
        assert_eq!(m.apply(r"C:\Users\Julie\Pictures\a.img"), r"~\Pictures\a.img");
        assert_eq!(m.apply(r"c:\users\julie\AppData"), r"~\AppData");
        assert_eq!(m.apply("C:/Users/Julie/x"), "~/x");
        assert_eq!(m.apply(r"D:\Backups\Julie\x"), r"D:\Backups\<user>\x");
        assert_eq!(m.apply("on desktop-7qx today"), "on <computer> today");
        // part of a longer word: left alone
        assert_eq!(m.apply("Julienne"), "Julienne");
        assert_eq!(m.apply("write to julie.m@example.org now"), "write to <e-mail> now");
        assert_eq!(m.apply("a@b, x@y.z."), "a@b, <e-mail>");
        // titles in recovered file names
        assert_eq!(m.apply("gardé f0007192_Contrat_de_location.pdf (295 Ko)"), "gardé f0007192.pdf (295 Ko)");
        assert_eq!(m.apply("recup.1/f0007192_Contrat.pdf	 7192-7783"), "recup.1/f0007192.pdf	 7192-7783");
        assert_eq!(m.apply("f0000136.jpg, f12345_Lettre de M. Durand.odt (2 Ko)"), "f0000136.jpg, f12345.odt (2 Ko)");
        assert_eq!(m.apply("f12345_Lettre perso	x"), "f12345	x");
        assert_eq!(m.apply("ref123456_x.pdf f12_x.pdf"), "ref123456_x.pdf f12_x.pdf");
    }

    #[test]
    fn dates_of_file_names() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_402), (2025, 11, 10));
        assert_eq!(civil(11_016), (2000, 2, 29));
    }

    #[test]
    fn writes_a_zip_of_masked_text() {
        let dir = std::env::temp_dir().join(format!("open-recovr-report-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("r.zip");
        let parts = [Part { name: "journal.txt", text: r"Disk C:\Users\Julie\demo.img".into() }];
        write(&path, &parts, &mask()).unwrap();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        let mut text = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("journal.txt").unwrap(), &mut text).unwrap();
        assert_eq!(text, r"Disk ~\demo.img");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
