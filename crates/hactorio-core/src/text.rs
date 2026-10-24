//! Converts between plaintext `:prototype-id:` tokens and private-use glyphs.

use hactorio_data::{Registry, signal_by_glyph, signal_by_id};

/// Replaces every known glyph with its `:prototype-id:` form.
///
/// Used when saving, so stored text never depends on the codepoint table.
pub fn to_plaintext(text: &str, registry: &Registry) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if let Some(signal) = signal_by_glyph(ch) {
            out.push(':');
            out.push_str(signal.id);
            out.push(':');
            continue;
        }
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

/// Replaces every complete `:prototype-id:` or `:signal:` token that names an
/// entry or signal with its glyph. Unknown tokens are left alone, so the text
/// stays parseable.
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
        if let Some(signal) = signal_by_id(id) {
            out.push(signal.glyph);
            rest = &after[end + 1..];
            continue;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use hactorio_data::{Locale, postprocess};

    fn registry() -> Registry {
        postprocess("{}", &Locale::empty()).unwrap()
    }

    #[test]
    fn signal_tokens_round_trip_through_plaintext() {
        let registry = registry();
        let plain = ".:clock: .:speed: .:lightning: .:stack-size: :stacked-belt: :iron-plate:";
        let glyphs = from_plaintext(plain, &registry);
        assert!(glyphs.contains(hactorio_data::signal_by_id("clock").unwrap().glyph));
        assert!(glyphs.contains(hactorio_data::signal_by_id("speed").unwrap().glyph));
        assert!(glyphs.contains(hactorio_data::signal_by_id("lightning").unwrap().glyph));
        assert!(glyphs.contains(hactorio_data::signal_by_id("stack-size").unwrap().glyph));
        assert!(glyphs.contains(hactorio_data::signal_by_id("stacked-belt").unwrap().glyph));
        assert_eq!(to_plaintext(&glyphs, &registry), plain);
    }
}
