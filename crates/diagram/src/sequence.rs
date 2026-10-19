//! Plume: a sequence diagram edited as a table, people (participants) and the messages they send
//! in order, written as a Mermaid `sequenceDiagram` and read back from one. Lines a table can't
//! show (notes, loops, activations…) keep the diagram in the source editor.

use serde::{Deserialize, Serialize};

/// Most participants and messages.
pub const MAX_PARTICIPANTS: usize = 50;
pub const MAX_MESSAGES: usize = 500;
const MAX_TEXT: usize = 200;

/// A message from participant `from` to `to` (indices in [`Sequence::participants`]).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Message {
    pub from: usize,
    pub to: usize,
    pub text: String,
    /// A reply: drawn dotted.
    pub reply: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Sequence {
    pub participants: Vec<String>,
    pub messages: Vec<Message>,
}

fn clip(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_TEXT).collect()
}

/// A participant's name as Mermaid reads it: letters, digits and `_`, other runs as `_`.
fn ident(name: &str, k: usize) -> String {
    let mut id: String = name.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    id = id.trim_matches('_').to_string();
    if id.is_empty() { format!("p{}", k + 1) } else { id }
}

impl Sequence {
    /// Participant `name`'s index, added when new.
    fn index(&mut self, name: &str) -> Option<usize> {
        let name = clip(name);
        if name.is_empty() {
            return None;
        }
        if let Some(i) = self.participants.iter().position(|p| *p == name) {
            return Some(i);
        }
        if self.participants.len() >= MAX_PARTICIPANTS {
            return None;
        }
        self.participants.push(name);
        Some(self.participants.len() - 1)
    }

    /// The diagram as Mermaid: each participant declared (`participant id as Name` when its
    /// name isn't an identifier), then the messages.
    pub fn to_mermaid(&self) -> String {
        let mut ids: Vec<String> = Vec::new();
        let mut out = vec!["sequenceDiagram".to_string()];
        for (k, p) in self.participants.iter().take(MAX_PARTICIPANTS).enumerate() {
            let mut id = ident(p, k);
            while ids.contains(&id) {
                id.push('_');
            }
            if id == *p {
                out.push(format!("    participant {id}"));
            } else {
                out.push(format!("    participant {id} as {}", clip(p)));
            }
            ids.push(id);
        }
        for m in self.messages.iter().take(MAX_MESSAGES) {
            let (Some(a), Some(b)) = (ids.get(m.from), ids.get(m.to)) else { continue };
            let text = clip(&m.text).replace([';', '#'], ",");
            out.push(format!("    {a}{}{b}: {text}", if m.reply { "-->>" } else { "->>" }));
        }
        out.join("\n")
    }

    /// A sequence diagram read back; `None` when it has lines a table can't show.
    pub fn parse(src: &str) -> Option<Sequence> {
        let mut lines = src.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("%%"));
        if lines.next()? != "sequenceDiagram" {
            return None;
        }
        let mut s = Sequence::default();
        let mut alias: Vec<(String, usize)> = Vec::new();
        for line in lines.take(MAX_MESSAGES + MAX_PARTICIPANTS + 10) {
            if let Some(rest) = line.strip_prefix("participant ").or_else(|| line.strip_prefix("actor ")) {
                let (id, name) = match rest.split_once(" as ") {
                    Some((id, name)) => (id.trim(), name.trim()),
                    None => (rest.trim(), rest.trim()),
                };
                let i = s.index(name)?;
                alias.push((id.to_string(), i));
                continue;
            }
            let (head, text) = line.split_once(':')?;
            let (op, reply) = [("-->>", true), ("->>", false)].into_iter().find(|(op, _)| head.contains(op))?;
            let (a, b) = head.split_once(op)?;
            let mut who = |n: &str| -> Option<usize> {
                let n = n.trim();
                match alias.iter().find(|(id, _)| id == n) {
                    Some((_, i)) => Some(*i),
                    None => s.index(n),
                }
            };
            let (from, to) = (who(a)?, who(b)?);
            if s.messages.len() >= MAX_MESSAGES {
                return None;
            }
            s.messages.push(Message { from, to, text: clip(text), reply });
        }
        (!s.participants.is_empty()).then_some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sequence_round_trips_and_draws() {
        let s = Sequence {
            participants: vec!["Author".into(), "Lead reviewer".into()],
            messages: vec![
                Message { from: 0, to: 1, text: "Request review".into(), reply: false },
                Message { from: 1, to: 0, text: "Comments; two".into(), reply: true },
            ],
        };
        let src = s.to_mermaid();
        assert_eq!(
            src,
            "sequenceDiagram\n    participant Author\n    participant Lead_reviewer as Lead reviewer\n    Author->>Lead_reviewer: Request review\n    Lead_reviewer-->>Author: Comments, two"
        );
        let back = Sequence::parse(&src).unwrap();
        assert_eq!(back.participants, s.participants);
        assert_eq!(back.messages[1].text, "Comments, two");
        crate::draw(&src).unwrap();
        assert!(Sequence::parse("sequenceDiagram\n A->>B: hi\n Note over A: x").is_none());
        assert_eq!(Sequence::parse("sequenceDiagram\n Alice->>Bob: hi").unwrap().participants, ["Alice", "Bob"]);
        for junk in ["", "sequenceDiagram\n ->>: ", "sequenceDiagram\n participant", "flowchart LR\n a-->b"] {
            assert!(Sequence::parse(junk).is_none(), "{junk}");
        }
    }
}
