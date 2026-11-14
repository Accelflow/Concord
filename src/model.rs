use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    pub runner: RunnerConfig,
    #[serde(default)]
    pub rust: RustAnalysisConfig,
    #[serde(default)]
    pub non_runtime_paths: Vec<String>,
    #[serde(default)]
    pub always_test_files: Vec<String>,
    #[serde(default)]
    pub commands: Vec<VerificationCommand>,
}
impl Config {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        use anyhow::Context;
        let config: Self = toml::from_str(
            &std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?,
        )
        .with_context(|| format!("parse {}", path.display()))?;
        anyhow::ensure!(
            config.schema_version == 1,
            "unsupported concord.toml schema_version"
        );
        Ok(config)
    }
}
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct RustAnalysisConfig {
    #[serde(default)]
    pub runtime_inputs: BTreeMap<String, Vec<String>>,
}
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunnerKind {
    #[default]
    Vitest,
    Cargo,
    Commands,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RunnerConfig {
    #[serde(default)]
    pub kind: RunnerKind,
    #[serde(default)]
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub version: String,
    #[serde(default = "timeout_default")]
    pub timeout_seconds: u64,
}
fn timeout_default() -> u64 {
    120
}
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct DefinitionDocument {
    pub schema_version: u32,
    #[serde(default)]
    pub requirements: Vec<Requirement>,
    #[serde(default)]
    pub contracts: Vec<Contract>,
    #[serde(default)]
    pub properties: Vec<Property>,
    #[serde(default)]
    pub relations: Vec<ContractPropertyRelation>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    pub description: String,
}
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    #[default]
    Proposed,
    Approved,
    Deprecated,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub id: String,
    pub description: String,
    pub version: String,
    pub scope: String,
    pub assumptions: Vec<String>,
    pub preconditions: Vec<String>,
    pub responsibility: String,
    #[serde(default)]
    pub approval_status: ApprovalStatus,
    pub requirement_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub property_ids: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Property {
    pub id: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub description: String,
    #[serde(default)]
    pub approval_status: ApprovalStatus,
    pub implementations: Vec<String>,
    pub obligations: Vec<VerificationObligation>,
}
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMethod {
    Test,
    Unit,
    Api,
    Integration,
    E2e,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct VerificationObligation {
    pub id: String,
    pub method: VerificationMethod,
    #[serde(default)]
    pub tests: Vec<TestRef>,
    #[serde(default)]
    pub commands: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct VerificationCommand {
    pub id: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Repository inputs, in addition to Contract/Property relationships.
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_patterns: Vec<String>,
    #[serde(default = "timeout_default")]
    pub timeout_seconds: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<CommandReportConfig>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CommandReportConfig {
    pub required_cases: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CommandCase {
    pub id: String,
    pub status: CommandCaseStatus,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandCaseStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CommandReportResult {
    pub sha256: String,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub cases: Vec<CommandCase>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CommandDecision {
    pub id: String,
    pub selected: bool,
    pub reasons: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CommandResult {
    pub id: String,
    pub exit_code: Option<i32>,
    pub status: String,
    pub stdout: String,
    pub stderr: String,
    pub elapsed_ms: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<CommandReportResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    Requires,
    ContributesTo,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Applicability {
    pub condition: String,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ContractPropertyRelation {
    pub id: String,
    pub contract_id: String,
    pub property_id: String,
    pub relation: RelationKind,
    #[serde(default)]
    pub applicability: Option<Applicability>,
    #[serde(default)]
    pub implementations: Vec<String>,
    #[serde(default)]
    pub obligations: Vec<VerificationObligation>,
}
impl DefinitionDocument {
    pub fn resolved_relations(&self) -> Vec<ContractPropertyRelation> {
        if !self.relations.is_empty() {
            return self.relations.clone();
        }
        self.contracts
            .iter()
            .flat_map(|c| {
                c.property_ids.iter().filter_map(|id| {
                    self.properties
                        .iter()
                        .find(|p| p.id == *id)
                        .map(|p| ContractPropertyRelation {
                            id: format!(
                                "compat:{}",
                                crate::git::hash(
                                    &serde_json::to_vec(&(&c.id, &p.id))
                                        .expect("serialize ID pair")
                                )
                            ),
                            contract_id: c.id.clone(),
                            property_id: p.id.clone(),
                            relation: RelationKind::Requires,
                            applicability: None,
                            implementations: p.implementations.clone(),
                            obligations: p.obligations.clone(),
                        })
                })
            })
            .collect()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RequiredObligation {
    pub relation_id: String,
    pub property_id: String,
    pub obligation: VerificationObligation,
}
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Unverified,
    Verified,
    Failed,
    Stale,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ObligationResult {
    pub relation_id: String,
    pub property_id: String,
    pub obligation_id: String,
    pub method: VerificationMethod,
    pub status: VerificationStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command_ids: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PropertyResult {
    pub relation_id: String,
    pub contract_id: String,
    pub relation: RelationKind,
    pub applicability: Option<Applicability>,
    pub contract_version: String,
    pub property_version: String,
    pub scope: String,
    pub commit_sha: String,
    pub run_id: String,
    pub environment_fingerprint: String,
    pub input_fingerprint: String,
    pub applicability_evaluated: bool,
    pub result: String,
    pub property_id: String,
    pub approval_status: ApprovalStatus,
    pub status: VerificationStatus,
    pub obligation_ids: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TestRef {
    pub file: String,
    pub name: String,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Test {
    pub id: String,
    pub file: String,
    pub name: String,
}
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Graph {
    pub dependencies: BTreeMap<String, BTreeSet<String>>,
    pub uncertainties: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Snapshot {
    pub schema_version: u32,
    pub fingerprint: String,
    pub head: String,
    pub files: BTreeMap<String, String>,
    pub tests: Vec<Test>,
    pub graph: Graph,
    pub definitions: DefinitionDocument,
    pub definition_source: String,
    pub legacy_schema: bool,
    pub runner_version: String,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Decision {
    pub test: Test,
    pub selected: bool,
    pub reasons: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Plan {
    pub schema_version: u32,
    pub plan_id: String,
    pub fingerprint: String,
    pub base_sha: String,
    pub merge_base_sha: String,
    pub head_sha: String,
    pub mode: String,
    pub changed_files: BTreeSet<String>,
    pub selected_files: BTreeSet<String>,
    pub excluded_files: BTreeSet<String>,
    pub fallback_reasons: BTreeSet<String>,
    pub affected_properties: BTreeSet<String>,
    pub affected_relations: BTreeSet<String>,
    pub relations: Vec<ContractPropertyRelation>,
    pub affected_contracts: BTreeSet<String>,
    pub property_gaps: Vec<String>,
    pub contracts: Vec<Contract>,
    pub properties: Vec<Property>,
    pub required_obligations: Vec<RequiredObligation>,
    pub tests: Vec<Test>,
    pub decisions: Vec<Decision>,
    #[serde(default)]
    pub selected_commands: Vec<VerificationCommand>,
    #[serde(default)]
    pub command_decisions: Vec<CommandDecision>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CaseResult {
    pub file: String,
    pub name: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_id: Option<String>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Run {
    pub schema_version: u32,
    pub run_id: String,
    pub plan: Plan,
    pub status: String,
    pub exit_code: i32,
    pub runner_exit_code: Option<i32>,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub cases: Vec<CaseResult>,
    #[serde(default)]
    pub command_results: Vec<CommandResult>,
    pub property_results: Vec<PropertyResult>,
    pub obligation_results: Vec<ObligationResult>,
    pub input_unchanged: bool,
    pub result_integrity: bool,
    pub environment_fingerprint: String,
    pub host: String,
    pub os: String,
    pub started_ms: u128,
    pub elapsed_ms: u128,
    pub plan_ms: u128,
    pub execution_ms: u128,
    pub result_hash: String,
}
