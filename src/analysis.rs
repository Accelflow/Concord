use crate::model::Graph;
use anyhow::{Context, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use tree_sitter::{Node, Parser};

pub fn graph(root: &Path, files: &BTreeMap<String, String>) -> Result<Graph> {
    let mut g = Graph::default();
    for path in files.keys() {
        let ext = Path::new(path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if !["ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts"].contains(&ext) {
            continue;
        }
        if path.ends_with(".d.ts") {
            g.uncertainties.push(format!("ambient_types:{path}"));
        }
        let source = fs::read_to_string(root.join(path))?;
        let mut parser = Parser::new();
        let language = if ext == "tsx" {
            tree_sitter_typescript::LANGUAGE_TSX.into()
        } else if ["ts", "mts", "cts"].contains(&ext) {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
        } else {
            tree_sitter_javascript::LANGUAGE.into()
        };
        parser.set_language(&language).context("parser language")?;
        let tree = parser.parse(&source, None).context("parse source")?;
        let mut dependencies = BTreeSet::new();
        if tree.root_node().has_error() {
            g.uncertainties.push(format!("parse_error:{path}"));
        }
        visit(
            tree.root_node(),
            source.as_bytes(),
            path,
            files,
            &mut dependencies,
            &mut g.uncertainties,
        )?;
        g.dependencies.insert(path.clone(), dependencies);
    }
    Ok(g)
}
fn visit(
    node: Node,
    source: &[u8],
    path: &str,
    files: &BTreeMap<String, String>,
    deps: &mut BTreeSet<String>,
    unknown: &mut Vec<String>,
) -> Result<()> {
    if ["import_statement", "export_statement"].contains(&node.kind()) {
        if let Some(src) = node.child_by_field_name("source") {
            let raw = src.utf8_text(source)?;
            let spec = &raw[1..raw.len().saturating_sub(1)];
            if spec.contains('\\') {
                unknown.push(format!("escaped_import:{path}"));
            } else if spec.starts_with('.') {
                let parent = Path::new(path).parent().unwrap();
                let mut parts = Vec::new();
                let mut invalid = false;
                for c in parent.join(spec).components() {
                    match c {
                        std::path::Component::Normal(v) => {
                            parts.push(v.to_string_lossy().into_owned())
                        }
                        std::path::Component::ParentDir => {
                            if parts.pop().is_none() {
                                invalid = true
                            }
                        }
                        std::path::Component::CurDir => {}
                        _ => invalid = true,
                    }
                }
                let base = parts.join("/");
                let mut candidates = vec![base.clone()];
                if Path::new(&base).extension().is_none() {
                    for ext in ["ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts"] {
                        candidates.push(format!("{base}.{ext}"));
                        candidates.push(format!("{base}/index.{ext}"));
                    }
                }
                let matches: Vec<_> = candidates
                    .iter()
                    .filter(|p| files.contains_key(*p))
                    .collect();
                if !invalid && matches.len() == 1 && is_source(matches[0]) {
                    deps.insert(matches[0].clone());
                } else {
                    unknown.push(format!("unresolved_import:{path}:{spec}"));
                }
            } else if spec != "vitest" {
                unknown.push(format!("external_dependency:{path}:{spec}"));
            }
        } else if node.kind() == "import_statement" {
            unknown.push(format!("unsupported_import:{path}"));
        }
    }
    if node.kind() == "call_expression"
        && let Some(f) = node.child_by_field_name("function")
        && (["import", "require", "eval"].contains(&f.utf8_text(source)?)
            || f.utf8_text(source)?.ends_with(".require"))
    {
        unknown.push(format!("dynamic_dependency:{path}"));
    }
    if [
        "import_require_clause",
        "import_alias",
        "internal_module",
        "ambient_declaration",
        "meta_property",
    ]
    .contains(&node.kind())
    {
        unknown.push(format!("unsupported_syntax:{path}:{}", node.kind()));
    }
    if node.kind() == "identifier" && ["require", "eval"].contains(&node.utf8_text(source)?) {
        unknown.push(format!("dynamic_reference:{path}"));
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit(child, source, path, files, deps, unknown)?;
    }
    Ok(())
}
pub fn is_source(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|x| ["ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts"].contains(&x))
}
