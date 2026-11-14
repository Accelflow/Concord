use crate::{git, model::*};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{collections::BTreeSet, fs, path::Path};

pub fn load(root: &Path) -> Result<(DefinitionDocument, String, bool)> {
    let split = root.join("concord");
    if crate::definition_source::has_split(&split) {
        let document = crate::definition_source::read(&split)?;
        validate(&document)?;
        return Ok((document, "concord/".into(), false));
    }
    let canonical = root.join("concord/contracts.yaml");
    let legacy = root.join("concord/guarantees.yaml");
    if canonical.try_exists()? && legacy.try_exists()? {
        bail!(
            "both concord/contracts.yaml and legacy concord/guarantees.yaml exist; select one definition source"
        )
    }
    let (document, source, old) = if canonical.try_exists()? {
        (
            serde_saphyr::from_str::<DefinitionDocument>(&fs::read_to_string(&canonical)?)?,
            "concord/contracts.yaml",
            false,
        )
    } else {
        let text = fs::read_to_string(&legacy)
            .context("read concord/contracts.yaml (or legacy concord/guarantees.yaml)")?;
        let legacy: LegacyDocument = serde_saphyr::from_str(&text)?;
        if legacy.schema_version != 1 {
            bail!("unsupported legacy definition schema_version")
        }
        let mut document = DefinitionDocument {
            schema_version: 2,
            requirements: legacy.requirements,
            contracts: Vec::new(),
            properties: Vec::new(),
            relations: Vec::new(),
        };
        for g in legacy.guarantees {
            document.contracts.push(Contract {
                id: format!("legacy:{}", g.id),
                description: g.description.clone(),
                version: "legacy-1".into(),
                scope: "Legacy definition; review the scope before approval".into(),
                assumptions: Vec::new(),
                preconditions: Vec::new(),
                responsibility: "Legacy definition; review the responsibility before approval"
                    .into(),
                approval_status: ApprovalStatus::Proposed,
                requirement_ids: g.requirement_ids,
                property_ids: vec![g.id.clone()],
            });
            document.properties.push(Property {
                version: None,
                depends_on: Vec::new(),
                id: g.id.clone(),
                description: g.description,
                approval_status: ApprovalStatus::Proposed,
                implementations: g.implementations,
                obligations: vec![VerificationObligation {
                    id: format!("legacy:{}:tests", g.id),
                    method: VerificationMethod::Test,
                    tests: g.tests,
                    commands: Vec::new(),
                }],
            });
        }
        (document, "concord/guarantees.yaml", true)
    };
    validate(&document)?;
    Ok((document, source.into(), old))
}

pub fn validate(document: &DefinitionDocument) -> Result<()> {
    if document.schema_version != 2 {
        bail!("unsupported contract definition schema_version; expected 2")
    }
    for o in document
        .properties
        .iter()
        .flat_map(|p| &p.obligations)
        .chain(document.relations.iter().flat_map(|r| &r.obligations))
    {
        let mut ids = BTreeSet::new();
        if o.commands
            .iter()
            .any(|id| id.trim().is_empty() || !ids.insert(id))
        {
            bail!("empty/duplicate verification command reference: {}", o.id)
        }
    }
    let mut requirements = BTreeSet::new();
    for r in &document.requirements {
        if r.id.trim().is_empty() || r.description.trim().is_empty() || !requirements.insert(&r.id)
        {
            bail!("invalid/duplicate requirement id")
        }
    }
    let mut properties = BTreeSet::new();
    let mut obligations = BTreeSet::new();
    for p in &document.properties {
        if p.id.trim().is_empty() || p.description.trim().is_empty() || !properties.insert(&p.id) {
            bail!("invalid/duplicate property id")
        }
        for path in &p.implementations {
            git::safe_path(path)?;
        }
        for o in &p.obligations {
            if o.id.trim().is_empty() || !obligations.insert(&o.id) {
                bail!("invalid/duplicate verification obligation id")
            }
            let mut tests = BTreeSet::new();
            for test in &o.tests {
                git::safe_path(&test.file)?;
                if test.name.trim().is_empty() || !tests.insert((&test.file, &test.name)) {
                    bail!("invalid/duplicate test reference in obligation {}", o.id)
                }
            }
        }
    }
    let mut contracts = BTreeSet::new();
    let mut linked = BTreeSet::new();
    for c in &document.contracts {
        if c.id.trim().is_empty()
            || !contracts.insert(&c.id)
            || [&c.description, &c.version, &c.scope, &c.responsibility]
                .iter()
                .any(|s| s.trim().is_empty())
            || c.assumptions
                .iter()
                .chain(&c.preconditions)
                .any(|s| s.trim().is_empty())
        {
            bail!("invalid/duplicate contract or empty metadata: {}", c.id)
        }
        if c.requirement_ids.is_empty()
            || c.requirement_ids
                .iter()
                .any(|id| !requirements.contains(id))
        {
            bail!("contract {} has unknown/missing requirement", c.id)
        }
        if document.relations.is_empty() && c.property_ids.is_empty()
            || c.property_ids.iter().any(|id| !properties.contains(id))
        {
            bail!("contract {} has unknown/missing property", c.id)
        }
        if c.property_ids.iter().collect::<BTreeSet<_>>().len() != c.property_ids.len() {
            bail!("duplicate Property reference in Contract {}", c.id)
        }
        linked.extend(c.property_ids.iter());
    }
    if !document.relations.is_empty()
        && document
            .contracts
            .iter()
            .any(|c| !c.property_ids.is_empty())
    {
        bail!("do not mix Contract property_ids with explicit relations")
    }
    let mut relation_ids = BTreeSet::new();
    for relation in &document.relations {
        if relation.id.trim().is_empty() || !relation_ids.insert(&relation.id) {
            bail!("invalid/duplicate relation id")
        }
        if !contracts.contains(&relation.contract_id) || !properties.contains(&relation.property_id)
        {
            bail!("relation {} has unknown Contract/Property", relation.id)
        }
        if relation
            .applicability
            .as_ref()
            .is_some_and(|a| a.condition.trim().is_empty())
        {
            bail!("empty applicability condition: {}", relation.id)
        }
        for path in &relation.implementations {
            git::safe_path(path)?;
        }
        let mut obligation_ids = BTreeSet::new();
        for o in &relation.obligations {
            if o.id.trim().is_empty() || !obligation_ids.insert(&o.id) {
                bail!("duplicate/empty obligation in relation {}", relation.id)
            }
            let mut tests = BTreeSet::new();
            for t in &o.tests {
                git::safe_path(&t.file)?;
                if t.name.trim().is_empty() || !tests.insert((&t.file, &t.name)) {
                    bail!("invalid/duplicate relation test reference")
                }
            }
        }
        linked.insert(&relation.property_id);
    }
    for p in &document.properties {
        if p.version.as_ref().is_some_and(|s| s.trim().is_empty()) {
            bail!("empty Property version")
        }
        let mut dependencies = BTreeSet::new();
        for dependency in &p.depends_on {
            if !properties.contains(dependency) || !dependencies.insert(dependency) {
                bail!("unknown/duplicate Property dependency")
            }
        }
    }
    if !document.relations.is_empty() {
        for p in &document.properties {
            if p.version.is_none() {
                bail!("explicit relations require a Property version: {}", p.id)
            }
            if !p.obligations.is_empty()
                && !document
                    .relations
                    .iter()
                    .any(|r| r.property_id == p.id && r.relation == RelationKind::ContributesTo)
            {
                bail!(
                    "requires verification obligations belong to the relation: {}",
                    p.id
                )
            }
        }
        for r in &document.relations {
            let p = document
                .properties
                .iter()
                .find(|p| p.id == r.property_id)
                .expect("validated Property");
            if r.relation == RelationKind::ContributesTo
                && r.obligations
                    .iter()
                    .any(|o| p.obligations.iter().any(|shared| shared.id == o.id))
            {
                bail!(
                    "relation and composed Property obligation IDs overlap: {}",
                    r.id
                )
            }
        }
    }
    let mut resolved = BTreeSet::new();
    loop {
        let before = resolved.len();
        for p in &document.properties {
            if p.depends_on.iter().all(|id| resolved.contains(id)) {
                resolved.insert(p.id.clone());
            }
        }
        if resolved.len() == before {
            break;
        }
    }
    if resolved.len() != document.properties.len() {
        bail!("cyclic Property dependencies")
    }
    if document.properties.iter().any(|p| !linked.contains(&p.id)) {
        bail!("property is not referenced by any contract")
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyDocument {
    schema_version: u32,
    #[serde(default)]
    requirements: Vec<Requirement>,
    #[serde(default)]
    guarantees: Vec<LegacyGuarantee>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyGuarantee {
    id: String,
    description: String,
    requirement_ids: Vec<String>,
    implementations: Vec<String>,
    tests: Vec<TestRef>,
}
