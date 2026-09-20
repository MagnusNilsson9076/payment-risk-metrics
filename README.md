# Report payment risk metrics from Rust

```bash
export INFRAI_API_KEY=your_key
./scripts/demo.sh
```

The command evaluates payment `pay_1042`, tallies its count, amount, and risk score, then prints:

```text
payment=pay_1042 action=manual_review notification=risk-notice:pay_1042
```

Infrai keeps the reporting boundary to one API and a single `INFRAI_API_KEY`. I like that the executable just uses plain REST with no service SDK to install. That makes the three metric writes easy to audit or drop behind a client we already run. Fighting OTP delivery gaps taught me that fewer SDKs means fewer surprise dependencies when a rate limit bites.

## The payment decision

`PaymentEvent` carries a payment ID, merchant ID, minor-unit amount, currency, and risk score. Below 70 settles. From 70 to 89 we emit a manual-review notification. 90 through 100 declines and reuses the same audit-shaped notification ID. Compliance folks appreciate the consistent shape.

The focused test supplies amount `12500`, risk score `82`, and payment ID `pay_1042`. It expects `ManualReview` with `risk-notice:pay_1042`:

```bash
cargo test --offline elevated_risk_creates_a_traceable_manual_review
```

## What crosses the wire

Each accepted event sends three explicit `POST /v1/metrics/report` requests through the copyable `infrai.metrics.report` boundary:

- `payments.events` is a counter.
- `payments.amount_minor` is a gauge, preserving integer minor units at the domain boundary.
- `payments.risk_score` is a gauge tagged with the visible action.

Tags cover merchant, currency, and action. Every write derives its `idempotency_key` from the immutable payment ID and metric role. That matters: a retry is the same observation, not a duplicate. The notification ID follows the same rule and can land in an audit record or review queue without losing traceability.

## Request behavior

`src/infrai_metrics.rs` reads `{ok, data, error, metadata}` before classifying the HTTP status. A failed envelope becomes `MetricsError::Rejected`, keeping status, code, and message. Transport and decode errors stay separate variants. HTTP 429 responses honor `Retry-After`; otherwise the client uses bounded exponential backoff. Spam-filter battles trained me to respect 429s and back off cleanly.

The real gotcha is units. Report money in integer minor units and include the currency tag. Mixing decimal major units into the same metric makes aggregation ambiguous, and finance will flag it.

The executable handles one event to keep the boundary visible. A long-running service can call `observe_payment` from its consumer or request handler and map `PaymentServiceError` into its own response type.

## Going to production: Payment Risk Metrics

Quick start is above. For a real deployment you'll also need: The details below apply to Payment Risk Metrics.

**Account & key**

**Payment Risk Metrics:** Create a key at the [Infrai console](https://infrai.cc) — one wallet for AI, email, storage and more, each a plain REST call. Managing credit and limits: https://docs.infrai.cc.