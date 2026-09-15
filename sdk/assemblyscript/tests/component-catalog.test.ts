import { describe, expect, test } from "bun:test";
import {
  Avatar, Badge, BottomSheet, Box, Button, Card, Checkbox, Column, Container, Dialog, Divider,
  EmptyState, Grid, Icon, IconButton, Image, ListView, ProgressCircle, ProgressIndicator, Row, ScrollView, SecretInput,
  Select, Slider, Spacer, Stack, SwitchControl, Text, TextInput, TextArea, Toast, Tooltip,
  Skeleton, Spinner, Tag,
  ButtonGroup, Combobox, Field, InputGroup, NumberInput, OtpInput, Radio, RangeSlider, Toggle,
  AlertDialog, Banner, CommandPalette, ContextMenu, Notification, Popover, Sheet,
} from "../assembly/widgets";

describe("closed AssemblyScript component catalog", () => {
  test("builds every layout, display, interaction, and overlay primitive", () => {
    const leaf = Text("content", "Studio");
    const nodes = [
      Box("box", [leaf]), Column("column", []), Row("row", []), Stack("stack", []),
      Grid("grid", 3, []), ScrollView("scroll", "vertical", leaf),
      ListView("list", "vertical", []), Spacer("space", 8), Divider("line", 1),
      Text("text", "value"), Icon("icon", "search"), Image("image", "assets/a.png", "A"),
      Card("card", leaf), Badge("badge", "New"), Tag("tag", "Featured", "success"),
      Avatar("avatar", "JD"), EmptyState("empty", "No items"), Skeleton("skeleton", 80, 12),
      ProgressIndicator("progress", 0.5), ProgressCircle("progress-circle", 0.5), Spinner("spinner"),
      Button("button", "Pay", "pay"), IconButton("icon-button", "close", "close"),
      Checkbox("check", "Tax", false, "tax"), SwitchControl("switch", "Receipt", true, "receipt"),
      Slider("slider", "Discount", 0, 1, 0.2, "discount"),
      RangeSlider("range", "Price", 0, 100, 20, 80, "range"),
      Select("select", "Category", "All", ["All", "Hair"], "category"),
      Combobox("combo", "Service", "Hair", ["Hair"], "combo"),
      NumberInput("number", "Quantity", 1, 0, 10, 1, "quantity"),
      TextInput("input", "Search", "", "search"), SecretInput("secret", "PIN", "pin_ready"),
      Radio("radio", "Choice", false, "radio"), Toggle("toggle", "Enabled", true, "toggle"),
      ButtonGroup("group", [Button("group-button", "One", "one")]),
      TextArea("area", "Notes", "", "notes"), Field("field", "Service"),
      InputGroup("input-group", "Amount", TextInput("group-input", "Amount", "", "amount")),
      OtpInput("otp", "Code", 6, "", "otp"),
      Dialog("dialog", "Confirm", true, leaf), AlertDialog("alert", "Confirm", "Proceed?", true, leaf),
      Popover("popover", true, leaf), Sheet("sheet", true, leaf), BottomSheet("bottom", true, leaf),
      Toast("toast", "Saved"), Notification("notification", "Saved"), Banner("banner", "Notice"),
      ContextMenu("context", "Actions", leaf), CommandPalette("commands", "Search", ["Pay"], true, leaf),
      Tooltip("tooltip", "Search", leaf),
    ];
    expect(nodes.map((node) => node.kind)).toEqual([
      "box", "column", "row", "stack", "grid", "scroll_view", "list_view", "spacer",
      "divider", "text", "icon", "image", "card", "badge", "tag", "avatar", "empty", "skeleton",
      "progress_indicator", "progress_circle", "spinner", "button",
      "icon_button", "checkbox", "switch", "slider", "range_slider", "select", "combobox",
      "number_input", "text_input", "secret_input", "radio", "toggle", "button_group",
      "text_area", "field", "input_group", "otp_input",
      "dialog", "alert_dialog", "popover", "sheet", "bottom_sheet", "toast", "notification",
      "banner", "context_menu", "command_palette", "tooltip",
    ]);
    expect(new Set(nodes.map((node) => node.id)).size).toBe(nodes.length);
    expect(nodes[21]?.props.get("on_pressed")).toBe("pay");
    expect(nodes[31]?.props.has("value")).toBe(false);
  });

  test("Container is a compatibility alias for Box", () => {
    expect(Container("legacy", []).kind).toBe("box");
    expect(Container("legacy", []).props.size).toBe(Box("modern", []).props.size);
  });
});
