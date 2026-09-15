import { Cart } from "./cart";
import { ALL_PRODUCTS } from "./catalog";
import { mountDesktop } from "./ui";

@external("studio_host", "emit")
declare function hostEmit(pointer: i32, length: i32): i32;

function studioAbort(_m: usize, _f: usize, _l: u32, _c: u32): void { unreachable(); }

const cart = new Cart();
cart.add("french-fries");
cart.add("wagyu");
cart.add("chicken-ramen");
cart.setDiscountFraction(0.10);
let sequence: i64 = 0;

let currentQuery: string = "";
let currentCategory: string = "All";
let discountType: string = "Promo";
let discountId: string = "";

function retained(): void {}
let keep: () => void = retained;

function emit(msg: string): i32 {
  const b = String.UTF8.encode(msg, false);
  return hostEmit(changetype<i32>(b), b.byteLength);
}
function q(v: string): string { return '"' + v + '"'; }
function upd(node: string, prop: string, valJson: string): string {
  return '{"op":"update_prop","node_id":"' + node + '","property":"' + prop + '","value":' + valJson + '}';
}
function totalsOps(): string[] {
  return [
    upd("subtotal", "text", q(cart.subtotal().format())),
    upd("tax", "text", q(cart.tax().format())),
    upd("discount-amount", "text", q(cart.discountLabel())),
    upd("total", "text", q(cart.total().format())),
  ];
}
function patch(ops: string[]): i32 {
  sequence += 1;
  return emit('{"type":"patch","payload":{"sequence":' + sequence.toString() + ',"operations":[' + ops.join(",") + ']}}');
}
function changed(id: string, qty: i32): i32 {
  return patch([
    upd(id + "-line", "visible", qty > 0 ? "true" : "false"),
    upd(id + "-qty", "text", q("(" + qty.toString() + ")")),
  ].concat(totalsOps()));
}
function extractText(event: string): string {
  const m = '"value":"';
  const s = event.indexOf(m);
  if (s < 0) return "";
  const a = s + m.length;
  const b = event.indexOf('"', a);
  return b < 0 ? "" : event.substring(a, b);
}
function filterVisible(query: string, category: string): string[] {
  const ops: string[] = [];
  for (let i = 0; i < ALL_PRODUCTS.length; i++) {
    const p = ALL_PRODUCTS[i];
    const ok = p.matches(query) && p.matchesCategory(category);
    ops.push(upd(p.id + "-card", "visible", ok ? "true" : "false"));
  }
  return ops;
}
function promoEligible(): bool {
  // Promo 10% off for combos: 2× Main Course + 1 Beverages, or subtotal >= $50
  let main = 0;
  let bev = 0;
  for (let i = 0; i < ALL_PRODUCTS.length; i++) {
    const p = ALL_PRODUCTS[i];
    const qty = cart.quantity(p.id);
    if (p.category == "Main Course") main += qty;
    if (p.category == "Beverages") bev += qty;
  }
  if (main >= 2 && bev >= 1) return true;
  if (cart.subtotal().cents >= 5000) return true;
  return false;
}

export function studio_alloc(len: i32): i32 { return len <= 65536 ? 15728640 : 0; }
export function studio_dealloc(_p: i32, _l: i32): void {}

export function studio_init(_p: i32, _l: i32): i32 {
  keep();
  return emit(mountDesktop());
}

export function studio_event(ptr: i32, len: i32): i32 {
  keep = retained;
  const ev = String.UTF8.decodeUnsafe(ptr, len, false);

  if (ev.includes('"node_id":"search"')) {
    currentQuery = extractText(ev);
    return patch(filterVisible(currentQuery, currentCategory));
  }

  // Tabs — Pospay pills
  if (ev.includes('"node_id":"tab-all"')) { currentCategory = "All"; return patch(filterVisible(currentQuery, currentCategory)); }
  if (ev.includes('"node_id":"tab-beverages"')) { currentCategory = "Beverages"; return patch(filterVisible(currentQuery, currentCategory)); }
  if (ev.includes('"node_id":"tab-main-course"')) { currentCategory = "Main Course"; return patch(filterVisible(currentQuery, currentCategory)); }
  if (ev.includes('"node_id":"tab-dessert"')) { currentCategory = "Dessert"; return patch(filterVisible(currentQuery, currentCategory)); }
  if (ev.includes('"node_id":"tab-appetizer"')) { currentCategory = "Appetizer"; return patch(filterVisible(currentQuery, currentCategory)); }

  // Discount dialog open
  if (ev.includes('"node_id":"discount-btn"')) {
    return patch([upd("discount-dialog", "open", "true")]);
  }
  if (ev.includes('"node_id":"cancel-discount"')) {
    return patch([upd("discount-dialog", "open", "false")]);
  }
  if (ev.includes('"node_id":"discount-type"')) {
    discountType = extractText(ev);
    let isPromo = discountType == "Promo";
    let label = discountType == "Military" ? "Military ID" : discountType == "Senior" ? "Driver License" : discountType == "Employee" ? "Employee ID" : "ID";
    return patch([
      upd("discount-type", "value", q(discountType)),
      upd("discount-id", "visible", isPromo ? "false" : "true"),
      upd("discount-id", "label", q(label)),
      upd("discount-id", "placeholder", q(label)),
      upd("promo-info", "visible", isPromo ? "true" : "false"),
    ]);
  }
  if (ev.includes('"node_id":"discount-id"')) { discountId = extractText(ev); return 0; }

  if (ev.includes('"node_id":"apply-discount"')) {
    if (discountType == "Military" || discountType == "Senior" || discountType == "Employee") {
      if (discountId.length == 0) return patch([upd("status", "value", q(discountType + " ID required"))]);
      if (discountType == "Military") cart.setDiscountFraction(0.15);
      else if (discountType == "Senior") cart.setDiscountFraction(0.10);
      else cart.setDiscountFraction(0.20);
    } else { // Promo
      if (!promoEligible()) return patch([upd("status", "value", q("Promo not eligible — need 2× Main Course + 1 Beverages or $50+"))]);
      cart.setDiscountFraction(0.10);
    }
    discountId = "";
    return patch([upd("discount-dialog", "open", "false"), upd("discount-id", "value", q("")), upd("status", "value", q(discountType + " discount applied"))].concat(totalsOps()));
  }

  // Confirm / clear
  if (ev.includes('"node_id":"confirm"')) {
    if (cart.isEmpty()) return patch([upd("status", "value", q("Cart empty"))]);
    cart.clear();
    const resets: string[] = [];
    for (let i = 0; i < ALL_PRODUCTS.length; i++) {
      const id = ALL_PRODUCTS[i].id;
      resets.push(upd(id + "-line", "visible", "false"));
      resets.push(upd(id + "-qty", "text", q("0")));
      resets.push(upd(id + "-card", "visible", "true"));
    }
    currentQuery = "";
    currentCategory = "All";
    discountType = "Promo";
    discountId = "";
    return patch(resets.concat([
      upd("search", "value", q("")),
      upd("discount-dialog", "open", "false"),
      upd("status", "value", q("Payment confirmed — next order")),
    ]).concat(totalsOps()));
  }

  if (ev.includes('"node_id":"refresh"') || ev.includes('"node_id":"refresh2"')) {
    currentQuery = ""; currentCategory = "All";
    return patch(filterVisible("", "All").concat([upd("search", "value", q(""))]));
  }

  // Add to cart — respect available flag (disabled button still fires? guard)
  for (let i = 0; i < ALL_PRODUCTS.length; i++) {
    const p = ALL_PRODUCTS[i];
    if (!p.available) continue;
    if (ev.includes('"node_id":"add-' + p.id + '"') || ev.includes('"node_id":"more-' + p.id + '"')) {
      cart.add(p.id);
      return changed(p.id, cart.quantity(p.id));
    }
    if (ev.includes('"node_id":"' + p.id + '-inc"')) { cart.add(p.id); return changed(p.id, cart.quantity(p.id)); }
    if (ev.includes('"node_id":"' + p.id + '-dec"')) { cart.remove(p.id); return changed(p.id, cart.quantity(p.id)); }
  }

  return 0;
}
