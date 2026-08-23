import { Money } from "./money";
import { ALL_PRODUCTS } from "./catalog";

export class Cart {
  private qty: Map<string, i32> = new Map<string, i32>();
  discountBasisPoints: i32 = 0;

  constructor() {
    for (let i = 0; i < ALL_PRODUCTS.length; i++) {
      this.qty.set(ALL_PRODUCTS[i].id, 0);
    }
  }

  add(id: string): void {
    const cur = this.qty.get(id);
    this.qty.set(id, cur + 1);
  }

  remove(id: string): void {
    const cur = this.qty.get(id);
    if (cur > 0) this.qty.set(id, cur - 1);
  }

  quantity(id: string): i32 { return this.qty.get(id); }

  itemCount(): i32 {
    let total: i32 = 0;
    const vals = this.qty.values();
    for (let i = 0; i < vals.length; i++) total += vals[i];
    return total;
  }

  itemCountLabel(): string {
    const n = this.itemCount();
    return n.toString() + (n == 1 ? " item" : " items");
  }

  subtotal(): Money {
    let cents: i32 = 0;
    for (let i = 0; i < ALL_PRODUCTS.length; i++) {
      const p = ALL_PRODUCTS[i];
      cents += p.price.cents * this.quantity(p.id);
    }
    return Money.fromCents(cents);
  }

  discountAmount(): Money {
    const sub = this.subtotal().cents;
    return Money.fromCents((sub * this.discountBasisPoints) / 10000);
  }

  discountedSubtotal(): Money {
    return Money.fromCents(this.subtotal().cents - this.discountAmount().cents);
  }

  tax(): Money {
    // Pospay reference: tax and promotion are both 10% of the subtotal.
    return Money.fromCents((this.subtotal().cents * 1000) / 10000);
  }

  total(): Money {
    return Money.fromCents(this.discountedSubtotal().cents + this.tax().cents);
  }

  discountLabel(): string {
    const amt = this.discountAmount();
    return amt.cents == 0 ? "-$0.00" : "-" + amt.format();
  }

  setDiscountFraction(fraction: f64): void {
    if (fraction < 0.0) fraction = 0.0;
    if (fraction > 0.3) fraction = 0.3;
    this.discountBasisPoints = i32(Math.round(fraction * 10000.0));
  }

  clear(): void {
    for (let i = 0; i < ALL_PRODUCTS.length; i++) this.qty.set(ALL_PRODUCTS[i].id, 0);
    this.discountBasisPoints = 0;
  }

  isEmpty(): bool { return this.itemCount() == 0; }
}
