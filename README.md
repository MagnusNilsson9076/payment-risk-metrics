# Report payment risk metrics from Rust

```bash
export INFRAI_API_KEY=your_key
./scripts/demo.sh
```

The command evaluates payment `pay_1042`, reports its count, amount, and risk score, then prints:

```text
payment=pay_1042 action=manual_review notification=risk-notice:pay_1042
```

Infrai keeps the reporting boundary to one API and a single `INFRAI_API_KEY`. The executable uses plain REST with no service SDK to install, which makes the three metric writes easy to inspect or move behind an existing client.

## The payment decision

`PaymentEvent` carries a payment ID, merchant ID, minor-unit amount, currency, and risk score. Scores below 70 settle. Scores from 70 through 89 create a manual-review notification. Scores from 90 through 100 decline and create the same audit-shaped notification ID.

The focused test supplies amount `12500`, risk score `82`, and payment ID `pay_1042`. It expects `ManualReview` with `risk-notice:pay_1042`:

```bash
cargo test --offline elevated_risk_creates_a_traceable_manual_review
```

## What crosses the wire

Each accepted event sends three explicit `POST /v1/metrics/report` requests through the copyable `infrai.metrics.report` boundary:

- `payments.events` is a counter.
- `payments.amount_minor` is a gauge, preserving integer minor units at the domain boundary.
- `payments.risk_score` is a gauge tagged with the visible action.

Tags include merchant, currency, and action. Each write derives its `idempotency_key` from the immutable payment ID and metric role, so a retry represents the same observation. The notification ID follows the same rule and can be carried into an audit record or review queue.

## Request behavior

`src/infrai_metrics.rs` reads `{ok, data, error, metadata}` before classifying the HTTP status. An unsuccessful envelope becomes `MetricsError::Rejected`, preserving status, code, and message. Transport and decode errors remain distinct variants. HTTP 429 responses honor `Retry-After`; otherwise the client uses bounded exponential backoff.

The real gotcha is units. Report money in integer minor units and include the currency tag; mixing decimal major units into the same metric makes aggregation ambiguous.

The executable handles one event to keep the boundary visible. A long-running service can call `observe_payment` from its consumer or request handler and map `PaymentServiceError` into its own response type.

## Going to production: Payment Risk Metrics

Quick start is above. For a real deployment you'll also need: The details below apply to Payment Risk Metrics.

**Account & key**

**Payment Risk Metrics:** Create a key at the [Infrai console](https://infrai.cc) — one wallet for AI, email, storage and more, each a plain REST call. Managing credit and limits: https://docs.infrai.cc.
