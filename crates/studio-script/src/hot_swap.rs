//! Transactional, identity-preserving module replacement.
//!
//! `replace_module` is a pure migration planner: given the live module and a
//! validated replacement, it reports per-slot migration (preserved,
//! initialized, disposed) or rejects without touching anything. The caller
//! applies the accepted replacement and rolls back by keeping the old module,
//! so swaps are atomic by construction.

use std::collections::{BTreeMap, BTreeSet};

use crate::ir::{ComponentDefinition, StudioModule};
use crate::types::compatible;

/// One slot migration decision inside an accepted swap.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotMigration {
    /// Slot name.
    pub name: String,
    /// What happens to the slot.
    pub outcome: SlotOutcome,
}

/// Per-slot migration outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotOutcome {
    /// Stable ID and type match: the live value survives.
    Preserved,
    /// New slot, or a genuine type change: initialize from the replacement.
    Initialized,
    /// Removed slot: dispose safely.
    Disposed,
}

/// Component-level migration inside an accepted swap.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentMigration {
    /// Component identity.
    pub id: String,
    /// Slot migrations in declaration order.
    pub slots: Vec<SlotMigration>,
    /// Whether the component itself is new (all slots initialize).
    pub added: bool,
    /// Whether the component was removed (all slots dispose).
    pub removed: bool,
}

/// Outcome of planning one module replacement.
#[derive(Clone, Debug, PartialEq)]
pub enum SwapOutcome {
    /// The replacement applies; the report describes every migration.
    Accepted {
        /// Per-component migration in module order.
        components: Vec<ComponentMigration>,
    },
    /// The replacement is structurally incompatible; the caller keeps the
    /// live module running.
    Rejected {
        /// Stable reason code.
        code: &'static str,
        /// Safe explanation.
        message: String,
    },
}

/// Plan the replacement of `old` by `new` without mutating either.
///
/// Components match by stable identity. State slots preserve only when stable
/// ID and `StudioType` both match (there is one numeric type, so numeric edits
/// never break compatibility); genuinely retyped slots dispose and initialize
/// from the replacement. Derived slots always re-initialize from the new
/// module. Removal disposes; addition initializes.
#[must_use]
pub fn replace_module(old: &StudioModule, new: &StudioModule) -> SwapOutcome {
    if old.version != new.version {
        return SwapOutcome::Rejected {
            code: "SWAP_VERSION_MISMATCH",
            message: format!(
                "cannot swap IR version {} with version {}",
                old.version, new.version
            ),
        };
    }
    let old_components: BTreeMap<&str, &ComponentDefinition> = old
        .components
        .iter()
        .map(|component| (component.id.as_str(), component))
        .collect();
    let mut seen = BTreeSet::new();
    let mut components = Vec::new();
    for component in &new.components {
        seen.insert(component.id.as_str());
        match old_components.get(component.id.as_str()) {
            None => components.push(ComponentMigration {
                id: component.id.clone(),
                slots: component
                    .state
                    .iter()
                    .map(|slot| SlotMigration {
                        name: slot.name.clone(),
                        outcome: SlotOutcome::Initialized,
                    })
                    .collect(),
                added: true,
                removed: false,
            }),
            Some(previous) => components.push(ComponentMigration {
                id: component.id.clone(),
                slots: migrate_slots(previous, component),
                added: false,
                removed: false,
            }),
        }
    }
    for component in &old.components {
        if !seen.contains(component.id.as_str()) {
            components.push(ComponentMigration {
                id: component.id.clone(),
                slots: component
                    .state
                    .iter()
                    .map(|slot| SlotMigration {
                        name: slot.name.clone(),
                        outcome: SlotOutcome::Disposed,
                    })
                    .collect(),
                added: false,
                removed: true,
            });
        }
    }
    SwapOutcome::Accepted { components }
}

fn migrate_slots(previous: &ComponentDefinition, next: &ComponentDefinition) -> Vec<SlotMigration> {
    let old_slots: BTreeMap<&str, &crate::ir::StateSlot> = previous
        .state
        .iter()
        .map(|slot| (slot.name.as_str(), slot))
        .collect();
    let mut migrations = Vec::new();
    let mut seen = BTreeSet::new();
    for slot in &next.state {
        seen.insert(slot.name.as_str());
        let outcome = match old_slots.get(slot.name.as_str()) {
            Some(old) if old.id == slot.id && compatible(&old.ty, &slot.ty) => {
                SlotOutcome::Preserved
            }
            _ => SlotOutcome::Initialized,
        };
        migrations.push(SlotMigration {
            name: slot.name.clone(),
            outcome,
        });
    }
    for slot in &previous.state {
        if !seen.contains(slot.name.as_str()) {
            migrations.push(SlotMigration {
                name: slot.name.clone(),
                outcome: SlotOutcome::Disposed,
            });
        }
    }
    migrations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::StateSlot;
    use crate::ir::StudioExpression;
    use crate::types::{NumberLiteral, StudioType, StudioValue};

    fn slot(id: &str, name: &str, ty: StudioType) -> StateSlot {
        StateSlot {
            id: id.to_owned(),
            name: name.to_owned(),
            ty,
            initial: StudioExpression::Literal(StudioValue::Number(NumberLiteral {
                text: "0".to_owned(),
                value: 0.0,
            })),
            span: crate::Span {
                start: crate::Location {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
                end: crate::Location {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
            },
        }
    }

    fn module_with(id: &str, slots: Vec<StateSlot>) -> StudioModule {
        StudioModule {
            version: crate::ir::STUDIO_IR_VERSION,
            id: id.to_owned(),
            imports: Vec::new(),
            exports: vec![id.to_owned()],
            components: vec![ComponentDefinition {
                id: id.to_owned(),
                name: id.to_owned(),
                props: Vec::new(),
                state: slots,
                derived: Vec::new(),
                handlers: Vec::new(),
                content: Vec::new(),
                template: Vec::new(),
                span: crate::Span {
                    start: crate::Location {
                        line: 1,
                        column: 1,
                        offset: 0,
                    },
                    end: crate::Location {
                        line: 1,
                        column: 1,
                        offset: 0,
                    },
                },
            }],
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn compatible_slots_preserve_and_type_changes_reset() {
        let old = module_with(
            "swap",
            vec![
                slot("swap#quantity", "quantity", StudioType::Number),
                slot("swap#name", "name", StudioType::String),
            ],
        );
        // Numeric edits keep the type: preserved. String-to-number is a
        // genuine change: disposes and initializes.
        let new = module_with(
            "swap",
            vec![
                slot("swap#quantity", "quantity", StudioType::Number),
                slot("swap#name", "name", StudioType::Number),
                slot("swap#extra", "extra", StudioType::Boolean),
            ],
        );
        let SwapOutcome::Accepted { components } = replace_module(&old, &new) else {
            panic!("swap must be accepted");
        };
        let outcomes: Vec<(&str, SlotOutcome)> = components[0]
            .slots
            .iter()
            .map(|migration| (migration.name.as_str(), migration.outcome))
            .collect();
        assert_eq!(
            outcomes,
            vec![
                ("quantity", SlotOutcome::Preserved),
                ("name", SlotOutcome::Initialized),
                ("extra", SlotOutcome::Initialized),
            ]
        );
    }

    #[test]
    fn removed_components_dispose_and_version_skew_rejects() {
        let old = module_with("swap", vec![slot("swap#gone", "gone", StudioType::Boolean)]);
        let new = module_with("replacement", vec![]);
        let SwapOutcome::Accepted { components } = replace_module(&old, &new) else {
            panic!("swap must be accepted");
        };
        let removed = components
            .iter()
            .find(|migration| migration.removed)
            .expect("removed component is reported");
        assert_eq!(removed.id, "swap");
        assert_eq!(removed.slots[0].outcome, SlotOutcome::Disposed);
        assert!(
            components
                .iter()
                .any(|migration| migration.added && migration.id == "replacement")
        );

        let mut skewed = new.clone();
        skewed.version += 1;
        assert!(matches!(
            replace_module(&old, &skewed),
            SwapOutcome::Rejected { .. }
        ));
    }
}
