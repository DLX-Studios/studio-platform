use std::collections::{BTreeMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{PROTOCOL_VERSION, ProtocolError, ProtocolLimits, validate_bounded_string};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MountTree {
    pub protocol_version: u16,
    pub route: String,
    pub root: UiNode,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UiNode {
    pub id: String,
    pub kind: NodeKind,
    #[serde(default)]
    pub props: BTreeMap<String, Value>,
    #[serde(default)]
    pub children: Vec<Self>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Box,
    Column,
    Row,
    Stack,
    Grid,
    ScrollView,
    ListView,
    Spacer,
    Divider,
    Text,
    Icon,
    Image,
    Card,
    Badge,
    Tag,
    Avatar,
    Empty,
    Skeleton,
    ProgressIndicator,
    ProgressCircle,
    Spinner,
    Button,
    IconButton,
    Checkbox,
    Radio,
    Switch,
    Toggle,
    ButtonGroup,
    Slider,
    RangeSlider,
    Select,
    Combobox,
    NumberInput,
    TextInput,
    TextArea,
    Field,
    InputGroup,
    OtpInput,
    SecretInput,
    Dialog,
    AlertDialog,
    Popover,
    Sheet,
    BottomSheet,
    Toast,
    Notification,
    Banner,
    ContextMenu,
    CommandPalette,
    Tooltip,
    Scaffold,
    AppBar,
    Sidebar,
    NavigationBar,
    NavigationRail,
    Drawer,
    Tabs,
    Breadcrumb,
    Stepper,
    Pagination,
    ListTile,
    SearchableList,
    VirtualList,
    DataTable,
    Tree,
    DescriptionList,
    Calendar,
    DatePicker,
    TimePicker,
    Separator,
    Accordion,
    Collapsible,
    HoverCard,
    MenuBar,
    StatusBar,
    KeyboardShortcuts,
    Kbd,
    ColorPicker,
    Rating,
    Resizable,
    Dock,
    Chart,
    Editor,
    RichText,
    Carousel,
    DragDrop,
    Theme,
    AspectRatio,
    Alert,
    Attachment,
    Bubble,
    Command,
    NativeSelect,
    NavigationMenu,
    ScrollArea,
    Item,
    Message,
    MessageScroller,
    ToggleGroup,
    Sonner,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PatchBatch {
    pub sequence: u64,
    pub operations: Vec<PatchOp>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum PatchOp {
    UpdateProp {
        node_id: String,
        property: String,
        value: Value,
    },
    InsertChild {
        parent_id: String,
        index: u32,
        node: UiNode,
    },
    RemoveNode {
        node_id: String,
    },
    ReplaceNode {
        node_id: String,
        node: UiNode,
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UiEvent {
    pub node_id: String,
    pub event: String,
    pub payload: Value,
}

pub(crate) fn validate_mount(
    mount: &MountTree,
    limits: ProtocolLimits,
) -> Result<(), ProtocolError> {
    if mount.protocol_version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(mount.protocol_version));
    }
    crate::navigation::validate_route(&mount.route)?;
    validate_tree(&mount.root, limits)
}

pub(crate) fn validate_patch(
    batch: &PatchBatch,
    limits: ProtocolLimits,
) -> Result<(), ProtocolError> {
    if batch.operations.is_empty() || batch.operations.len() > limits.max_patch_operations {
        return Err(ProtocolError::InvalidPatchOperationCount(
            limits.max_patch_operations,
        ));
    }
    crate::validate_patch_sequence(batch, None)?;
    for operation in &batch.operations {
        match operation {
            PatchOp::UpdateProp {
                node_id,
                property,
                value,
            } => {
                validate_node_id(node_id, limits)?;
                if property.is_empty() {
                    return Err(ProtocolError::InvalidPatchOperationCount(
                        limits.max_patch_operations,
                    ));
                }
                validate_value_strings(value, limits.max_string_bytes)?;
            }
            PatchOp::InsertChild {
                parent_id, node, ..
            } => {
                validate_node_id(parent_id, limits)?;
                validate_tree(node, limits)?;
            }
            PatchOp::RemoveNode { node_id } => validate_node_id(node_id, limits)?,
            PatchOp::ReplaceNode { node_id, node } => {
                validate_node_id(node_id, limits)?;
                validate_tree(node, limits)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_ui_event(
    event: &UiEvent,
    limits: ProtocolLimits,
) -> Result<(), ProtocolError> {
    validate_node_id(&event.node_id, limits)?;
    if event.event.is_empty() {
        return Err(ProtocolError::InvalidLifecycle("empty UI event name"));
    }
    validate_value_strings(&event.payload, limits.max_string_bytes)
}

fn validate_tree(root: &UiNode, limits: ProtocolLimits) -> Result<(), ProtocolError> {
    let mut ids = HashSet::new();
    let mut stack = vec![(root, 1_usize)];
    let mut node_count = 0_usize;
    while let Some((node, depth)) = stack.pop() {
        if depth > limits.max_tree_depth {
            return Err(ProtocolError::TreeTooDeep(limits.max_tree_depth));
        }
        node_count += 1;
        if node_count > limits.max_nodes {
            return Err(ProtocolError::TooManyNodes(limits.max_nodes));
        }
        validate_node_id(&node.id, limits)?;
        if !ids.insert(node.id.as_str()) {
            return Err(ProtocolError::DuplicateNodeId(node.id.clone()));
        }
        crate::validate_node_contract(node, limits)?;
        stack.extend(node.children.iter().map(|child| (child, depth + 1)));
    }
    Ok(())
}

fn validate_node_id(id: &str, limits: ProtocolLimits) -> Result<(), ProtocolError> {
    if id.is_empty() || id.len() > limits.max_node_id_bytes {
        return Err(ProtocolError::InvalidNodeId(id.to_owned()));
    }
    Ok(())
}

fn validate_value_strings(value: &Value, limit: usize) -> Result<(), ProtocolError> {
    match value {
        Value::String(value) => validate_bounded_string(value, limit),
        Value::Array(values) => values
            .iter()
            .try_for_each(|value| validate_value_strings(value, limit)),
        Value::Object(values) => values
            .values()
            .try_for_each(|value| validate_value_strings(value, limit)),
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
    }
}
