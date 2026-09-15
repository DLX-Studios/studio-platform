//! Deterministic WAT and WASM fixture construction for sandbox boundary tests.

use std::fmt::Write as _;

use thiserror::Error;

/// A WebAssembly numeric value type supported by the fixture builder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueType {
    /// A 32-bit integer.
    I32,
    /// A 64-bit integer.
    I64,
}

impl ValueType {
    const fn wat(self) -> &'static str {
        match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
        }
    }
}

/// A function's parameter and result types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionSignature {
    parameters: Vec<ValueType>,
    results: Vec<ValueType>,
}

impl FunctionSignature {
    /// Create a signature from ordered parameter and result types.
    pub fn new(
        parameters: impl IntoIterator<Item = ValueType>,
        results: impl IntoIterator<Item = ValueType>,
    ) -> Self {
        Self {
            parameters: parameters.into_iter().collect(),
            results: results.into_iter().collect(),
        }
    }

    fn write_wat(&self, output: &mut String) {
        if !self.parameters.is_empty() {
            output.push_str(" (param");
            for parameter in &self.parameters {
                write!(output, " {}", parameter.wat()).expect("writing to String cannot fail");
            }
            output.push(')');
        }
        if !self.results.is_empty() {
            output.push_str(" (result");
            for result in &self.results {
                write!(output, " {}", result.wat()).expect("writing to String cannot fail");
            }
            output.push(')');
        }
    }
}

/// A function body selected to exercise runtime and ABI behavior.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FunctionBody {
    /// Return one constant `i32`.
    I32Const(i32),
    /// Execute a branch-backed loop until interrupted.
    InfiniteLoop,
    /// Trap immediately with `unreachable`.
    Trap,
    /// Call a declared import with intentionally caller-selected raw arguments.
    CallImport {
        /// Zero-based import declaration index.
        import_index: usize,
        /// Raw `i32` arguments, including invalid pointer/length values.
        i32_arguments: Vec<i32>,
    },
}

/// A locally defined function and optional export name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionSpec {
    export_name: Option<String>,
    signature: FunctionSignature,
    body: FunctionBody,
}

impl FunctionSpec {
    /// Create a function exported under `name`.
    pub fn exported(
        name: impl Into<String>,
        signature: FunctionSignature,
        body: FunctionBody,
    ) -> Self {
        Self {
            export_name: Some(name.into()),
            signature,
            body,
        }
    }

    /// Create a non-exported helper function.
    #[must_use]
    pub fn private(signature: FunctionSignature, body: FunctionBody) -> Self {
        Self {
            export_name: None,
            signature,
            body,
        }
    }
}

/// A bounded or deliberately unbounded linear-memory declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemorySpec {
    minimum_pages: u32,
    maximum_pages: Option<u32>,
    export_name: Option<String>,
}

impl MemorySpec {
    /// Create a memory with a minimum number of 64 KiB pages.
    #[must_use]
    pub const fn new(minimum_pages: u32) -> Self {
        Self {
            minimum_pages,
            maximum_pages: None,
            export_name: None,
        }
    }

    /// Declare a maximum page count.
    #[must_use]
    pub const fn maximum(mut self, maximum_pages: u32) -> Self {
        self.maximum_pages = Some(maximum_pages);
        self
    }

    /// Export the memory under a wire name.
    #[must_use]
    pub fn export_as(mut self, name: impl Into<String>) -> Self {
        self.export_name = Some(name.into());
        self
    }
}

/// A bounded or deliberately unbounded `funcref` table declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableSpec {
    minimum_elements: u32,
    maximum_elements: Option<u32>,
    export_name: Option<String>,
}

impl TableSpec {
    /// Create a table with a minimum element count.
    #[must_use]
    pub const fn new(minimum_elements: u32) -> Self {
        Self {
            minimum_elements,
            maximum_elements: None,
            export_name: None,
        }
    }

    /// Declare a maximum element count.
    #[must_use]
    pub const fn maximum(mut self, maximum_elements: u32) -> Self {
        self.maximum_elements = Some(maximum_elements);
        self
    }

    /// Export the table under a wire name.
    #[must_use]
    pub fn export_as(mut self, name: impl Into<String>) -> Self {
        self.export_name = Some(name.into());
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImportSpec {
    module: String,
    name: String,
    signature: FunctionSignature,
}

/// A deterministic module builder for valid and hostile fixtures.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WasmFixtureBuilder {
    imports: Vec<ImportSpec>,
    memories: Vec<MemorySpec>,
    tables: Vec<TableSpec>,
    functions: Vec<FunctionSpec>,
}

impl WasmFixtureBuilder {
    /// Create an empty module builder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            imports: Vec::new(),
            memories: Vec::new(),
            tables: Vec::new(),
            functions: Vec::new(),
        }
    }

    /// Add a function import in declaration order.
    #[must_use]
    pub fn import_function(
        mut self,
        module: impl Into<String>,
        name: impl Into<String>,
        signature: FunctionSignature,
    ) -> Self {
        self.imports.push(ImportSpec {
            module: module.into(),
            name: name.into(),
            signature,
        });
        self
    }

    /// Add a linear memory declaration.
    #[must_use]
    pub fn memory(mut self, memory: MemorySpec) -> Self {
        self.memories.push(memory);
        self
    }

    /// Add a `funcref` table declaration.
    #[must_use]
    pub fn table(mut self, table: TableSpec) -> Self {
        self.tables.push(table);
        self
    }

    /// Add a local function declaration.
    #[must_use]
    pub fn function(mut self, function: FunctionSpec) -> Self {
        self.functions.push(function);
        self
    }

    /// Encode the complete module as deterministic WAT and WASM.
    ///
    /// # Errors
    ///
    /// Returns [`FixtureError::Encode`] when declarations are inconsistent or WAT encoding fails.
    pub fn build(self) -> Result<WasmFixture, FixtureError> {
        self.validate()?;
        let wat = self.render_wat();
        let wasm = wat::parse_str(&wat).map_err(|error| FixtureError::Encode(error.to_string()))?;
        Ok(WasmFixture { wat, wasm })
    }

    fn validate(&self) -> Result<(), FixtureError> {
        for function in &self.functions {
            match &function.body {
                FunctionBody::I32Const(_) if function.signature.results != [ValueType::I32] => {
                    return Err(FixtureError::Encode(
                        "i32.const body requires one i32 result".to_owned(),
                    ));
                }
                FunctionBody::CallImport {
                    import_index,
                    i32_arguments,
                } => {
                    let Some(import) = self.imports.get(*import_index) else {
                        return Err(FixtureError::Encode("unknown import index".to_owned()));
                    };
                    if import.signature.parameters.len() != i32_arguments.len()
                        || import
                            .signature
                            .parameters
                            .iter()
                            .any(|value| *value != ValueType::I32)
                        || import.signature.results != function.signature.results
                    {
                        return Err(FixtureError::Encode(
                            "call body does not match imported signature".to_owned(),
                        ));
                    }
                }
                FunctionBody::I32Const(_) | FunctionBody::InfiniteLoop | FunctionBody::Trap => {}
            }
        }
        Ok(())
    }

    fn render_wat(&self) -> String {
        let mut output = String::from("(module\n");
        for (index, import) in self.imports.iter().enumerate() {
            write!(
                output,
                "  (import {:?} {:?} (func $import_{index}",
                import.module, import.name
            )
            .expect("writing to String cannot fail");
            import.signature.write_wat(&mut output);
            output.push_str("))\n");
        }
        for (index, memory) in self.memories.iter().enumerate() {
            let identifier = if index == 0 {
                "$memory".to_owned()
            } else {
                format!("$memory_{index}")
            };
            write!(output, "  (memory {identifier} {}", memory.minimum_pages)
                .expect("writing to String cannot fail");
            if let Some(maximum) = memory.maximum_pages {
                write!(output, " {maximum}").expect("writing to String cannot fail");
            }
            output.push_str(")\n");
            if let Some(export_name) = &memory.export_name {
                writeln!(output, "  (export {export_name:?} (memory {identifier}))")
                    .expect("writing to String cannot fail");
            }
        }
        for (index, table) in self.tables.iter().enumerate() {
            let identifier = if index == 0 {
                "$table".to_owned()
            } else {
                format!("$table_{index}")
            };
            write!(output, "  (table {identifier} {}", table.minimum_elements)
                .expect("writing to String cannot fail");
            if let Some(maximum) = table.maximum_elements {
                write!(output, " {maximum}").expect("writing to String cannot fail");
            }
            output.push_str(" funcref)\n");
            if let Some(export_name) = &table.export_name {
                writeln!(output, "  (export {export_name:?} (table {identifier}))")
                    .expect("writing to String cannot fail");
            }
        }
        for (index, function) in self.functions.iter().enumerate() {
            let identifier = function_identifier(index, function.export_name.as_deref());
            write!(output, "  (func {identifier}").expect("writing to String cannot fail");
            function.signature.write_wat(&mut output);
            output.push('\n');
            match &function.body {
                FunctionBody::I32Const(value) => {
                    writeln!(output, "    i32.const {value}")
                        .expect("writing to String cannot fail");
                }
                FunctionBody::InfiniteLoop => {
                    output.push_str("    (loop $spin (br $spin))\n    unreachable\n");
                }
                FunctionBody::Trap => output.push_str("    unreachable\n"),
                FunctionBody::CallImport {
                    import_index,
                    i32_arguments,
                } => {
                    for argument in i32_arguments {
                        writeln!(output, "    (i32.const {argument})")
                            .expect("writing to String cannot fail");
                    }
                    writeln!(output, "    call $import_{import_index}")
                        .expect("writing to String cannot fail");
                }
            }
            output.push_str("  )\n");
            if let Some(export_name) = &function.export_name {
                writeln!(output, "  (export {export_name:?} (func {identifier}))")
                    .expect("writing to String cannot fail");
            }
        }
        output.push_str(")\n");
        output
    }
}

fn function_identifier(index: usize, export_name: Option<&str>) -> String {
    let Some(export_name) = export_name else {
        return format!("$func_{index}");
    };
    let sanitized: String = export_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        format!("$func_{index}")
    } else {
        format!("${sanitized}")
    }
}

/// A generated text/binary fixture pair.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WasmFixture {
    wat: String,
    wasm: Vec<u8>,
}

impl WasmFixture {
    /// Return the deterministic WebAssembly text representation.
    #[must_use]
    pub fn wat(&self) -> &str {
        &self.wat
    }

    /// Return the encoded WebAssembly module bytes.
    #[must_use]
    pub fn wasm(&self) -> &[u8] {
        &self.wasm
    }
}

/// Fixture construction failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum FixtureError {
    /// Declarations could not be encoded into a valid fixture.
    #[error("failed to encode WAT fixture: {0}")]
    Encode(String),
}
