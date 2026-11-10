//! The scheme file as written (LANG-1): TOML read with serde, before validation. Unknown fields are refused, so a
//! typo is an error and never a silently ignored line.

use serde::Deserialize;
use std::collections::BTreeMap;
use toml::Spanned;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawFile {
    pub scheme: Spanned<RawHeader>,
    #[serde(default)]
    pub implies: BTreeMap<String, Spanned<String>>,
    /// Reserved for the series facts better_erc reads (LANG-6); refused until the format is defined
    pub series: Option<Spanned<toml::Value>>,
    #[serde(default)]
    pub segment: Vec<Spanned<RawSegment>>,
    #[serde(default)]
    pub available: Vec<Spanned<BTreeMap<String, RawRowValue>>>,
    #[serde(default)]
    pub excluded: Vec<Spanned<BTreeMap<String, RawRowValue>>>,
    #[serde(default)]
    pub example: Vec<Spanned<RawExample>>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawHeader {
    pub id: String,
    pub version: u32,
    pub maker: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub class: String,
    pub title: String,
    #[serde(default)]
    pub sources: Vec<RawSource>,
    pub availability: Option<String>,
    pub min_stem: Option<String>,
    #[allow(dead_code)]
    pub note: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawSource {
    pub doc: String,
    pub url: String,
    pub fetched: String,
    pub pages: Option<String>,
    #[allow(dead_code)]
    pub note: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawSegment {
    pub name: Option<String>,
    pub param: Option<String>,
    #[serde(default)]
    pub optional: bool,
    pub absent: Option<String>,
    pub when: Option<BTreeMap<String, RawRowValue>>,
    #[allow(dead_code)]
    pub note: Option<String>,

    pub literal: Option<String>,
    pub table: Option<BTreeMap<String, RawEntry>>,
    pub code: Option<String>,
    pub free: Option<RawFree>,
    pub group: Option<Vec<RawSegment>>,
    pub alternatives: Option<Vec<RawSegment>>,

    // Options of a code segment
    pub unit: Option<String>,
    pub series: Option<OneOrMany>,
    pub digits: Option<usize>,
    pub scale: Option<i32>,
    pub places: Option<usize>,
    pub letters: Option<BTreeMap<String, i32>>,
    pub lead_zero: Option<bool>,
    pub pad_whole: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub(crate) enum RawEntry {
    Value(String),
    Full {
        value: Option<String>,
        #[serde(default)]
        default: bool,
        #[allow(dead_code)]
        note: Option<String>,
    },
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawFree {
    pub chars: String,
    pub len: RawLen,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub(crate) enum RawLen {
    Exact(usize),
    Range { min: usize, max: usize },
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub(crate) enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    pub fn list(&self) -> Vec<String> {
        match self {
            OneOrMany::One(s) => vec![s.clone()],
            OneOrMany::Many(v) => v.clone(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub(crate) enum RawRowValue {
    One(String),
    Many(Vec<String>),
    /// The segment's code itself, for a segment with a param whose codes share a value
    Code {
        code: OneOrMany,
    },
    Range {
        min: Option<String>,
        max: Option<String>,
        series: Option<OneOrMany>,
    },
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawExample {
    pub mpn: String,
    pub params: BTreeMap<String, String>,
    #[allow(dead_code)]
    pub note: Option<String>,
    #[allow(dead_code)]
    pub source: Option<String>,
}
