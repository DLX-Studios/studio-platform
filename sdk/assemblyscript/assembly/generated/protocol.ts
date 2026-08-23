// Generated from Rust-authoritative protocol-v1 schemas. Do not edit.

/** Host–guest protocol major version. */
export const PROTOCOL_VERSION: u16 = 1;

export const GUEST_MESSAGE_MOUNT: string = "mount";
export const GUEST_MESSAGE_PATCH: string = "patch";
export const GUEST_MESSAGE_NAVIGATE: string = "navigate";
export const GUEST_MESSAGE_ACTION: string = "action";
export const GUEST_MESSAGE_LOG: string = "log";
export const HOST_EVENT_UI: string = "ui";
export const HOST_EVENT_NAVIGATION: string = "navigation";
export const HOST_EVENT_ACTION_RESULT: string = "action_result";
export const HOST_EVENT_LIFECYCLE: string = "lifecycle";
export const PATCH_OPERATION_UPDATE_PROP: string = "update_prop";
export const PATCH_OPERATION_INSERT_CHILD: string = "insert_child";
export const PATCH_OPERATION_REMOVE_NODE: string = "remove_node";
export const PATCH_OPERATION_REPLACE_NODE: string = "replace_node";
export const NAVIGATION_OPERATION_PUSH: string = "push";
export const NAVIGATION_OPERATION_REPLACE: string = "replace";
export const NAVIGATION_OPERATION_POP: string = "pop";
export const NAVIGATION_OPERATION_POP_TO: string = "pop_to";
export const NAVIGATION_OPERATION_RESET: string = "reset";
export const ACTION_STATUS_SUCCESS: string = "success";
export const ACTION_STATUS_FAILURE: string = "failure";
export const LIFECYCLE_STATE_LOADING: string = "loading";
export const LIFECYCLE_STATE_RUNNING: string = "running";
export const LIFECYCLE_STATE_TRAPPED: string = "trapped";
export const LIFECYCLE_STATE_STOPPED: string = "stopped";
export const NODE_KIND_BOX: string = "box";
export const NODE_KIND_COLUMN: string = "column";
export const NODE_KIND_ROW: string = "row";
export const NODE_KIND_STACK: string = "stack";
export const NODE_KIND_GRID: string = "grid";
export const NODE_KIND_SCROLL_VIEW: string = "scroll_view";
export const NODE_KIND_LIST_VIEW: string = "list_view";
export const NODE_KIND_SPACER: string = "spacer";
export const NODE_KIND_DIVIDER: string = "divider";
export const NODE_KIND_TEXT: string = "text";
export const NODE_KIND_ICON: string = "icon";
export const NODE_KIND_IMAGE: string = "image";
export const NODE_KIND_CARD: string = "card";
export const NODE_KIND_BADGE: string = "badge";
export const NODE_KIND_PROGRESS_INDICATOR: string = "progress_indicator";
export const NODE_KIND_BUTTON: string = "button";
export const NODE_KIND_ICON_BUTTON: string = "icon_button";
export const NODE_KIND_CHECKBOX: string = "checkbox";
export const NODE_KIND_SWITCH: string = "switch";
export const NODE_KIND_SLIDER: string = "slider";
export const NODE_KIND_SELECT: string = "select";
export const NODE_KIND_TEXT_INPUT: string = "text_input";
export const NODE_KIND_SECRET_INPUT: string = "secret_input";
export const NODE_KIND_DIALOG: string = "dialog";
export const NODE_KIND_BOTTOM_SHEET: string = "bottom_sheet";
export const NODE_KIND_TOAST: string = "toast";
export const NODE_KIND_TOOLTIP: string = "tooltip";

/** A copied JSON envelope ready for the Studio ABI boundary. */
export class ProtocolEnvelopeV1 {
  constructor(
    public readonly type: string,
    public readonly payloadJson: string,
  ) {}
}
