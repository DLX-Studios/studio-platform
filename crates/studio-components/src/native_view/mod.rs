//! Shared host-owned native rendering for Runtime and Designer projections.

mod renderer;
mod selection;
use crate::{InputAction, RuntimeControl};
pub use renderer::{
    AnimationPolicy, FoundationFeature, FoundationGalleryModel, FoundationSnapshot,
    NativeRuntimeView,
};
use std::collections::BTreeMap;
use studio_protocol::{NodeKind, UiNode};

/// Immutable retained-node copy consumed by a native render pass.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeRenderNode {
    /// Stable source identity.
    pub id: String,
    /// Closed protocol component kind.
    pub kind: NodeKind,
    /// Semantic control mapping.
    pub control: Option<RuntimeControl>,
    /// Resolved protocol properties.
    pub props: BTreeMap<String, serde_json::Value>,
    /// Ordered children.
    pub children: Vec<Self>,
}
impl From<&UiNode> for NativeRenderNode {
    fn from(node: &UiNode) -> Self {
        Self {
            id: node.id.clone(),
            kind: node.kind,
            control: RuntimeControl::from_node_kind(node.kind),
            props: node.props.clone(),
            children: node.children.iter().map(Self::from).collect(),
        }
    }
}

/// Host implementation of a mounted native surface. Never exposed to a guest.
pub trait NativeSurface: 'static {
    /// Host-specific dispatch failure.
    type Error;
    /// Current immutable tree.
    fn render_tree(&self) -> Option<NativeRenderNode>;
    /// Deliver one semantic native action.
    ///
    /// # Errors
    /// Returns a host rejection when the action cannot be admitted or processed.
    fn process_input(&mut self, node_id: &str, action: InputAction) -> Result<(), Self::Error>;
    /// Drain completed host actions.
    ///
    /// # Errors
    /// Returns a host error if a completed action cannot be applied.
    fn poll_action_results(&mut self) -> Result<bool, Self::Error>;
    /// Read an admitted asset by its exact package key.
    fn asset(&self, path: &str) -> Option<&[u8]>;
    /// Persistent host warning, if needed.
    fn warning(&self) -> Option<&str>;
}

/// Native events observed by an editor host without exposing GPUI to application code.
#[derive(Clone, Debug)]
pub enum NativeViewEvent {
    /// A semantic input delivered to the mounted surface.
    Input {
        /// Stable node receiving input.
        node_id: String,
        /// Closed native action.
        action: InputAction,
    },
    /// Authoring selection, intercepted before native control activation.
    Selected(String),
}

/// An immutable projection and admitted asset bytes for an editor host.
#[derive(Default)]
pub struct ProjectionSurface {
    /// Current projected tree. Missing projection renders an empty canvas.
    pub root: Option<NativeRenderNode>,
    /// Host-admitted assets; this adapter never reads arbitrary filesystem paths.
    pub assets: BTreeMap<String, std::sync::Arc<[u8]>>,
}
impl NativeSurface for ProjectionSurface {
    type Error = std::convert::Infallible;
    fn render_tree(&self) -> Option<NativeRenderNode> {
        self.root.clone()
    }
    fn process_input(&mut self, _: &str, _: InputAction) -> Result<(), Self::Error> {
        Ok(())
    }
    fn poll_action_results(&mut self) -> Result<bool, Self::Error> {
        Ok(false)
    }
    fn asset(&self, path: &str) -> Option<&[u8]> {
        self.assets.get(path).map(AsRef::as_ref)
    }
    fn warning(&self) -> Option<&str> {
        None
    }
}
