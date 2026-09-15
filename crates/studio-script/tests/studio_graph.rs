//! US4 module-graph contract: static resolution, named cycles, ordered
//! invalidation, and virtual contracts — over realistic `.studio` import
//! shapes.
use studio_script::graph::{ModuleEdges, ModuleGraph, is_virtual_module, scan_imports};

fn edges(id: &str, imports: &[&str]) -> ModuleEdges {
    ModuleEdges {
        id: id.to_owned(),
        imports: imports.iter().map(|name| (*name).to_owned()).collect(),
    }
}

#[test]
fn studio_import_shapes_resolve_and_invalidate_in_order() {
    let graph = ModuleGraph::build(&[
        edges(
            "app",
            &["./screens/home.studio", "@studio/generated/routes"],
        ),
        edges("./screens/home.studio", &["../components/card.studio"]),
        edges("../components/card.studio", &[]),
    ])
    .unwrap();
    assert_eq!(
        graph.invalidate("../components/card.studio"),
        vec!["../components/card.studio", "./screens/home.studio", "app",]
    );
}

#[test]
fn studio_cycles_name_every_member() {
    let error = ModuleGraph::build(&[
        edges("a.studio", &["b.studio"]),
        edges("b.studio", &["c.studio"]),
        edges("c.studio", &["a.studio"]),
    ])
    .unwrap_err();
    assert_eq!(error.code, "STUDIO340");
    for member in ["a.studio", "b.studio", "c.studio"] {
        assert!(error.message.contains(member), "cycle names {member}");
    }
}

#[test]
fn scanner_reads_static_imports_and_ignores_dynamic_calls() {
    let script = "import { Card } from \"./card.studio\";\nimport type { Props } from './types';\nconst name = \"card\";\nshow(import(name));";
    assert_eq!(
        scan_imports(script),
        vec!["./card.studio".to_owned(), "./types".to_owned()]
    );
    assert!(is_virtual_module("@studio/bootstrap"));
}
