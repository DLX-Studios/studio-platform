#![allow(missing_docs)]

use studio_testkit::wasm::{
    FunctionBody, FunctionSignature, FunctionSpec, MemorySpec, TableSpec, ValueType,
    WasmFixtureBuilder,
};
use studio_wasm::{ModulePolicy, PolicyErrorCode, SandboxEngine};

fn signature(parameters: &[ValueType], results: &[ValueType]) -> FunctionSignature {
    FunctionSignature::new(parameters.iter().copied(), results.iter().copied())
}

fn abi_builder() -> WasmFixtureBuilder {
    WasmFixtureBuilder::new()
        .import_function(
            "studio_host",
            "emit",
            signature(&[ValueType::I32, ValueType::I32], &[ValueType::I32]),
        )
        .memory(MemorySpec::new(1).maximum(256).export_as("memory"))
        .table(TableSpec::new(0).maximum(1024))
        .function(FunctionSpec::exported(
            "studio_alloc",
            signature(&[ValueType::I32], &[ValueType::I32]),
            FunctionBody::Trap,
        ))
        .function(FunctionSpec::exported(
            "studio_dealloc",
            signature(&[ValueType::I32, ValueType::I32], &[]),
            FunctionBody::Trap,
        ))
        .function(FunctionSpec::exported(
            "studio_init",
            signature(&[ValueType::I32, ValueType::I32], &[ValueType::I32]),
            FunctionBody::Trap,
        ))
        .function(FunctionSpec::exported(
            "studio_event",
            signature(&[ValueType::I32, ValueType::I32], &[ValueType::I32]),
            FunctionBody::Trap,
        ))
}

fn validate(builder: WasmFixtureBuilder) -> Result<(), PolicyErrorCode> {
    let fixture = builder.build().unwrap();
    let engine = SandboxEngine::new().unwrap();
    ModulePolicy::default()
        .validate(&engine, fixture.wasm())
        .map(|_| ())
        .map_err(|error| error.code())
}

#[test]
fn accepts_exact_studio_v1_surface() {
    validate(abi_builder()).unwrap();
}

#[test]
fn rejects_missing_extra_wrong_and_wasi_imports() {
    let without_import = WasmFixtureBuilder::new()
        .memory(MemorySpec::new(1).maximum(256).export_as("memory"))
        .table(TableSpec::new(0).maximum(1024));
    assert_eq!(
        validate(without_import).unwrap_err(),
        PolicyErrorCode::ImportInvalid
    );

    for builder in [
        abi_builder().import_function("wasi_snapshot_preview1", "fd_write", signature(&[], &[])),
        abi_builder().import_function("studio_host", "extra", signature(&[], &[])),
        WasmFixtureBuilder::new().import_function(
            "guest_selected",
            "emit",
            signature(&[ValueType::I32, ValueType::I32], &[ValueType::I32]),
        ),
    ] {
        assert_eq!(
            validate(builder).unwrap_err(),
            PolicyErrorCode::ImportInvalid
        );
    }
}

#[test]
fn rejects_missing_user_and_assemblyscript_runtime_exports() {
    let missing_exports = WasmFixtureBuilder::new()
        .import_function(
            "studio_host",
            "emit",
            signature(&[ValueType::I32, ValueType::I32], &[ValueType::I32]),
        )
        .memory(MemorySpec::new(1).maximum(256).export_as("memory"))
        .table(TableSpec::new(0).maximum(1024));
    assert_eq!(
        validate(missing_exports).unwrap_err(),
        PolicyErrorCode::ExportInvalid
    );

    for export in ["guest_extra", "__new", "__pin", "__rtti_base"] {
        assert_eq!(
            validate(abi_builder().function(FunctionSpec::exported(
                export,
                signature(&[], &[]),
                FunctionBody::Trap,
            )))
            .unwrap_err(),
            PolicyErrorCode::ExportInvalid
        );
    }
}

#[test]
fn rejects_wrong_required_function_signatures() {
    let builder = WasmFixtureBuilder::new()
        .import_function(
            "studio_host",
            "emit",
            signature(&[ValueType::I32, ValueType::I32], &[ValueType::I32]),
        )
        .memory(MemorySpec::new(1).maximum(256).export_as("memory"))
        .table(TableSpec::new(0).maximum(1024))
        .function(FunctionSpec::exported(
            "studio_alloc",
            signature(&[ValueType::I64], &[ValueType::I32]),
            FunctionBody::Trap,
        ))
        .function(FunctionSpec::exported(
            "studio_dealloc",
            signature(&[ValueType::I32, ValueType::I32], &[]),
            FunctionBody::Trap,
        ))
        .function(FunctionSpec::exported(
            "studio_init",
            signature(&[ValueType::I32, ValueType::I32], &[ValueType::I32]),
            FunctionBody::Trap,
        ))
        .function(FunctionSpec::exported(
            "studio_event",
            signature(&[ValueType::I32, ValueType::I32], &[ValueType::I32]),
            FunctionBody::Trap,
        ));
    assert_eq!(
        validate(builder).unwrap_err(),
        PolicyErrorCode::ExportInvalid
    );
}

#[test]
fn rejects_unbounded_or_oversized_memory_and_missing_or_unbounded_table() {
    let unbounded_memory = WasmFixtureBuilder::new()
        .import_function(
            "studio_host",
            "emit",
            signature(&[ValueType::I32, ValueType::I32], &[ValueType::I32]),
        )
        .memory(MemorySpec::new(1).export_as("memory"))
        .table(TableSpec::new(0).maximum(1024));
    assert_eq!(
        validate(unbounded_memory).unwrap_err(),
        PolicyErrorCode::MemoryInvalid
    );

    let oversized_memory = abi_builder().memory(MemorySpec::new(1).maximum(257));
    assert!(matches!(
        validate(oversized_memory).unwrap_err(),
        PolicyErrorCode::CompilationRejected | PolicyErrorCode::MemoryInvalid
    ));

    let missing_table = WasmFixtureBuilder::new()
        .import_function(
            "studio_host",
            "emit",
            signature(&[ValueType::I32, ValueType::I32], &[ValueType::I32]),
        )
        .memory(MemorySpec::new(1).maximum(256).export_as("memory"));
    assert_eq!(
        validate(missing_table).unwrap_err(),
        PolicyErrorCode::TableInvalid
    );

    let unbounded_table = abi_builder().table(TableSpec::new(0));
    assert_eq!(
        validate(unbounded_table).unwrap_err(),
        PolicyErrorCode::TableInvalid
    );
}

#[test]
fn rejects_disabled_threads_simd_memory64_and_multi_memory_proposals() {
    let engine = SandboxEngine::new().unwrap();
    let policy = ModulePolicy::default();
    for wat in [
        "(module (memory 1 1 shared))",
        "(module (func (param v128)))",
        "(module (memory i64 1 1))",
        "(module (memory 1 1) (memory 1 1))",
    ] {
        let wasm = wat::parse_str(wat).unwrap();
        assert_eq!(
            policy.validate(&engine, &wasm).unwrap_err().code(),
            PolicyErrorCode::CompilationRejected
        );
    }
}
