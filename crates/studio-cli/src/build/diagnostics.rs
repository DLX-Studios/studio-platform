//! Closed build-phase diagnostics and the documented exit-code families.
//!
//! The contract lives in `specs/003-studio-toolchain/contracts/cli.md`: every
//! failure reports one JSON diagnostic per problem on stderr and maps to
//! exactly one process exit-code family.

use serde::Serialize;

/// Success exit code.
pub const EXIT_SUCCESS: i32 = 0;
/// Cancelled watch session (Ctrl-C observed by the shell as SIGINT).
pub const EXIT_CANCELLED: i32 = 130;

/// Named build stage that owns a diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildPhase {
    /// Command-line usage problem.
    Usage,
    /// Project layout and input validation.
    Validate,
    /// lucide collection, route generation, and asc compilation.
    Compile,
    /// Bundle assembly, signing, and atomic output.
    Package,
    /// Filesystem or toolchain environment problems.
    Io,
    /// Watch session problems (runtime launch, lost runtime).
    Watch,
}

impl BuildPhase {
    /// The documented exit code for this phase's failure family.
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Usage => 2,
            Self::Validate => 10,
            Self::Compile => 11,
            Self::Package => 12,
            Self::Io => 13,
            Self::Watch => 14,
        }
    }
}

/// One safe, machine-readable build problem.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BuildDiagnostic {
    /// Phase that owns the problem.
    pub phase: BuildPhase,
    /// Stable, namespaced code (packaging reuses studio-package families).
    pub code: String,
    /// `error` or `warning`.
    pub severity: &'static str,
    /// Safe, actionable message; never contains secret material.
    pub message: String,
    /// Source line when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    /// Source column when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
}

impl BuildDiagnostic {
    /// Construct an error-severity diagnostic without a source location.
    #[must_use]
    pub fn error(phase: BuildPhase, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            phase,
            code: code.into(),
            severity: "error",
            message: message.into(),
            line: None,
            column: None,
        }
    }

    /// Construct a warning-severity diagnostic without a source location.
    #[must_use]
    pub fn warning(phase: BuildPhase, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            phase,
            code: code.into(),
            severity: "warning",
            message: message.into(),
            line: None,
            column: None,
        }
    }

    /// Attach a known source location.
    #[must_use]
    pub fn at(mut self, line: u64, column: u64) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }

    /// Emit this diagnostic as one JSON object on stderr.
    pub fn report(&self) {
        if let Ok(line) = serde_json::to_string(self) {
            eprintln!("{line}");
        }
    }
}

/// A failed build: owning phase plus the collected diagnostics.
#[derive(Clone, Debug)]
pub struct BuildFailure {
    /// Phase that failed.
    pub phase: BuildPhase,
    /// Every problem observed before the pipeline stopped.
    pub diagnostics: Vec<BuildDiagnostic>,
}

impl BuildFailure {
    #[must_use]
    pub fn new(phase: BuildPhase, diagnostics: Vec<BuildDiagnostic>) -> Self {
        Self { phase, diagnostics }
    }

    /// Report every diagnostic on stderr.
    pub fn report(&self) {
        for diagnostic in &self.diagnostics {
            diagnostic.report();
        }
    }

    /// The documented exit code for this failure.
    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        self.phase.exit_code()
    }
}

/// Convert a studio-package error-code variant name into a stable
/// `FAMILY_VARIANT` diagnostic code (for example `MANIFEST_INVALID_JSON`).
#[must_use]
pub fn package_code(family: &str, variant: &str) -> String {
    let mut code = String::from(family);
    code.push('_');
    let mut uppercase_next = true;
    for character in variant.chars() {
        if character.is_uppercase() {
            if !uppercase_next {
                code.push('_');
            }
            code.extend(character.to_lowercase());
            uppercase_next = true;
        } else {
            code.push(character);
            uppercase_next = false;
        }
    }
    code
}

/// Diagnose `asc` output text into at most one structured diagnostic per
/// recognized `path:line:column` header, or one summary diagnostic.
pub fn asc_diagnostics(output: &str) -> Vec<BuildDiagnostic> {
    let mut diagnostics = Vec::new();
    let pattern = regex::Regex::new(r"([A-Za-z0-9_./-]+):(\d+):(\d+):").unwrap();
    let mut seen = 0usize;
    for line in output.lines() {
        if let Some(captures) = pattern.captures(line) {
            let line_number = captures[2].parse::<u64>().ok();
            let column_number = captures[3].parse::<u64>().ok();
            let mut diagnostic = BuildDiagnostic::error(
                BuildPhase::Compile,
                "BUILD_COMPILE_ASC",
                line.trim().to_owned(),
            );
            if let (Some(line_number), Some(column_number)) = (line_number, column_number) {
                diagnostic = diagnostic.at(line_number, column_number);
            }
            diagnostics.push(diagnostic);
            seen += 1;
            if seen >= 8 {
                break;
            }
        }
    }
    if diagnostics.is_empty() {
        let tail: String = output
            .lines()
            .filter(|line| !line.trim().is_empty())
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        diagnostics.push(BuildDiagnostic::error(
            BuildPhase::Compile,
            "BUILD_COMPILE_ASC",
            if tail.is_empty() {
                "asc failed without output".to_owned()
            } else {
                tail
            },
        ));
    }
    diagnostics
}
