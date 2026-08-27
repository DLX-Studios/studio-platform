import { ALL_PRODUCTS, CATEGORIES, countFor } from "./catalog";

function esc(s: string): string {
  return s.indexOf("\"") < 0 ? s : s.replace("\"", "\\\"");
}

function textNode(id: string, text: string, role: string = ""): string {
  const r = role.length == 0 ? "" : ',"typography_role":"' + role + '"';
  return ['{"id":"', id, '","kind":"text","props":{"text":"', esc(text), '"', r, '},"children":[]}'].join("");
}
function iconNode(id: string, name: string): string {
  return ['{"id":"', id, '","kind":"icon","props":{"name":"', esc(name), '"},"children":[]}'].join("");
}
function badgeNode(id: string, label: string, available: bool): string {
  return ['{"id":"', id, '","kind":"tag","props":{"label":"', esc(label), '","variant":"', available ? "success" : "destructive", '"},"children":[]}'].join("");
}
function tagNode(id: string, label: string, variant: string = "default"): string {
  return ['{"id":"', id, '","kind":"tag","props":{"label":"', esc(label), '","variant":"', variant, '"},"children":[]}'].join("");
}
function buttonNode(id: string, label: string, variant: string = "", fullWidth: bool = false): string {
  const v = variant.length == 0 ? "" : ',"variant":"' + variant + '"';
  const width = fullWidth ? ',"width":"full"' : "";
  return ['{"id":"', id, '","kind":"button","props":{"label":"', esc(label), '"', v, width, ',"enabled":true,"on_pressed":"', id, '_pressed"},"children":[]}'].join("");
}
function disabledButtonNode(id: string, label: string): string {
  return ['{"id":"', id, '","kind":"button","props":{"label":"', esc(label), '","width":"full","enabled":false,"on_pressed":"', id, '_pressed"},"children":[]}'].join("");
}
function imageNode(id: string, asset: string, alt: string, width: i32 = 0, height: i32 = 0): string {
  const dimensions = width > 0 && height > 0 ? ',"width":' + width.toString() + ',"height":' + height.toString() : "";
  return ['{"id":"', id, '","kind":"image","props":{"asset":"', esc(asset), '","alt":"', esc(alt), '"', dimensions, '},"children":[]}'].join("");
}

function productCard(id: string, name: string, price: string, asset: string, available: bool): string {
  const badge = badgeNode(id + "-avail", available ? "● Available" : "● Not Available", available);
  const btn = available ? buttonNode("add-" + id, "Add to Cart", "", true) : disabledButtonNode("add-" + id, "Not Available");
  return [
    '{"id":"', id, '-card","kind":"card","props":{"padding":8,"visible":true},"children":[',
    '{"id":"', id, '-col","kind":"column","props":{"gap":8},"children":[',
    imageNode(id + "-img", asset, name, 0, 128), ',',
    badge, ',',
    '{"id":"', id, '-meta","kind":"row","props":{"gap":8,"alignment":"space_between"},"children":[',
    textNode(id + "-name", name, "label"), ',',
    textNode(id + "-price", price, "label"),
    ']},',
    btn,
    ']}]}',
  ].join("");
}

function cartLine(id: string, name: string, price: string, asset: string, note: string, initial: i32): string {
  return [
    '{"id":"', id, '-line","kind":"card","props":{"padding":8,"visible":', initial > 0 ? "true" : "false", '},"children":[',
    '{"id":"', id, '-line-row","kind":"row","props":{"gap":10},"children":[',
    imageNode(id + "-cart-img", asset, name, 72, 72), ',',
    '{"id":"', id, '-line-copy","kind":"column","props":{"gap":3,"flex":1},"children":[',
    '{"id":"', id, '-line-title","kind":"row","props":{"gap":4},"children":[',
    textNode(id + "-line-name", name, "label"), ',',
    textNode(id + "-qty", "(" + initial.toString() + ")", "caption"),
    ']},',
    textNode(id + "-line-note", note, "caption"), ',',
    textNode(id + "-line-price", price, "label"),
    ']},',
    buttonNode(id + "-dec", "−", "secondary"),
    ']}' ,
    ']}',
  ].join("");
}

function tabsRow(): string {
  // Pospay pill tabs with counts — keep title Studio Market, pills show real counts
  const pills = CATEGORIES.map<string>((c) => {
    const cnt = countFor(c);
    const label = c + " " + cnt.toString();
    const isMain = c == "Main Course";
    const slug = c.toLowerCase().replace(" ", "-");
    return ['{"id":"tab-', slug, '","kind":"button","props":{"label":"', esc(label), '","variant":"', isMain ? "selected" : "secondary", '","enabled":true,"on_pressed":"tab-', slug, '_pressed"},"children":[]}'].join("");
  }).join(",");
  return ['{"id":"desk-tabs","kind":"row","props":{"gap":8},"children":[', pills, ']}'].join("");
}

export function mountDesktop(): string {
  const cards = ALL_PRODUCTS.map<string>((p) => productCard(p.id, p.name, p.price.format(), p.asset, p.available)).join(",");
  const lines = ALL_PRODUCTS.map<string>((p) => {
    const initial: i32 = p.id == "french-fries" || p.id == "wagyu" || p.id == "chicken-ramen" ? 1 : 0;
    const note = p.id == "french-fries" ? "Notes: None  •  Size: Large" : p.id == "wagyu" ? "Notes: Well Med  •  Size: Small" : "Spicy: Normal  •  Size: Medium";
    return cartLine(p.id, p.name, p.price.format(), p.asset, note, initial);
  }).join(",");

  return ['{"type":"mount","payload":{"protocol_version":1,"route":"/pos","root":',
    '{"id":"root","kind":"scaffold","props":{},"children":[',

    // Studio Market title — keep per feedback, minimal chrome
    '{"id":"top-bar","kind":"app_bar","props":{"title":"Studio Market — Desktop POS"},"children":[',
    '{"id":"top-bar-row","kind":"row","props":{"gap":12},"children":[',
    iconNode("brand-icon", "store") + ',',
    textNode("brand-title", "Studio Market", "headline") + ',',
    textNode("brand-sub", "Cashier • 720p", "caption"),
    ']}',
    ']},',

    // Main row — sidebar fixed 220, catalog flex, order 390 (foundation handles)
    '{"id":"main-row","kind":"row","props":{"gap":0,"flex":1},"children":[',

    // Sidebar — fixed 240 per Pospay, left nav
    '{"id":"nav","kind":"sidebar","props":{"items":["Dashboard","Menu Order","Analytics","Withdrawal","Manage Table","Manage Dish","Manage Payment"]},"children":[',
    '{"id":"nav-col","kind":"column","props":{"gap":6},"children":[',
    textNode("nav-dashboard", "Dashboard", "caption") + ',',
    '{"id":"nav-menu","kind":"box","props":{"padding":8,"background":"surface_variant"},"children":[' + textNode("nav-menu-active", "Menu Order", "label") + ']},',
    textNode("nav-analytics", "Analytics", "caption") + ',',
    textNode("nav-withdrawal", "Withdrawal", "caption") + ',',
    textNode("nav-settings", "Settings", "caption"),
    ']}',
    ']},',

    // Catalog pane — Pospay close
    '{"id":"catalog-pane","kind":"box","props":{"padding":16,"flex":1},"children":[',
    '{"id":"catalog-col","kind":"column","props":{"gap":12},"children":[',

    '{"id":"toolbar","kind":"row","props":{"gap":12},"children":[',
    textNode("dish-menu", "▣  Dish Menu", "label") + ',' + tabsRow() + ',',
    buttonNode("refresh2", "↻  Refresh", "secondary") + ',',
    '{"id":"search","kind":"text_input","props":{"label":"Search Menu","value":"","placeholder":"Search Menu","enabled":true,"on_changed":"search_changed"},"children":[]}',
    ']},',

    // 4-col capped grid (pospay 4 cols) — images from assets
    '{"id":"catalog-scroll","kind":"scroll_view","props":{"axis":"vertical"},"children":[',
    '{"id":"product-grid","kind":"grid","props":{"columns":4,"gap":12},"children":[' + cards + ']}',
    ']}',

    ']}',
    ']},',

    // Order pane — fixed 390, order-content for stretch handling
    '{"id":"order-pane","kind":"box","props":{"padding":16,"background":"surface_variant","width":390,"shrink":true},"children":[',
    '{"id":"order-content","kind":"column","props":{"gap":12,"alignment":"space_between"},"children":[',
    '{"id":"order-head","kind":"row","props":{"gap":8,"alignment":"space_between"},"children":[',
    textNode("order-title", "Order Summary", "label") + ',',
    textNode("order-no", "#B12309", "caption"),
    ']},',
    '{"id":"cart-lines","kind":"list_view","props":{"axis":"vertical","gap":8},"children":[' + lines + ']},',
    // Fixed footer — order-summary flex_shrink_0 in foundation
    '{"id":"order-summary","kind":"box","props":{"padding":12,"shrink":true},"children":[',
    '{"id":"summary-col","kind":"column","props":{"gap":8},"children":[',
    '{"id":"subtotal-row","kind":"row","props":{"gap":8,"alignment":"space_between"},"children":[' + textNode("subtotal-label", "Subtotal", "caption") + ',' + textNode("subtotal", "$56.37", "caption") + ']},',
    '{"id":"taxes-row","kind":"row","props":{"gap":8,"alignment":"space_between"},"children":[' + textNode("taxes-label", "Taxes", "caption") + ',' + textNode("tax", "$5.63", "caption") + ']},',
    '{"id":"discount-row","kind":"row","props":{"gap":8,"alignment":"space_between"},"children":[' + textNode("discount-label", "Discount", "caption") + ',' + textNode("discount-amount", "-$5.63", "caption") + ']},',
    '{"id":"total-row","kind":"row","props":{"gap":8,"alignment":"space_between"},"children":[' + textNode("total-label", "Total Payment", "label") + ',' + textNode("total", "$56.37", "headline") + ']},',
    '{"id":"sep2","kind":"separator","props":{},"children":[]},',
    '{"id":"order-type","kind":"select","props":{"label":"Order type","value":"Dine-in","options":["Dine-in","Takeaway","Delivery"],"enabled":true},"children":[]},',
    '{"id":"select-table","kind":"select","props":{"label":"Table","value":"A-12B","options":["A-12B","A-13B","B-01"],"enabled":true},"children":[]},',
    buttonNode("discount-btn", "10% Discount") + ',',
    buttonNode("confirm", "Confirm Payment", "primary"),
    ']}',
    ']}',
    ']}',
    ']}',

    ']},',

    // Retained for validation/error announcements without adding a reference-external footer.
    '{"id":"status","kind":"status_bar","props":{"value":"Ready","visible":false},"children":[]},',

    // Discount dialog — single input shifts label per type (saves fuel)
    '{"id":"discount-dialog","kind":"dialog","props":{"title":"Apply Discount","open":false},"children":[',
    '{"id":"discount-col","kind":"column","props":{"gap":12},"children":[',
    textNode("discount-hint", "Choose discount type — ID required", "caption") + ',',
    '{"id":"discount-type","kind":"select","props":{"label":"Discount Type","value":"Promo","options":["Military","Senior","Employee","Promo"],"enabled":true,"on_changed":"discount_type_changed"},"children":[]},',
    '{"id":"discount-id","kind":"text_input","props":{"label":"ID","value":"","placeholder":"Enter ID","enabled":true,"on_changed":"discount_id_changed","visible":false},"children":[]},',
    '{"id":"promo-info","kind":"text","props":{"text":"Promo: 10% off for 2× Main Course + 1 Beverages","typography_role":"caption"},"children":[]},',
    buttonNode("apply-discount", "Apply") + ',',
    buttonNode("cancel-discount", "Cancel", "secondary"),
    ']}',
    ']},',

    '{"id":"palette-hidden","kind":"command_palette","props":{"placeholder":"Search Menu","open":false,"commands":["Butter Chicken","French Fries","Confirm Payment"],"visible":false},"children":[' + textNode("palette-hint", "Search Menu", "caption") + ']}',

    ']}',
    '}}'].join("");
}
