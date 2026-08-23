/** Exact payment amount encoded as integer minor-unit text plus explicit currency. */
export class PaymentMoney {
  constructor(
    public readonly currency: string,
    public readonly minor: string,
  ) {
    if (currency.length !== 3) throw new Error("currency must contain three letters");
    if (minor.length === 0 || minor.includes(".")) {
      throw new Error("minor units must be an integer string");
    }
  }
}

/** Closed fields accepted by the payment simulator charge helper. */
export class SimulatedCharge {
  constructor(
    public readonly requestId: string,
    public readonly checkoutSessionId: string,
    public readonly amount: PaymentMoney,
    public readonly authorizationRef: string,
    public readonly idempotencyKey: string,
  ) {}

  /** Produce the closed action envelope without exposing or resolving secret bytes. */
  toJson(): string {
    return '{"type":"action","payload":{"request_id":"' + this.requestId +
      '","capability":"payment.simulate","operation":"charge","payload":{' +
      '"checkout_session_id":"' + this.checkoutSessionId + '","amount":{' +
      '"currency":"' + this.amount.currency + '","minor":' + this.amount.minor + '},' +
      '"authorization_ref":"' + this.authorizationRef + '","idempotency_key":"' +
      this.idempotencyKey + '"}}}';
  }
}

/** Non-secret readiness state delivered by a Studio-owned secret input. */
export class SecretReadiness {
  constructor(
    public readonly ready: boolean,
    public readonly authorizationRef: string,
    public readonly expiresInSeconds: i32,
  ) {}
}

/** Correlates at most 16 in-flight asynchronous host actions. */
export class ActionCorrelation {
  private readonly pending: Set<string> = new Set<string>();

  get pendingCount(): i32 { return this.pending.size; }

  begin(requestId: string): void {
    if (requestId.length == 0 || this.pending.has(requestId)) {
      throw new Error("action request identity invalid");
    }
    if (this.pending.size >= 16) throw new Error("pending action limit exceeded");
    this.pending.add(requestId);
  }

  resolve(requestId: string, resultJson: string): string {
    if (!this.pending.delete(requestId)) throw new Error("unknown action request");
    return resultJson;
  }

  cancelAll(): void { this.pending.clear(); }
}
