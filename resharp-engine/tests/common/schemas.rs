use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EngineFile {
    #[serde(default)]
    pub description: Option<String>,
    pub test: Vec<EngineCase>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EngineCase {
    #[serde(default)]
    pub name: String,
    pub pattern: String,
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub matches: Vec<[usize; 2]>,
    #[serde(default)]
    pub ignore: bool,
    #[serde(default)]
    pub expect_error: bool,
    #[serde(default)]
    pub anchored: bool,
    #[serde(default)]
    pub ascii: bool,
    #[serde(default)]
    pub javascript: bool,
    #[serde(default)]
    pub full: bool,
    #[serde(default)]
    pub prefix_kind: Option<String>,
    #[serde(default)]
    pub not_prefix_kind: Option<String>,
    #[serde(default)]
    pub vs_regex: bool,
    #[serde(default)]
    pub vs_find_all: bool,
    #[serde(default)]
    pub supported: Option<bool>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InternalFile {
    #[serde(default)]
    pub description: Option<String>,
    pub test: Vec<InternalCase>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InternalCase {
    #[serde(default)]
    pub name: String,
    pub pattern: String,
    pub pp: Option<String>,
    pub ts_rev: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrefixFile {
    #[serde(default)]
    pub description: Option<String>,
    pub test: Vec<PrefixCase>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrefixCase {
    pub name: String,
    pub pattern: String,
    #[serde(default)]
    pub ignore: bool,
    pub kind: Option<String>,
    pub prefix_rev: Option<String>,
    pub potential_rev: Option<String>,
    pub potential_fwd: Option<String>,
    pub conv_literal: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RevNullsFile {
    #[serde(default)]
    pub description: Option<String>,
    pub test: Vec<RevNullsCase>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RevNullsCase {
    pub name: String,
    pub pattern: String,
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub ignore: bool,
    pub rev_nulls: Vec<usize>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutoHardenFile {
    #[serde(default)]
    pub description: Option<String>,
    pub test: Vec<AutoHardenCase>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutoHardenCase {
    pub pattern: String,
    pub hardened: bool,
    pub fwd: Option<bool>,
    #[serde(default)]
    pub ignore: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuadraticFile {
    #[serde(default)]
    pub description: Option<String>,
    pub test: Vec<QuadraticCase>,
}

#[derive(Deserialize, JsonSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum QuadKind {
    Fwd,
    Dfa,
}

impl Default for QuadKind {
    fn default() -> Self {
        QuadKind::Fwd
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuadraticCase {
    #[serde(default)]
    pub name: String,
    pub pattern: String,
    pub unit: String,
    #[serde(default)]
    pub kind: QuadKind,
    #[serde(default)]
    pub note: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DerivFile {
    #[serde(default)]
    pub description: Option<String>,
    pub test: Vec<DerivCase>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DerivCase {
    pub name: String,
    pub pattern: String,
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub ignore: bool,
    #[serde(default)]
    pub rev: Vec<String>,
    #[serde(default)]
    pub fwd: Vec<String>,
    pub rev_nulls: Option<Vec<usize>>,
    pub fwd_nulls: Option<Vec<usize>>,
    #[serde(default)]
    pub rev_effects: Vec<String>,
    #[serde(default)]
    pub fwd_effects: Vec<String>,
    #[serde(default)]
    pub ascii: bool,
}
