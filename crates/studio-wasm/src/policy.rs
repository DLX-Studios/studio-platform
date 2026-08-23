//! Pre-instantiation module surface and resource policy.

use std::collections::BTreeMap;

use thiserror::Error;
use wasmparser::{Parser, Payload, RefType};
use wasmtime::{ExternType, FuncType, Module, ValType};

use crate::SandboxEngine;

const WASM_PAGE_BYTES: u64 = 65_536;
const MAX_MEMORY_BYTES: u64 = 16 * 1024 * 1024;

/// Closed pre-instantiation policy for Studio protocol-v1 modules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModulePolicy {
    /// Maximum linear-memory pages (16 MiB at standard 64 KiB pages).
    pub max_memory_pages: u64,
    /// Maximum elements in the module's single table.
    pub max_table_elements: u64,
}

impl Default for ModulePolicy {
    fn default() -> Self {
        Self {
            max_memory_pages: MAX_MEMORY_BYTES / WASM_PAGE_BYTES,
            max_table_elements: 1024,
        }
    }
}

impl ModulePolicy {
    /// Compile and validate an untrusted module without instantiating it.
    ///
    /// # Errors
    ///
    /// Returns [`PolicyError`] for compilation, import/export, memory, or table violations.
    pub fn validate(
        &self,
        engine: &SandboxEngine,
        bytes: &[u8],
    ) -> Result<ValidatedModule, PolicyError> {
        let module = Module::new(&engine.inner, bytes)
            .map_err(|error| PolicyError::CompilationRejected(error.to_string()))?;
        Self::validate_imports(&module)?;
        self.validate_resources(bytes, &module)?;
        Self::validate_exports(&module)?;
        Ok(ValidatedModule { module })
    }

    fn validate_imports(module: &Module) -> Result<(), PolicyError> {
        let imports: Vec<_> = module.imports().collect();
        if imports.len() != 1 {
            return Err(PolicyError::ImportInvalid(
                "module must import exactly studio_host.emit".to_owned(),
            ));
        }
        let import = &imports[0];
        if import.module() != "studio_host" || import.name() != "emit" {
            return Err(PolicyError::ImportInvalid(
                "only studio_host.emit may be imported".to_owned(),
            ));
        }
        let ExternType::Func(function) = import.ty() else {
            return Err(PolicyError::ImportInvalid(
                "studio_host.emit must be a function".to_owned(),
            ));
        };
        if !signature_matches(&function, &[ValType::I32, ValType::I32], &[ValType::I32]) {
            return Err(PolicyError::ImportInvalid(
                "studio_host.emit must have (i32, i32) -> i32".to_owned(),
            ));
        }
        Ok(())
    }

    fn validate_resources(&self, bytes: &[u8], module: &Module) -> Result<(), PolicyError> {
        let memories: Vec<_> = module
            .exports()
            .filter_map(|export| match export.ty() {
                ExternType::Memory(memory) => Some((export.name(), memory)),
                _ => None,
            })
            .collect();
        if memories.len() != 1 || memories[0].0 != "memory" {
            return Err(PolicyError::MemoryInvalid(
                "exactly one memory export named memory is required".to_owned(),
            ));
        }
        let memory = &memories[0].1;
        if memory.is_64() || memory.is_shared() {
            return Err(PolicyError::MemoryInvalid(
                "memory64 and shared memory are forbidden".to_owned(),
            ));
        }
        let Some(maximum) = memory.maximum() else {
            return Err(PolicyError::MemoryInvalid(
                "linear memory must declare a maximum".to_owned(),
            ));
        };
        if maximum > self.max_memory_pages || memory.minimum() > maximum {
            return Err(PolicyError::MemoryInvalid(format!(
                "linear memory maximum exceeds {} pages",
                self.max_memory_pages
            )));
        }
        let mut declared_memories = Vec::new();
        let mut declared_tables = Vec::new();
        for payload in Parser::new(0).parse_all(bytes) {
            match payload.map_err(|error| PolicyError::CompilationRejected(error.to_string()))? {
                Payload::MemorySection(section) => {
                    for memory in section {
                        declared_memories.push(memory.map_err(|error| {
                            PolicyError::CompilationRejected(error.to_string())
                        })?);
                    }
                }
                Payload::TableSection(section) => {
                    for table in section {
                        declared_tables.push(
                            table
                                .map_err(|error| {
                                    PolicyError::CompilationRejected(error.to_string())
                                })?
                                .ty,
                        );
                    }
                }
                _ => {}
            }
        }
        if declared_memories.len() != 1 {
            return Err(PolicyError::MemoryInvalid(
                "exactly one defined linear memory is required".to_owned(),
            ));
        }
        if declared_tables.len() != 1 {
            return Err(PolicyError::TableInvalid(
                "exactly one defined table is required".to_owned(),
            ));
        }
        let table = declared_tables[0];
        let Some(table_maximum) = table.maximum else {
            return Err(PolicyError::TableInvalid(
                "table must declare a maximum".to_owned(),
            ));
        };
        if table.table64
            || table.shared
            || table.element_type != RefType::FUNCREF
            || table.initial > table_maximum
            || table_maximum > self.max_table_elements
        {
            return Err(PolicyError::TableInvalid(format!(
                "table must be bounded funcref with at most {} elements",
                self.max_table_elements
            )));
        }
        Ok(())
    }

    fn validate_exports(module: &Module) -> Result<(), PolicyError> {
        let exports: BTreeMap<_, _> = module
            .exports()
            .map(|export| (export.name().to_owned(), export.ty()))
            .collect();
        let expected = [
            "memory",
            "studio_alloc",
            "studio_dealloc",
            "studio_event",
            "studio_init",
        ];
        if exports.len() != expected.len()
            || expected.iter().any(|name| !exports.contains_key(*name))
        {
            return Err(PolicyError::ExportInvalid(
                "module must export exactly the five Studio ABI entries".to_owned(),
            ));
        }
        check_function(&exports, "studio_alloc", &[ValType::I32], &[ValType::I32])?;
        check_function(
            &exports,
            "studio_dealloc",
            &[ValType::I32, ValType::I32],
            &[],
        )?;
        check_function(
            &exports,
            "studio_init",
            &[ValType::I32, ValType::I32],
            &[ValType::I32],
        )?;
        check_function(
            &exports,
            "studio_event",
            &[ValType::I32, ValType::I32],
            &[ValType::I32],
        )?;
        Ok(())
    }
}

fn check_function(
    exports: &BTreeMap<String, ExternType>,
    name: &str,
    parameters: &[ValType],
    results: &[ValType],
) -> Result<(), PolicyError> {
    let Some(ExternType::Func(function)) = exports.get(name) else {
        return Err(PolicyError::ExportInvalid(format!(
            "{name} must be a function"
        )));
    };
    if !signature_matches(function, parameters, results) {
        return Err(PolicyError::ExportInvalid(format!(
            "{name} has an invalid signature"
        )));
    }
    Ok(())
}

fn signature_matches(function: &FuncType, parameters: &[ValType], results: &[ValType]) -> bool {
    let actual_parameters: Vec<_> = function.params().collect();
    let actual_results: Vec<_> = function.results().collect();
    actual_parameters.len() == parameters.len()
        && actual_results.len() == results.len()
        && actual_parameters
            .iter()
            .zip(parameters)
            .all(|(actual, expected)| same_value_type(actual, expected))
        && actual_results
            .iter()
            .zip(results)
            .all(|(actual, expected)| same_value_type(actual, expected))
}

fn same_value_type(actual: &ValType, expected: &ValType) -> bool {
    matches!(
        (actual, expected),
        (ValType::I32, ValType::I32)
            | (ValType::I64, ValType::I64)
            | (ValType::F32, ValType::F32)
            | (ValType::F64, ValType::F64)
            | (ValType::V128, ValType::V128)
    )
}

/// A compiled module proven to satisfy the Studio v1 pre-instantiation policy.
#[derive(Debug)]
pub struct ValidatedModule {
    pub(crate) module: Module,
}

impl ValidatedModule {
    /// Borrow the compiled module after policy validation.
    #[must_use]
    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub(crate) fn into_module(self) -> Module {
        self.module
    }
}

/// Stable module-policy error family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyErrorCode {
    /// Wasmtime rejected the bytes or a disabled proposal.
    CompilationRejected,
    /// The import surface was not exactly `studio_host.emit`.
    ImportInvalid,
    /// The export surface or a required signature was invalid.
    ExportInvalid,
    /// Linear-memory count, shape, or bound was invalid.
    MemoryInvalid,
    /// Table count, shape, or bound was invalid.
    TableInvalid,
}

/// Detailed module-policy failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PolicyError {
    /// Wasmtime rejected the module before policy inspection.
    #[error("module compilation rejected: {0}")]
    CompilationRejected(String),
    /// Import policy violation.
    #[error("invalid module import surface: {0}")]
    ImportInvalid(String),
    /// Export policy violation.
    #[error("invalid module export surface: {0}")]
    ExportInvalid(String),
    /// Linear-memory policy violation.
    #[error("invalid module memory: {0}")]
    MemoryInvalid(String),
    /// Table policy violation.
    #[error("invalid module table: {0}")]
    TableInvalid(String),
}

impl PolicyError {
    /// Return the stable error family for this detailed rejection.
    #[must_use]
    pub const fn code(&self) -> PolicyErrorCode {
        match self {
            Self::CompilationRejected(_) => PolicyErrorCode::CompilationRejected,
            Self::ImportInvalid(_) => PolicyErrorCode::ImportInvalid,
            Self::ExportInvalid(_) => PolicyErrorCode::ExportInvalid,
            Self::MemoryInvalid(_) => PolicyErrorCode::MemoryInvalid,
            Self::TableInvalid(_) => PolicyErrorCode::TableInvalid,
        }
    }
}
