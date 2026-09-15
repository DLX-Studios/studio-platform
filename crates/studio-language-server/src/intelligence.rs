//! Studio-owned code intelligence: catalog completion and hover backed by
//! the closed component catalog, runes, routes, and assets.
//!
//! The catalog list mirrors `docs/component-matrix.md`. The
//! `catalog_resolves` test asserts every entry against the protocol, so the
//! list cannot drift silently.

/// Closed Studio catalog component names for completion, with one-line docs
/// for the core set.
pub const CATALOG: &[(&str, &str)] = &[
    ("Box", "Container: generic layout box"),
    ("Column", "Container: vertical stack"),
    ("Row", "Container: horizontal row"),
    ("Stack", "Container: overlapping layers"),
    ("Grid", "Container: grid layout"),
    ("ScrollView", "Container: scrollable region"),
    ("ListView", "Container: scrollable list"),
    ("Spacer", "Layout: flexible space"),
    ("Divider", "Layout: rule between content"),
    ("Text", "Text: styled run"),
    ("Icon", "Media: named icon"),
    ("Image", "Media: raster image"),
    ("Card", "Container: elevated card"),
    ("Badge", "Display: status badge"),
    ("Tag", "Display: label tag"),
    ("Avatar", "Media: identity avatar"),
    ("Empty", "Display: empty state"),
    ("Skeleton", "Display: loading placeholder"),
    ("ProgressIndicator", "Feedback: linear progress"),
    ("ProgressCircle", "Feedback: circular progress"),
    ("Spinner", "Feedback: indeterminate spinner"),
    ("Button", "Control: press action"),
    ("IconButton", "Control: icon press action"),
    ("Checkbox", "Control: boolean checkbox"),
    ("Radio", "Control: exclusive option"),
    ("Switch", "Control: boolean switch"),
    ("Toggle", "Control: two-state toggle"),
    ("ButtonGroup", "Control: grouped buttons"),
    ("Slider", "Control: ranged value"),
    ("RangeSlider", "Control: ranged interval"),
    ("Select", "Control: option select"),
    ("Combobox", "Control: searchable select"),
    ("NumberInput", "Control: numeric input"),
    ("TextInput", "Control: text input"),
    ("TextArea", "Control: multi-line input"),
    ("Field", "Control: labeled field"),
    ("InputGroup", "Control: grouped inputs"),
    ("OtpInput", "Control: one-time code input"),
    ("SecretInput", "Control: host-owned secret input"),
    ("Dialog", "Overlay: modal dialog"),
    ("AlertDialog", "Overlay: destructive confirm"),
    ("Popover", "Overlay: anchored popover"),
    ("Sheet", "Overlay: edge sheet"),
    ("BottomSheet", "Overlay: bottom sheet"),
    ("Toast", "Overlay: transient toast"),
    ("Notification", "Overlay: persistent notification"),
    ("Banner", "Display: inline banner"),
    ("ContextMenu", "Overlay: context menu"),
    ("CommandPalette", "Overlay: command palette"),
    ("Tooltip", "Overlay: hover tooltip"),
    ("Scaffold", "Navigation: application shell"),
    ("AppBar", "Navigation: top app bar"),
    ("Sidebar", "Navigation: side navigation"),
    ("NavigationBar", "Navigation: bottom bar"),
    ("NavigationRail", "Navigation: side rail"),
    ("Drawer", "Navigation: slide-in drawer"),
    ("Tabs", "Navigation: tab strip"),
    ("Breadcrumb", "Navigation: breadcrumb trail"),
    ("Stepper", "Navigation: stepped flow"),
    ("Pagination", "Navigation: page control"),
    ("ListTile", "Data: list row"),
    ("SearchableList", "Data: filterable list"),
    ("VirtualList", "Data: virtualized list"),
    ("DataTable", "Data: sortable table"),
    ("Tree", "Data: hierarchical tree"),
    ("DescriptionList", "Data: term list"),
    ("Calendar", "Data: month calendar"),
    ("DatePicker", "Data: date selection"),
    ("TimePicker", "Control: time selection"),
    ("Separator", "Layout: separator"),
    ("Accordion", "Display: collapsible sections"),
    ("Collapsible", "Display: collapsible region"),
    ("HoverCard", "Overlay: hover card"),
    ("MenuBar", "Navigation: menu bar"),
    ("StatusBar", "Display: status bar"),
    ("KeyboardShortcuts", "Display: shortcut list"),
    ("Kbd", "Display: key cap"),
    ("ColorPicker", "Control: color selection"),
    ("Rating", "Control: star rating"),
    ("Resizable", "Layout: resizable region"),
    ("Dock", "Layout: docked panels"),
    ("Chart", "Display: chart (deferred render)"),
    ("Editor", "Control: code editor (deferred)"),
    ("RichText", "Text: rich content"),
    ("Carousel", "Display: carousel"),
    ("DragDrop", "Interaction: drag source/target"),
    ("Theme", "Display: theme scope"),
    ("AspectRatio", "Layout: ratio box"),
    ("Alert", "Display: alert box"),
    ("Attachment", "Data: file attachment"),
    ("Bubble", "Display: chat bubble"),
    ("Command", "Control: command row"),
    ("NativeSelect", "Control: native select"),
    ("NavigationMenu", "Display: navigation menu"),
    ("ScrollArea", "Container: scroll area"),
    ("Item", "Data: generic item"),
    ("Message", "Display: message row"),
    ("MessageScroller", "Display: message scroller"),
    ("ToggleGroup", "Control: exclusive toggles"),
    ("Sonner", "Overlay: sonner toasts"),
];

/// Runes admitted by the portable subset, with documentation.
pub const RUNES: &[(&str, &str)] = &[
    ("$props", "Declare typed component props with defaults"),
    ("$state", "Declare reactive local state"),
    ("$derived", "Declare values derived from other bindings"),
];

/// Convert PascalCase catalog names to protocol snake_case like the lowerer.
fn catalog_kind_name(name: &str) -> String {
    let mut converted = String::with_capacity(name.len() + 4);
    for (index, character) in name.chars().enumerate() {
        if character.is_ascii_uppercase() && index != 0 {
            converted.push('_');
        }
        converted.push(character.to_ascii_lowercase());
    }
    converted
}

/// Whether a PascalCase name resolves to a closed catalog kind.
#[must_use]
pub fn catalog_kind_exists(name: &str) -> bool {
    let quoted = format!("\"{}\"", catalog_kind_name(name));
    serde_json::from_str::<studio_protocol::NodeKind>(&quoted).is_ok()
}

/// Completion labels for component tags.
#[must_use]
pub fn component_completions() -> Vec<(String, String)> {
    CATALOG
        .iter()
        .map(|(name, documentation)| ((*name).to_owned(), (*documentation).to_owned()))
        .collect()
}

/// Hover documentation for one catalog component, if known.
#[must_use]
pub fn component_documentation(name: &str) -> Option<String> {
    CATALOG
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(name, documentation)| format!("**{name}** — {documentation}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_list_has_one_hundred_entries() {
        assert_eq!(CATALOG.len(), 100);
    }

    #[test]
    fn every_catalog_entry_resolves() {
        for (name, _) in CATALOG {
            assert!(
                catalog_kind_exists(name),
                "catalog entry {name} must resolve through the protocol"
            );
        }
    }
}
