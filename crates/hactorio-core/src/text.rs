//! Converts between plaintext `:prototype-id:` tokens and private-use glyphs.

use hactorio_data::Registry;

/// Replaces every known glyph with its `:prototype-id:` form.
///
/// Used when saving, so stored text never depends on the codepoint table.
pub fn to_plaintext(text: &str, registry: &Registry) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match registry.entry_by_glyph(ch) {
            Some(entry) => {
                out.push(':');
                out.push_str(&entry.id);
                out.push(':');
            }
            None => out.push(ch),
        }
    }
    out
}

/// Replaces every complete `:prototype-id:` token that names an entry with its
/// glyph. Unknown tokens are left alone, so the text stays parseable.
pub fn from_plaintext(text: &str, registry: &Registry) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(colon) = rest.find(':') {
        out.push_str(&rest[..colon]);
        let after = &rest[colon + 1..];
        let Some(end) = after.find(':') else {
            out.push_str(&rest[colon..]);
            return out;
        };
        let id = &after[..end];
        match registry.entry(id) {
            Some(entry) => out.push(entry.glyph),
            None => {
                out.push(':');
                out.push_str(id);
                out.push(':');
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}
