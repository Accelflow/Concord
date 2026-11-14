use crate::{
    definitions,
    model::{ApprovalStatus, Config, DefinitionDocument},
};
use serde_json::{Value, json};
use std::{fs, path::Path};

/// Authoring-time checks only: no checkout, runner, state database, or source writes.
pub fn check_file(path: &Path) -> Value {
    check(path, None)
}

pub fn check(path: &Path, config_path: Option<&Path>) -> Value {
    let mut report = json!({"schema_version":3,"file":path.to_string_lossy(),"status":"invalid","valid":false,"verification_performed":false,"counts":null,"diagnostics":[],"warnings":[]});
    if !path.exists() {
        report["diagnostics"] = json!([{"code":"file_read_error","message":format!("definition source not found: {}", path.display())}]);
        return report;
    }
    let parsed = (|| -> anyhow::Result<DefinitionDocument> {
        let document = crate::definition_source::read(path)?;
        definitions::validate(&document)?;
        Ok(document)
    })();
    match parsed {
        Err(error) => {
            report["diagnostics"] =
                json!([{"code":"definition_invalid","message":format!("{error:#}")}]);
        }
        Ok(document) => {
            if let Some(config_path) = config_path {
                report["configuration_checked"] = json!(true);
                let result = validate_configuration(config_path, &document);
                if let Err(error) = result {
                    report["diagnostics"] = json!([{"code":"command_configuration_invalid","message":format!("{error:#}")}]);
                    return report;
                }
            }
            let mut warnings = Vec::new();
            if document.requirements.is_empty()
                && document.contracts.is_empty()
                && document.properties.is_empty()
            {
                warnings.push(json!({"code":"empty_document","message":"No requirements, contracts, or properties are defined"}));
            }
            for c in &document.contracts {
                if c.approval_status != ApprovalStatus::Approved {
                    warnings.push(json!({"code":"contract_not_approved","id":c.id,"message":"Contract is a draft or deprecated; format validity does not approve it"}));
                }
            }
            for p in &document.properties {
                if p.approval_status != ApprovalStatus::Approved {
                    warnings.push(json!({"code":"property_not_approved","id":p.id,"message":"Property is a draft or deprecated; format validity does not approve it"}));
                }
                if p.obligations.is_empty()
                    && !document
                        .resolved_relations()
                        .iter()
                        .any(|r| r.property_id == p.id && !r.obligations.is_empty())
                {
                    warnings.push(json!({"code":"no_obligations","id":p.id,"message":"Property has no verification obligations"}));
                }
                for o in &p.obligations {
                    if o.tests.is_empty() && o.commands.is_empty() {
                        warnings.push(json!({"code":"no_tests","id":o.id,"message":"Verification obligation has no test references"}));
                    }
                }
            }
            for r in document.resolved_relations() {
                for o in &r.obligations {
                    if o.tests.is_empty() && o.commands.is_empty() {
                        warnings.push(json!({"code":"no_tests","id":o.id,"relation_id":r.id,"message":"Relation obligation has no test references"}));
                    }
                }
                if r.obligations.is_empty() {
                    warnings.push(json!({"code":"no_relation_obligations","id":r.id,"message":"Relation has no verification obligations"}));
                }
            }
            report["status"] = json!("valid");
            report["valid"] = json!(true);
            report["counts"] = json!({"requirements":document.requirements.len(),"relations":document.resolved_relations().len(),"contracts":document.contracts.len(),"properties":document.properties.len(),"obligations":document.properties.iter().map(|p|p.obligations.len()).sum::<usize>() + document.relations.iter().map(|r|r.obligations.len()).sum::<usize>()});
            report["warnings"] = json!(warnings);
        }
    }
    report
}

fn validate_configuration(path: &Path, document: &DefinitionDocument) -> anyhow::Result<()> {
    let config = Config::load(path)?;
    let root = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?;
    crate::commands::validate(&config, document, |relative| {
        let file = root.join(relative);
        fs::symlink_metadata(&file).is_ok_and(|metadata| metadata.is_file())
            && file
                .canonicalize()
                .is_ok_and(|path| path.starts_with(&root))
    })
}
