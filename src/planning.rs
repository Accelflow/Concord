use crate::model::*;
use std::collections::BTreeSet;

pub fn obligations(
    document: &DefinitionDocument,
    r: &ContractPropertyRelation,
) -> Vec<VerificationObligation> {
    let mut values = r.obligations.clone();
    if r.relation == RelationKind::ContributesTo
        && let Some(p) = document.properties.iter().find(|p| p.id == r.property_id)
    {
        values.extend(p.obligations.clone());
    }
    values
}

pub fn apply_properties(
    snapshot: &Snapshot,
    touched: &BTreeSet<String>,
    all: bool,
    selected: &mut BTreeSet<String>,
    changed_commands: &BTreeSet<&str>,
) -> (
    BTreeSet<String>,
    BTreeSet<String>,
    BTreeSet<String>,
    Vec<String>,
) {
    let document = &snapshot.definitions;
    let relations = document.resolved_relations();
    let mut direct = BTreeSet::new();
    let mut edge_targets = BTreeSet::new();
    let command_touched = |o: &VerificationObligation| {
        o.commands
            .iter()
            .any(|id| changed_commands.contains(id.as_str()))
    };
    for p in &document.properties {
        if p.implementations.iter().any(|f| touched.contains(f))
            || p.obligations.iter().any(command_touched)
            || p.obligations
                .iter()
                .flat_map(|o| &o.tests)
                .any(|t| touched.contains(&t.file))
        {
            direct.insert(p.id.clone());
        }
    }
    for r in &relations {
        if r.implementations.iter().any(|f| touched.contains(f))
            || r.obligations.iter().any(command_touched)
            || r.obligations
                .iter()
                .flat_map(|o| &o.tests)
                .any(|t| touched.contains(&t.file))
        {
            edge_targets.insert(r.id.clone());
        }
    }
    // Expand a changed relation to its Contract's other verification obligations.
    // A shared Property alone must not activate unrelated Contracts. Only explicit
    // Property dependencies cross that context boundary.
    let mut active_edges = edge_targets.clone();
    active_edges.extend(
        relations
            .iter()
            .filter(|r| all || direct.contains(&r.property_id))
            .map(|r| r.id.clone()),
    );
    loop {
        let before = active_edges.len();
        let active_contracts: BTreeSet<_> = relations
            .iter()
            .filter(|r| active_edges.contains(&r.id))
            .map(|r| &r.contract_id)
            .collect();
        let impacted: BTreeSet<_> = relations
            .iter()
            .filter(|r| active_edges.contains(&r.id))
            .map(|r| &r.property_id)
            .collect();
        let mut propagated = BTreeSet::new();
        for p in &document.properties {
            if p.depends_on.iter().any(|id| impacted.contains(id)) {
                propagated.insert(&p.id);
            }
        }
        let additional: Vec<_> = relations
            .iter()
            .filter(|r| {
                active_contracts.contains(&r.contract_id) || propagated.contains(&r.property_id)
            })
            .map(|r| r.id.clone())
            .collect();
        active_edges.extend(additional);
        if active_edges.len() == before {
            break;
        }
    }
    let mut properties = BTreeSet::new();
    let mut contracts = BTreeSet::new();
    let mut edges = BTreeSet::new();
    let mut gaps = Vec::new();
    for r in &relations {
        if !active_edges.contains(&r.id) {
            continue;
        }
        let p = document
            .properties
            .iter()
            .find(|p| p.id == r.property_id)
            .expect("validated Property");
        let c = document
            .contracts
            .iter()
            .find(|c| c.id == r.contract_id)
            .expect("validated Contract");
        properties.insert(p.id.clone());
        contracts.insert(c.id.clone());
        edges.insert(r.id.clone());
        let prefix = format!("relation:{:?}:", r.id);
        if p.approval_status != ApprovalStatus::Approved {
            gaps.push(format!("{prefix}property_not_approved"));
        }
        if c.approval_status != ApprovalStatus::Approved {
            gaps.push(format!("{prefix}contract_not_approved"));
        }
        for path in p.implementations.iter().chain(&r.implementations) {
            if !snapshot.files.contains_key(path) {
                gaps.push(format!("{prefix}missing_implementation:{path}"));
            }
        }
        let needed = obligations(document, r);
        if r.obligations.is_empty() {
            gaps.push(format!("{prefix}no_relation_obligations"));
        }
        if r.relation == RelationKind::ContributesTo
            && !p.obligations.iter().any(|o| {
                matches!(
                    o.method,
                    VerificationMethod::Integration | VerificationMethod::E2e
                )
            })
        {
            gaps.push(format!(
                "{prefix}composed_property_requires_integration_or_e2e"
            ));
        }
        for o in needed {
            if o.tests.is_empty() && o.commands.is_empty() {
                gaps.push(format!("{prefix}obligation:{}:no_tests", o.id));
            }
            for t in &o.tests {
                if snapshot
                    .tests
                    .iter()
                    .any(|case| case.file == t.file && case.name == t.name)
                {
                    selected.insert(t.file.clone());
                } else {
                    gaps.push(format!(
                        "{prefix}obligation:{}:missing_test:{}:{}",
                        o.id, t.file, t.name
                    ));
                }
            }
        }
    }
    gaps.sort();
    gaps.dedup();
    (properties, contracts, edges, gaps)
}

pub fn evaluate(run: &Run) -> (Vec<PropertyResult>, Vec<ObligationResult>, Vec<String>) {
    let mut rows = Vec::new();
    let mut results = Vec::new();
    let mut gaps = run.plan.property_gaps.clone();
    for r in &run.plan.relations {
        let p = run
            .plan
            .properties
            .iter()
            .find(|p| p.id == r.property_id)
            .expect("planned Property");
        let c = run
            .plan
            .contracts
            .iter()
            .find(|c| c.id == r.contract_id)
            .expect("planned Contract");
        let needed: Vec<_> = run
            .plan
            .required_obligations
            .iter()
            .filter(|o| o.relation_id == r.id)
            .collect();
        let mut statuses = Vec::new();
        for o in needed {
            let found: Vec<_> = o
                .obligation
                .tests
                .iter()
                .map(|t| {
                    run.cases
                        .iter()
                        .find(|case| case.file == t.file && case.name == t.name)
                })
                .collect();
            let command_results: Vec<_> = o
                .obligation
                .commands
                .iter()
                .map(|id| run.command_results.iter().find(|c| &c.id == id))
                .collect();
            let status = if !run.input_unchanged {
                VerificationStatus::Stale
            } else if !run.result_integrity {
                VerificationStatus::Unverified
            } else if command_results
                .iter()
                .any(|c| c.is_some_and(|c| c.status == "failed"))
                || found
                    .iter()
                    .any(|t| t.is_some_and(|t| t.status == "failed"))
            {
                VerificationStatus::Failed
            } else if (!found.is_empty() || !command_results.is_empty())
                && command_results
                    .iter()
                    .all(|c| c.is_some_and(|c| c.status == "passed"))
                && found
                    .iter()
                    .all(|t| t.is_some_and(|t| t.status == "passed"))
            {
                VerificationStatus::Verified
            } else {
                VerificationStatus::Unverified
            };
            if status != VerificationStatus::Verified {
                gaps.push(format!(
                    "relation:{:?}:obligation:{}:unverified",
                    r.id, o.obligation.id
                ));
            }
            statuses.push(status.clone());
            results.push(ObligationResult {
                relation_id: r.id.clone(),
                property_id: p.id.clone(),
                obligation_id: o.obligation.id.clone(),
                method: o.obligation.method.clone(),
                status,
                command_ids: o.obligation.commands.clone(),
            });
        }
        let prefix = format!("relation:{:?}:", r.id);
        let status = if !run.input_unchanged {
            VerificationStatus::Stale
        } else if statuses.contains(&VerificationStatus::Failed) {
            VerificationStatus::Failed
        } else if run.result_integrity
            && p.approval_status == ApprovalStatus::Approved
            && c.approval_status == ApprovalStatus::Approved
            && !statuses.is_empty()
            && statuses.iter().all(|s| *s == VerificationStatus::Verified)
            && !run
                .plan
                .property_gaps
                .iter()
                .any(|g| g.starts_with(&prefix))
        {
            VerificationStatus::Verified
        } else {
            VerificationStatus::Unverified
        };
        if status != VerificationStatus::Verified {
            gaps.push(format!("{prefix}property_unverified"));
        }
        let result = match status {
            VerificationStatus::Verified => "passed",
            VerificationStatus::Failed => "failed",
            _ => "inconclusive",
        };
        rows.push(PropertyResult {
            relation_id: r.id.clone(),
            contract_id: c.id.clone(),
            relation: r.relation.clone(),
            applicability: r.applicability.clone(),
            contract_version: c.version.clone(),
            property_version: p.version.clone().unwrap_or_else(|| "1".into()),
            scope: c.scope.clone(),
            commit_sha: run.plan.head_sha.clone(),
            run_id: run.run_id.clone(),
            environment_fingerprint: run.environment_fingerprint.clone(),
            input_fingerprint: run.plan.fingerprint.clone(),
            applicability_evaluated: false,
            result: result.into(),
            property_id: p.id.clone(),
            approval_status: p.approval_status.clone(),
            status,
            obligation_ids: run
                .plan
                .required_obligations
                .iter()
                .filter(|o| o.relation_id == r.id)
                .map(|o| o.obligation.id.clone())
                .collect(),
        });
    }
    gaps.sort();
    gaps.dedup();
    (rows, results, gaps)
}
