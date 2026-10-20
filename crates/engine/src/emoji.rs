//! Plume: the emoji Insert › Emoji offers, in Unicode's keyboard order and groups, with their
//! English and French names and keywords (CLDR) and their skin-tone variants. The table is
//! `emoji.tsv`, written by `cargo xtask emoji-data`; it is read once, the first time it's needed.

use std::sync::OnceLock;

/// The groups, in order (Unicode's, without the components).
pub const GROUPS: [&str; 9] =
    ["Smileys & Emotion", "People & Body", "Animals & Nature", "Food & Drink", "Travel & Places", "Activities", "Objects", "Symbols", "Flags"];

/// The skin tones, lightest first.
pub const TONES: [&str; 5] = ["\u{1F3FB}", "\u{1F3FC}", "\u{1F3FD}", "\u{1F3FE}", "\u{1F3FF}"];

/// One emoji.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Emoji {
    /// Index in [`GROUPS`].
    pub group: usize,
    pub text: &'static str,
    pub name_en: &'static str,
    pub name_fr: &'static str,
    /// Keywords in both languages, lower case.
    pub keywords: Vec<&'static str>,
    /// The five skin-tone variants, lightest first; empty when it has none.
    pub tones: Vec<&'static str>,
}

impl Emoji {
    /// The name in `lang` (`fr…` French, anything else English).
    pub fn name(&self, lang: &str) -> &'static str {
        if lang.starts_with("fr") && !self.name_fr.is_empty() { self.name_fr } else { self.name_en }
    }

    /// The emoji in skin tone `tone` (1–5, lightest first); itself for 0, or when it has none.
    pub fn in_tone(&self, tone: usize) -> &'static str {
        tone.checked_sub(1).and_then(|t| self.tones.get(t)).copied().unwrap_or(self.text)
    }

    /// How well it matches `words` (all of them must match): 0 = not at all, higher is better.
    fn score(&self, words: &[String]) -> u32 {
        let mut total = 0;
        for w in words {
            let hit = |s: &str| {
                let s = fold(s);
                if s == *w {
                    4
                } else if s.split(|c: char| !c.is_alphanumeric()).any(|x| x == w) {
                    3
                } else if s.starts_with(w.as_str()) {
                    2
                } else if s.contains(w.as_str()) {
                    1
                } else {
                    0
                }
            };
            // A word of the name counts more than a keyword.
            let name = hit(self.name_en).max(hit(self.name_fr));
            let keyword = self.keywords.iter().map(|k| hit(k)).max().unwrap_or(0);
            let best = if name > 0 { name + 4 } else { keyword };
            if best == 0 {
                return 0;
            }
            total += best;
        }
        total
    }
}

/// `s` lower case without accents, for matching (`Cœur` and `coeur`, `étoile` and `etoile`).
fn fold(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' => o.push('a'),
            'é' | 'è' | 'ê' | 'ë' => o.push('e'),
            'î' | 'ï' | 'í' => o.push('i'),
            'ô' | 'ö' | 'ó' | 'õ' => o.push('o'),
            'ù' | 'û' | 'ü' | 'ú' => o.push('u'),
            'ç' => o.push('c'),
            'ÿ' => o.push('y'),
            'ñ' => o.push('n'),
            'œ' => o.push_str("oe"),
            'æ' => o.push_str("ae"),
            '’' => o.push('\''),
            c => o.push(c),
        }
    }
    o
}

/// Every emoji, in order.
pub fn all() -> &'static [Emoji] {
    static ALL: OnceLock<Vec<Emoji>> = OnceLock::new();
    ALL.get_or_init(|| {
        include_str!("emoji.tsv")
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let mut f = l.split('\t');
                let group = f.next()?.parse::<usize>().ok().filter(|g| *g < GROUPS.len())?;
                let text = f.next().filter(|t| !t.is_empty())?;
                let (name_en, name_fr) = (f.next().unwrap_or(""), f.next().unwrap_or(""));
                let keywords = f.next().unwrap_or("").split('|').filter(|k| !k.is_empty()).collect();
                let tones: Vec<&str> = f.next().unwrap_or("").split(' ').filter(|t| !t.is_empty()).collect();
                Some(Emoji { group, text, name_en, name_fr, keywords, tones: if tones.len() == 5 { tones } else { Vec::new() } })
            })
            .collect()
    })
}

/// The emoji of group `g`.
pub fn group(g: usize) -> impl Iterator<Item = &'static Emoji> {
    all().iter().filter(move |e| e.group == g)
}

/// The emoji matching `query` (names and keywords, English or French, accents ignored), best
/// first, at most `limit`. An emoji typed as the query finds itself.
pub fn search(query: &str, limit: usize) -> Vec<&'static Emoji> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    if let Some(e) = all().iter().find(|e| e.text == q || e.tones.contains(&q)) {
        return vec![e];
    }
    let words: Vec<String> = fold(q).split_whitespace().take(8).map(str::to_string).collect();
    let mut hits: Vec<(u32, usize, &Emoji)> = all().iter().enumerate().map(|(i, e)| (e.score(&words), i, e)).filter(|(s, _, _)| *s > 0).collect();
    // Best match first; among equals, the shorter name (the closer one: `pouce` finds 👍 « pouce
    // vers le haut » before 🤏 « pouce et index rapprochés »), then Unicode's order.
    let short = |e: &Emoji| e.name_en.chars().count().min(if e.name_fr.is_empty() { usize::MAX } else { e.name_fr.chars().count() });
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(short(a.2).cmp(&short(b.2))).then(a.1.cmp(&b.1)));
    hits.into_iter().take(limit).map(|(_, _, e)| e).collect()
}

/// The emoji `text` is (any of its skin tones too), if it is one.
pub fn find(text: &str) -> Option<&'static Emoji> {
    all().iter().find(|e| e.text == text || e.tones.contains(&text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_complete_and_in_order() {
        let all = all();
        assert!(all.len() > 1800, "{}", all.len());
        assert_eq!(all.first().map(|e| e.text), Some("😀"));
        for g in 0..GROUPS.len() {
            assert!(group(g).count() > 50, "group {g}");
        }
        let thumbs = find("👍").unwrap();
        assert_eq!((thumbs.name_en, thumbs.name_fr), ("thumbs up", "pouce vers le haut"));
        assert_eq!(thumbs.in_tone(3), "👍🏽");
        assert_eq!(thumbs.in_tone(0), "👍");
        assert_eq!(find("👍🏿").map(|e| e.text), Some("👍"));
        assert!(find("🇫🇷").is_some_and(|e| e.group == 8));
        assert!(all.iter().all(|e| !e.name_en.is_empty()));
    }

    #[test]
    fn search_finds_by_english_or_french_name_and_keyword() {
        let first = |q: &str| search(q, 10).first().map(|e| e.text);
        assert_eq!(first("thumbs up"), Some("👍"));
        assert_eq!(first("pouce"), Some("👍"));
        assert_eq!(first("coeur rouge"), Some("❤️"));
        assert_eq!(first("drapeau france"), Some("🇫🇷"));
        assert!(search("etoile", 20).iter().any(|e| e.text == "⭐"), "accents ignored");
        assert_eq!(first("🚀"), Some("🚀"));
        assert!(search("zzzqqq", 10).is_empty() && search("  ", 10).is_empty());
        assert_eq!(search("a", 5).len(), 5);
    }
}
