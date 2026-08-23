//! Studio-owned native component wrappers and retained-node mappings.
//! Studio-owned protocol-to-native component and event boundaries.

mod catalog;
mod controls;
mod events;
mod secret_input;
mod state;
mod transition;
mod update;

pub use catalog::{
    CatalogError, CatalogErrorCode, ComponentCatalog, NativeComponent, NativeLayer, TargetSize,
};
pub use controls::RuntimeControl;
pub use events::{DispatchError, DispatchErrorCode, HostEventDispatcher, InputAction};
pub use gpui_component::animation;
pub use secret_input::{
    HostSecretInput, SecretInputError, SecretInputErrorCode, SecretInputSnapshot,
};
pub use state::{NativeStateSnapshot, NativeStateStore};
pub use transition::PropertyTransition;
pub use update::{UpdateError, UpdateErrorCode, UpdateReport};
