//! How makers spell their own part numbers (ENG-5): characters a maker treats as the same, so two spellings of one
//! MPN match. Panjit writes `BC847BPN_R1_00001` (its documents, Digi-Key), people and some distributors write
//! `BC847BPN-R1-00001`; both are one part.
//!
//! A rule applies when the maker is known and matches the rule's maker or an alias, or, without a maker, when the
//! MPN has the rule's shape (Panjit's packing code at the end: `_R1_00001`).
//!
//! ```
//! use mpn_schemes::spelling;
//!
//! assert!(spelling::same_mpn("SMF22A-R1-00001", "SMF22A_R1_00001", None));
//! assert!(spelling::same_mpn("smf22a_r1_00001", "SMF22A-R1-00001", Some("PANJIT International Inc.")));
//! assert!(!spelling::same_mpn("BAT760-7-F", "BAT760_7_F", Some("Diodes Incorporated")));
//! assert_eq!(spelling::maker_spelling("SMF22A-R1-00001", None).as_deref(), Some("SMF22A_R1_00001"));
//! ```

/// One maker's spelling rule
#[derive(Debug, Clone, Copy)]
pub struct Spelling {
    /// The maker's name
    pub maker: &'static str,
    /// Other names of the maker (case and punctuation ignored)
    pub aliases: &'static [&'static str],
    /// Characters the maker treats as the same: each `(own, other)` pair reads `other` as `own`, the maker's own
    /// spelling
    pub same: &'static [(char, char)],
    /// Whether an MPN without a maker has this maker's shape
    pub shape: fn(&str) -> bool,
}

/// Every maker's rule
pub const SPELLINGS: &[Spelling] = &[Spelling {
    maker: "Panjit",
    aliases: &["PANJIT International Inc.", "Pan Jit", "PJ Semiconductor"],
    same: &[('_', '-')],
    shape: panjit_shape,
}];

/// Panjit's packing code at the end of an MPN: a separator, a letter and a digit, a separator, five digits
/// (`_R1_00001`, `-R2-00001`, `_T0_00601`)
fn panjit_shape(mpn: &str) -> bool {
    let b = mpn.trim().as_bytes();
    if b.len() < 10 {
        return false;
    }
    let t = &b[b.len() - 9..];
    let sep = |c: u8| c == b'_' || c == b'-';
    sep(t[0])
        && t[1].is_ascii_alphabetic()
        && t[2].is_ascii_digit()
        && sep(t[3])
        && t[4..].iter().all(u8::is_ascii_digit)
}

fn norm_name(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

impl Spelling {
    /// Whether the maker's name or an alias matches `maker` (case and punctuation ignored, either contains the
    /// other)
    pub fn maker_matches(&self, maker: &str) -> bool {
        let h = norm_name(maker);
        !h.is_empty()
            && std::iter::once(&self.maker).chain(self.aliases).any(|m| {
                let m = norm_name(m);
                !m.is_empty() && (m.contains(&h) || h.contains(&m))
            })
    }

    /// `mpn` in the maker's own spelling
    pub fn own(&self, mpn: &str) -> String {
        mpn.chars()
            .map(|c| {
                self.same
                    .iter()
                    .find(|(_, other)| *other == c)
                    .map_or(c, |(own, _)| *own)
            })
            .collect()
    }
}

/// The rule for `mpn` by `maker`: the maker's when it has one, else (no maker given) the first whose shape `mpn`
/// has
pub fn rule(mpn: &str, maker: Option<&str>) -> Option<&'static Spelling> {
    match maker.filter(|m| !m.trim().is_empty()) {
        Some(m) => SPELLINGS.iter().find(|s| s.maker_matches(m)),
        None => SPELLINGS.iter().find(|s| (s.shape)(mpn)),
    }
}

/// The text two spellings of one MPN share: trimmed, upper case, in the maker's own spelling when a rule applies
pub fn match_key(mpn: &str, maker: Option<&str>) -> String {
    let mpn = mpn.trim();
    let own = match rule(mpn, maker) {
        Some(r) => r.own(mpn),
        None => mpn.to_string(),
    };
    own.to_uppercase()
}

/// Whether `a` and `b` are one MPN of `maker` (case ignored). Without a maker, a rule applies when either has its
/// shape.
pub fn same_mpn(a: &str, b: &str, maker: Option<&str>) -> bool {
    if a.trim().eq_ignore_ascii_case(b.trim()) {
        return true;
    }
    let r = match maker.filter(|m| !m.trim().is_empty()) {
        Some(_) => rule(a, maker),
        None => rule(a, None).or_else(|| rule(b, None)),
    };
    r.is_some_and(|r| r.own(a.trim()).eq_ignore_ascii_case(&r.own(b.trim())))
}

/// `mpn` as its maker writes it, when that differs from how it is written here (`SMF22A-R1-00001` →
/// `SMF22A_R1_00001`); what to ask a supplier for when the MPN as written is not found
pub fn maker_spelling(mpn: &str, maker: Option<&str>) -> Option<String> {
    let mpn = mpn.trim();
    rule(mpn, maker)
        .map(|r| r.own(mpn))
        .filter(|own| own != mpn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panjit_separators_are_the_same() {
        for (a, b) in [
            ("BC847BPN_R1_00001", "BC847BPN-R1-00001"),
            ("SMF22A_R1_00001", "smf22a-r1-00001"),
            ("SBA130CS_R1_00001", "SBA130CS-R1_00001"),
        ] {
            assert!(same_mpn(a, b, None), "{a} {b}");
            assert!(same_mpn(a, b, Some("Panjit International Inc.")), "{a} {b}");
            assert_eq!(match_key(a, None), match_key(b, None));
        }
    }

    #[test]
    fn other_makers_keep_their_separators() {
        assert!(!same_mpn("BAT760-7-F", "BAT760_7_F", None));
        assert!(!same_mpn(
            "BAT760-7-F",
            "BAT760_7_F",
            Some("Diodes Incorporated")
        ));
        // Panjit's shape, but another maker named: no rule
        assert!(!same_mpn("X_R1_00001", "X-R1-00001", Some("Nexperia")));
        assert_eq!(match_key(" bat760-7-f ", None), "BAT760-7-F");
    }

    #[test]
    fn maker_names_its_rule() {
        // no packing code, but the maker is Panjit
        assert!(same_mpn("PJA3407_A", "PJA3407-A", Some("PANJIT")));
        assert_eq!(
            maker_spelling("PJA3407-A", Some("Panjit")).as_deref(),
            Some("PJA3407_A")
        );
        assert_eq!(maker_spelling("SMF22A_R1_00001", None), None);
        assert_eq!(maker_spelling("BAT760-7-F", None), None);
    }

    #[test]
    fn shape_is_the_packing_code_at_the_end() {
        assert!(panjit_shape("BZT52C5V1_R1_00001"));
        assert!(panjit_shape("1N4148W-R2-00001"));
        assert!(panjit_shape("MMBT3904_T0_00601"));
        assert!(!panjit_shape("R1_00001"));
        assert!(!panjit_shape("BAT760-7-F"));
        assert!(!panjit_shape("ABC_R1_0001"));
    }
}
