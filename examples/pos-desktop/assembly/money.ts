export class Money {
  constructor(public cents: i32) {}

  static fromCents(cents: i32): Money { return new Money(cents); }

  add(other: Money): Money { return new Money(this.cents + other.cents); }

  mul(qty: i32): Money { return new Money(this.cents * qty); }

  discount(basisPoints: i32): Money {
    return new Money((this.cents * (10000 - basisPoints)) / 10000);
  }

  taxRate(bps: i32): Money {
    return new Money((this.cents * bps) / 10000);
  }

  format(): string {
    const sign = this.cents < 0 ? "-" : "";
    const abs = this.cents < 0 ? -this.cents : this.cents;
    const dollars = abs / 100;
    const cents = abs % 100;
    const centsStr = cents < 10 ? "0" + cents.toString() : cents.toString();
    // Emit the decimal format consumed directly by generic text rendering.
    return sign + "$" + dollars.toString() + "." + centsStr;
  }
}
