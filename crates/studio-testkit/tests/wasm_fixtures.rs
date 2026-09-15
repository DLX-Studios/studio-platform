#![allow(missing_docs)]

use studio_testkit::wasm::{
    FunctionBody, FunctionSignature, FunctionSpec, MemorySpec, TableSpec, ValueType,
    WasmFixtureBuilder,
};

fn emit_signature() -> FunctionSignature {
    FunctionSignature::new([ValueType::I32, ValueType::I32], [ValueType::I32])
}

#[test]
fn generates_declared_imports_exports_memory_and_table_as_wat_and_wasm() {
    let fixture = WasmFixtureBuilder::new()
        .import_function("studio_host", "emit", emit_signature())
        .memory(MemorySpec::new(1).maximum(256).export_as("memory"))
        .table(TableSpec::new(1).maximum(8).export_as("callbacks"))
        .function(FunctionSpec::exported(
            "studio_init",
            FunctionSignature::new([ValueType::I32, ValueType::I32], [ValueType::I32]),
            FunctionBody::I32Const(0),
        ))
        .build()
        .unwrap();

    assert!(fixture.wat().contains(r#"(import "studio_host" "emit""#));
    assert!(
        fixture
            .wat()
            .contains(r#"(export "memory" (memory $memory))"#)
    );
    assert!(
        fixture
            .wat()
            .contains(r#"(export "callbacks" (table $table))"#)
    );
    assert!(
        fixture
            .wat()
            .contains(r#"(export "studio_init" (func $studio_init))"#)
    );
    assert_eq!(&fixture.wasm()[..4], b"\0asm");
}

#[test]
fn generates_deterministic_loop_and_trap_fixtures() {
    let build = || {
        WasmFixtureBuilder::new()
            .function(FunctionSpec::exported(
                "loop_forever",
                FunctionSignature::new([], []),
                FunctionBody::InfiniteLoop,
            ))
            .function(FunctionSpec::exported(
                "trap_now",
                FunctionSignature::new([], []),
                FunctionBody::Trap,
            ))
            .build()
            .unwrap()
    };

    let first = build();
    let second = build();
    assert_eq!(first.wat(), second.wat());
    assert_eq!(first.wasm(), second.wasm());
    assert!(first.wat().contains("(loop $spin (br $spin))"));
    assert!(first.wat().contains("unreachable"));
}

#[test]
fn generates_bad_abi_calls_without_executing_them() {
    let fixture = WasmFixtureBuilder::new()
        .import_function("studio_host", "emit", emit_signature())
        .function(FunctionSpec::exported(
            "bad_emit",
            FunctionSignature::new([], [ValueType::I32]),
            FunctionBody::CallImport {
                import_index: 0,
                i32_arguments: vec![-1, i32::MAX],
            },
        ))
        .build()
        .unwrap();

    assert!(fixture.wat().contains("(i32.const -1)"));
    assert!(fixture.wat().contains(&format!("(i32.const {})", i32::MAX)));
    assert!(fixture.wat().contains("call $import_0"));
    assert_eq!(&fixture.wasm()[..4], b"\0asm");
}

#[test]
fn rejects_fixture_declarations_that_cannot_form_valid_wasm() {
    let error = WasmFixtureBuilder::new()
        .function(FunctionSpec::exported(
            "bad_result",
            FunctionSignature::new([], [ValueType::I64]),
            FunctionBody::I32Const(0),
        ))
        .build()
        .unwrap_err();

    assert!(error.to_string().contains("failed to encode WAT fixture"));
}
