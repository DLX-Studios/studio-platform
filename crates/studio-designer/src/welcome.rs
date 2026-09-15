//! Product welcome matching the application-shell Monolith prototype.

use gpui::{
    App, ClickEvent, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px, relative, rgb,
};
use gpui_component::{TitleBar, h_flex, v_flex};

use crate::shell_theme::{
    COLOR_BG, COLOR_BRAND, COLOR_LINE, COLOR_LINE_STRONG, COLOR_MUTED, COLOR_PANEL, COLOR_PANEL_2,
    COLOR_SOFT, COLOR_TEXT,
};

pub const WELCOME_EYEBROW: &str = "AI-native Studio authoring";
pub const WELCOME_HEADLINE: &str = "Design the interface. Shape the runtime.";
pub const WELCOME_LEAD: &str = "A native design workspace for creating responsive Studio applications with semantic primitives, live agents, and one verified path into Studio Runtime.";
pub const WELCOME_GET_STARTED: &str = "Get started →";
pub const WELCOME_OPEN_LOCAL: &str = "Open local identity";

pub fn welcome_screen(
    on_get_started: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_open_local: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    v_flex()
        .id("product-welcome")
        .size_full()
        .bg(rgb(COLOR_BG))
        .text_color(rgb(COLOR_TEXT))
        .font_family("monospace")
        .child(standalone_titlebar())
        .child(
            h_flex()
                .id("welcome-monolith")
                .flex_1()
                .min_h_0()
                .w_full()
                .items_start()
                .child(copy_column(on_get_started, on_open_local))
                .child(board_column()),
        )
}

fn standalone_titlebar() -> impl IntoElement {
    TitleBar::new()
        .h(px(52.))
        .px_3()
        .bg(rgb(COLOR_BG))
        .border_color(rgb(COLOR_LINE))
        .child(
            h_flex().gap_3().child(brand_mark()).child(
                v_flex()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .child("Studio Designer"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(COLOR_MUTED))
                            .child("Product welcome"),
                    ),
            ),
        )
}

fn brand_mark() -> impl IntoElement {
    div()
        .w(px(28.))
        .h(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(COLOR_TEXT))
        .text_color(rgb(COLOR_BG))
        .font_weight(FontWeight::BOLD)
        .child("S")
}

fn copy_column(
    on_get_started: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_open_local: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    v_flex()
        .id("welcome-copy-column")
        .h_full()
        .flex_shrink_0()
        .w(relative(0.44))
        .min_w(px(360.))
        .px(px(64.))
        .pt(px(34.))
        .pb(px(48.))
        .border_r_1()
        .border_color(rgb(COLOR_LINE))
        .child(product_brand())
        .child(
            v_flex()
                .id("welcome-copy")
                .flex_1()
                .justify_center()
                .gap_5()
                .max_w(px(650.))
                .child(eyebrow(WELCOME_EYEBROW))
                .child(
                    div()
                        .text_size(px(52.))
                        .font_weight(FontWeight::BOLD)
                        .line_height(relative(1.02))
                        .child(WELCOME_HEADLINE),
                )
                .child(
                    div()
                        .max_w(px(540.))
                        .text_sm()
                        .text_color(rgb(COLOR_SOFT))
                        .line_height(relative(1.75))
                        .child(WELCOME_LEAD),
                )
                .child(
                    h_flex()
                        .gap_3()
                        .child(primary_button(
                            "dismiss-welcome",
                            WELCOME_GET_STARTED,
                            on_get_started,
                        ))
                        .child(secondary_button(
                            "open-local-identity",
                            WELCOME_OPEN_LOCAL,
                            on_open_local,
                        )),
                ),
        )
        .child(
            h_flex()
                .pt_4()
                .border_t_1()
                .border_color(rgb(COLOR_LINE))
                .justify_between()
                .child(footer_meta("Local-first"))
                .child(footer_meta("Desktop · Studio Runtime")),
        )
}

fn product_brand() -> impl IntoElement {
    h_flex().gap_3().child(brand_mark()).child(
        v_flex()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Studio Designer"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(COLOR_MUTED))
                    .child("Native authoring"),
            ),
    )
}

fn board_column() -> impl IntoElement {
    div()
        .id("welcome-board")
        .relative()
        .flex_1()
        .h_full()
        .min_w_0()
        .bg(rgb(COLOR_PANEL))
        .child(artifact_card(
            "Mobile storefront",
            "390 × 844",
            px(72.),
            px(64.),
            px(420.),
        ))
        .child(artifact_card(
            "Operations dashboard",
            "1440 × 1024",
            px(280.),
            px(220.),
            px(380.),
        ))
        .child(
            div()
                .id("welcome-connected-chip")
                .absolute()
                .top(px(250.))
                .left(px(32.))
                .px_3()
                .py_2()
                .border_1()
                .border_color(rgb(COLOR_LINE_STRONG))
                .bg(rgb(COLOR_BG))
                .text_xs()
                .text_color(rgb(COLOR_MUTED))
                .child("02 connected screens"),
        )
}

fn artifact_card(
    name: &'static str,
    size: &'static str,
    top: gpui::Pixels,
    left: gpui::Pixels,
    width: gpui::Pixels,
) -> impl IntoElement {
    v_flex()
        .absolute()
        .top(top)
        .left(left)
        .w(width)
        .border_1()
        .border_color(rgb(COLOR_LINE_STRONG))
        .bg(rgb(COLOR_PANEL_2))
        .child(
            h_flex()
                .h(px(36.))
                .px_3()
                .justify_between()
                .border_b_1()
                .border_color(rgb(COLOR_LINE))
                .child(div().text_xs().text_color(rgb(COLOR_MUTED)).child(name))
                .child(div().text_xs().text_color(rgb(COLOR_MUTED)).child(size)),
        )
        .child(
            h_flex()
                .h(px(160.))
                .p_4()
                .gap_3()
                .child(
                    div()
                        .w(relative(0.34))
                        .h_full()
                        .border_1()
                        .border_color(rgb(COLOR_LINE))
                        .bg(rgb(COLOR_BG)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .h_full()
                        .gap_2()
                        .child(div().h(px(24.)).w(relative(0.62)).bg(rgb(COLOR_TEXT)))
                        .child(div().h(px(7.)).w_full().bg(rgb(COLOR_LINE_STRONG)))
                        .child(div().h(px(7.)).w(relative(0.72)).bg(rgb(COLOR_LINE_STRONG)))
                        .child(
                            h_flex()
                                .mt_auto()
                                .gap_2()
                                .child(
                                    div()
                                        .h(px(40.))
                                        .flex_1()
                                        .border_1()
                                        .border_color(rgb(COLOR_LINE))
                                        .bg(rgb(COLOR_BG)),
                                )
                                .child(
                                    div()
                                        .h(px(40.))
                                        .flex_1()
                                        .border_1()
                                        .border_color(rgb(COLOR_LINE))
                                        .bg(rgb(COLOR_BG)),
                                ),
                        ),
                ),
        )
}

fn eyebrow(label: &'static str) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(rgb(COLOR_MUTED))
        .child(label.to_uppercase())
}

fn footer_meta(label: &'static str) -> impl IntoElement {
    div().text_xs().text_color(rgb(COLOR_MUTED)).child(label)
}

fn primary_button(
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .h(px(40.))
        .px_4()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(COLOR_BRAND))
        .text_color(rgb(COLOR_BG))
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .on_click(on_click)
        .child(label)
}

fn secondary_button(
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .h(px(40.))
        .px_4()
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(rgb(COLOR_LINE_STRONG))
        .text_color(rgb(COLOR_TEXT))
        .text_sm()
        .cursor_pointer()
        .on_click(on_click)
        .child(label)
}
