//! Module graph: static imports, cycle detection, and ordered invalidation.
//!
//! The graph is pure logic over declared edges: modules name their imports,
//! the graph resolves them, rejects cycles by naming the cycle, and orders
//! dependent reevaluation so dependencies always rebuild before dependents.

use std::collections::{BTreeMap, BTreeSet};

/// Reserved host-provided contracts that resolve without a source module.
pub const VIRTUAL_MODULES: &[&str] = &[
    "@studio/generated/routes",
    "@studio/generated/assets",
    "@studio/dev-runtime",
    "@studio/bootstrap",
];

/// Whether a module identity names a reserved virtual contract.
#[must_use]
pub fn is_virtual_module(name: &str) -> bool {
    VIRTUAL_MODULES.contains(&name)
}

/// One module's declared edges.
#[derive(Clone, Debug, PartialEq)]
pub struct ModuleEdges {
    /// Stable module identity.
    pub id: String,
    /// Imported module identities in source order (duplicates allowed).
    pub imports: Vec<String>,
}

/// Directed import graph with dependents indexed.
#[derive(Clone, Debug, Default)]
pub struct ModuleGraph {
    modules: BTreeMap<String, BTreeSet<String>>,
    dependents: BTreeMap<String, BTreeSet<String>>,
}

/// Graph construction or invalidation failure.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphError {
    /// Stable code (`STUDIO340` family).
    pub code: &'static str,
    /// Safe message naming the modules involved.
    pub message: String,
}

impl GraphError {
    fn unresolvable(from: &str, name: &str) -> Self {
        Self {
            code: crate::CODE_MODULE_GRAPH,
            message: format!("module `{from}` imports unresolvable `{name}`"),
        }
    }

    fn cycle(cycle: &[String]) -> Self {
        Self {
            code: crate::CODE_MODULE_GRAPH,
            message: format!("import cycle detected: {}", cycle.join(" -> ")),
        }
    }
}

impl ModuleGraph {
    /// Build the graph from declared edges, resolving every import.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError`] for unresolvable imports or import cycles.
    pub fn build(modules: &[ModuleEdges]) -> Result<Self, GraphError> {
        let known: BTreeSet<&str> = modules.iter().map(|module| module.id.as_str()).collect();
        let mut graph = Self::default();
        for module in modules {
            let mut edges = BTreeSet::new();
            for import in &module.imports {
                if is_virtual_module(import) {
                    continue;
                }
                if !known.contains(import.as_str()) {
                    return Err(GraphError::unresolvable(&module.id, import));
                }
                if import != &module.id {
                    edges.insert(import.clone());
                }
            }
            graph.dependents.entry(module.id.clone()).or_default();
            for edge in &edges {
                graph
                    .dependents
                    .entry(edge.clone())
                    .or_default()
                    .insert(module.id.clone());
            }
            graph.modules.insert(module.id.clone(), edges);
        }
        // Self-imports are degenerate cycles only when nothing else imports
        // the module; treat explicit self-import as a one-node cycle.
        for module in modules {
            if module.imports.iter().any(|import| import == &module.id) {
                return Err(GraphError::cycle(&[module.id.clone(), module.id.clone()]));
            }
        }
        graph.check_cycles()?;
        Ok(graph)
    }

    /// Modules to reevaluate when `changed` edits, in dependency order:
    /// the changed module first, then dependents after their own
    /// dependencies. Deterministic (sorted) within each level.
    #[must_use]
    pub fn invalidate(&self, changed: &str) -> Vec<String> {
        let mut ordered = Vec::new();
        let mut visited = BTreeSet::new();
        let mut queue = vec![changed.to_owned()];
        visited.insert(changed.to_owned());
        while !queue.is_empty() {
            let mut level: Vec<String> = std::mem::take(&mut queue);
            level.sort();
            for id in level {
                ordered.push(id.clone());
                if let Some(dependents) = self.dependents.get(&id) {
                    for dependent in dependents {
                        if visited.insert(dependent.clone()) {
                            queue.push(dependent.clone());
                        }
                    }
                }
            }
        }
        ordered
    }

    fn check_cycles(&self) -> Result<(), GraphError> {
        let mut visiting = BTreeSet::new();
        let mut done = BTreeSet::new();
        let mut stack: Vec<String> = Vec::new();
        let mut ids: Vec<&String> = self.modules.keys().collect();
        ids.sort();
        for id in ids {
            self.visit(id, &mut visiting, &mut done, &mut stack)?;
        }
        Ok(())
    }

    fn visit(
        &self,
        id: &str,
        visiting: &mut BTreeSet<String>,
        done: &mut BTreeSet<String>,
        stack: &mut Vec<String>,
    ) -> Result<(), GraphError> {
        if done.contains(id) {
            return Ok(());
        }
        if !visiting.insert(id.to_owned()) {
            let start = stack.iter().position(|member| member == id).unwrap_or(0);
            let mut cycle: Vec<String> = stack[start..].to_vec();
            cycle.push(id.to_owned());
            return Err(GraphError::cycle(&cycle));
        }
        stack.push(id.to_owned());
        if let Some(edges) = self.modules.get(id) {
            let mut sorted: Vec<&String> = edges.iter().collect();
            sorted.sort();
            for edge in sorted {
                self.visit(edge, visiting, done, stack)?;
            }
        }
        stack.pop();
        visiting.remove(id);
        done.insert(id.to_owned());
        Ok(())
    }
}

/// Scan owned script text for static `import ... from "..."` specifiers.
///
/// Only static string-literal sources are collected; dynamic imports are a
/// validator rejection, not graph input.
#[must_use]
pub fn scan_imports(script: &str) -> Vec<String> {
    let mut imports = Vec::new();
    let mut search = 0usize;
    while let Some(found) = script[search..].find("import") {
        let absolute = search + found;
        if !is_word_boundary(script, absolute, "import".len()) {
            search = absolute + 1;
            continue;
        }
        let rest = &script[absolute + 6..];
        let Some(from) = rest.find("from") else {
            search = absolute + 1;
            continue;
        };
        if !is_word_boundary(rest, from, "from".len()) {
            search = absolute + 1;
            continue;
        }
        let after = rest[from + 4..].trim_start();
        let quote = after.chars().next();
        if !matches!(quote, Some('"' | '\'')) {
            search = absolute + 1;
            continue;
        }
        let quote = quote.unwrap_or('"');
        let Some(end) = after[1..].find(quote) else {
            search = absolute + 1;
            continue;
        };
        let specifier = after[1..=end].to_owned();
        if !imports.contains(&specifier) {
            imports.push(specifier);
        }
        search = absolute + 1;
    }
    imports
}

fn is_word_boundary(text: &str, absolute: usize, length: usize) -> bool {
    let before = text[..absolute].chars().next_back();
    let after = text[absolute + length..].chars().next();
    let boundary = |character: Option<char>| {
        character.is_none_or(|character| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '$')
        })
    };
    boundary(before) && boundary(after)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edges(id: &str, imports: &[&str]) -> ModuleEdges {
        ModuleEdges {
            id: id.to_owned(),
            imports: imports.iter().map(|name| (*name).to_owned()).collect(),
        }
    }

    #[test]
    fn invalidation_orders_dependents_after_dependencies() {
        let graph = ModuleGraph::build(&[
            edges("app", &["./card.studio", "@studio/generated/routes"]),
            edges("./card.studio", &["./money.studio"]),
            edges("./money.studio", &[]),
        ])
        .unwrap();
        assert_eq!(
            graph.invalidate("./money.studio"),
            vec!["./money.studio", "./card.studio", "app"]
        );
        assert_eq!(graph.invalidate("app"), vec!["app"]);
    }

    #[test]
    fn cycles_name_the_cycle_and_missing_imports_fail() {
        let error = ModuleGraph::build(&[edges("a", &["b"]), edges("b", &["a"])]).unwrap_err();
        assert_eq!(error.code, "STUDIO340");
        assert!(error.message.contains('a') && error.message.contains('b'));

        let error = ModuleGraph::build(&[edges("a", &["./missing.studio"])]).unwrap_err();
        assert!(error.message.contains("./missing.studio"));

        // Virtual contracts resolve without sources.
        assert!(ModuleGraph::build(&[edges("a", &["@studio/bootstrap"])]).is_ok());
        assert!(is_virtual_module("@studio/generated/routes"));
        assert!(!is_virtual_module("./card.studio"));
    }

    #[test]
    fn import_scanner_collects_static_specifiers() {
        let script = "import { x } from \"./a.studio\";\nimport y from '@studio/bootstrap';\nconst lazy = import(\"./b.studio\");";
        assert_eq!(
            scan_imports(script),
            vec!["./a.studio".to_owned(), "@studio/bootstrap".to_owned()]
        );
        // Dynamic import() is not a static specifier.
        assert!(!scan_imports("const m = import(name);").contains(&"name".to_owned()));
    }
}
