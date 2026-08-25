# Studio Component Renderer Matrix

This matrix tracks the renderer contract per closed protocol kind. `Native mapped` means the
catalog resolves to a Studio-owned native layer. `Semantically rendered` means the GPUI renderer
honors the declared properties for that kind. `Verified` means automated fixtures cover the
renderer/state contract. A mapped kind is not automatically considered rendered.

| Kind | Family | Protocol declared | Native mapped | Semantically rendered | Verified |
| --- | --- | --- | --- | --- | --- |
| Box | Container | yes | yes | yes | yes |
| Column | Container | yes | yes | yes | yes |
| Row | Container | yes | yes | yes | yes |
| Stack | Container | yes | yes | yes | yes |
| Grid | Container | yes | yes | yes | yes |
| ScrollView | Container | yes | yes | yes | yes |
| ListView | Container | yes | yes | yes | yes |
| Spacer | Container | yes | yes | yes | yes |
| Divider | Container | yes | yes | yes | yes |
| Text | Text | yes | yes | yes | yes |
| Icon | Media | yes | yes | yes | yes |
| Image | Media | yes | yes | yes | yes |
| Card | Container | yes | yes | yes | yes |
| Badge | Text/display | yes | yes | yes | yes |
| Tag | Text/display | yes | yes | yes | yes |
| Avatar | Media | yes | yes | yes | yes |
| Empty | Display | yes | yes | yes | yes |
| Skeleton | Display | yes | yes | yes | yes |
| ProgressIndicator | Feedback | yes | yes | no | no |
| ProgressCircle | Feedback | yes | yes | no | no |
| Spinner | Feedback | yes | yes | no | no |
| Button | Form/input | yes | yes | no | no |
| IconButton | Form/input | yes | yes | no | no |
| Checkbox | Form/input | yes | yes | no | no |
| Radio | Form/input | yes | yes | no | no |
| Switch | Form/input | yes | yes | no | no |
| Toggle | Form/input | yes | yes | no | no |
| ButtonGroup | Form/input | yes | yes | no | no |
| Slider | Form/input | yes | yes | no | no |
| RangeSlider | Form/input | yes | yes | no | no |
| Select | Form/input | yes | yes | no | no |
| Combobox | Form/input | yes | yes | no | no |
| NumberInput | Form/input | yes | yes | no | no |
| TextInput | Form/input | yes | yes | no | no |
| TextArea | Form/input | yes | yes | no | no |
| Field | Form/input | yes | yes | no | no |
| InputGroup | Form/input | yes | yes | no | no |
| OtpInput | Form/input | yes | yes | no | no |
| SecretInput | Form/input | yes | yes | no | no |
| Dialog | Overlay | yes | yes | no | no |
| AlertDialog | Overlay | yes | yes | no | no |
| Popover | Overlay | yes | yes | no | no |
| Sheet | Overlay | yes | yes | no | no |
| BottomSheet | Overlay | yes | yes | no | no |
| Toast | Feedback | yes | yes | no | no |
| Notification | Feedback | yes | yes | no | no |
| Banner | Feedback | yes | yes | no | no |
| ContextMenu | Overlay | yes | yes | no | no |
| CommandPalette | Overlay | yes | yes | no | no |
| Tooltip | Overlay | yes | yes | no | no |
| Scaffold | Navigation | yes | yes | no | no |
| AppBar | Navigation | yes | yes | no | no |
| Sidebar | Navigation | yes | yes | no | no |
| NavigationBar | Navigation | yes | yes | no | no |
| NavigationRail | Navigation | yes | yes | no | no |
| Drawer | Overlay/navigation | yes | yes | no | no |
| Tabs | Navigation | yes | yes | no | no |
| Breadcrumb | Navigation | yes | yes | no | no |
| Stepper | Navigation | yes | yes | no | no |
| Pagination | Navigation | yes | yes | no | no |
| ListTile | Data display | yes | yes | no | no |
| SearchableList | Data display | yes | yes | no | no |
| VirtualList | Data display | yes | yes | no | no |
| DataTable | Data display | yes | yes | no | no |
| Tree | Data display | yes | yes | no | no |
| DescriptionList | Data display | yes | yes | no | no |
| Calendar | Data display | yes | yes | no | no |
| DatePicker | Data display | yes | yes | no | no |
| TimePicker | Data display | yes | yes | no | no |
| Separator | Container | yes | yes | yes | yes |
| Accordion | Deferred display | yes | yes | no | no |
| Collapsible | Deferred display | yes | yes | no | no |
| HoverCard | Deferred overlay | yes | yes | no | no |
| MenuBar | Navigation | yes | yes | no | no |
| StatusBar | Navigation | yes | yes | no | no |
| KeyboardShortcuts | Deferred display | yes | yes | no | no |
| Kbd | Deferred display | yes | yes | no | no |
| ColorPicker | Deferred input | yes | yes | no | no |
| Rating | Deferred input/display | yes | yes | no | no |
| Resizable | Deferred display | yes | yes | no | no |
| Dock | Deferred display | yes | yes | no | no |
| Chart | Deferred display | yes | yes | no | no |
| Editor | Deferred input/display | yes | yes | no | no |
| RichText | Deferred display | yes | yes | no | no |
| Carousel | Deferred display | yes | yes | no | no |
| DragDrop | Deferred input/display | yes | yes | no | no |
| Theme | Deferred display | yes | yes | no | no |
| AspectRatio | Container | yes | yes | yes | yes |
| Alert | Deferred feedback | yes | yes | no | no |
| Attachment | Deferred data display | yes | yes | no | no |
| Bubble | Deferred data display | yes | yes | no | no |
| Command | Deferred input | yes | yes | no | no |
| NativeSelect | Deferred input | yes | yes | no | no |
| NavigationMenu | Navigation | yes | yes | no | no |
| ScrollArea | Container | yes | yes | no | no |
| Item | Deferred data display | yes | yes | no | no |
| Message | Deferred data display | yes | yes | no | no |
| MessageScroller | Deferred data display | yes | yes | no | no |
| ToggleGroup | Form/input | yes | yes | no | no |
| Sonner | Deferred feedback | yes | yes | no | no |
