pub mod infrai_metrics;

use std::{collections::BTreeMap, fmt};

use infrai_metrics::{InfraiMetrics, MetricsError};

#[derive(Debug, Clone)]
pub struct PaymentEvent {
    pub payment_id: String,
    pub merchant_id: String,
    pub amount_minor: u64,
    pub currency: String,
    pub risk_score: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentAction {
    Settle,
    ManualReview { notification_id: String },
    Decline { notification_id: String },
}

impl PaymentAction {
    fn metric_tag(&self) -> &'static str {
        match self {
            Self::Settle => "settle",
            Self::ManualReview { .. } => "manual_review",
            Self::Decline { .. } => "decline",
        }
    }
}

#[derive(Debug)]
pub enum PaymentServiceError {
    InvalidRiskScore(u8),
    Metrics(MetricsError),
}

impl fmt::Display for PaymentServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRiskScore(score) => write!(f, "risk score must be 0..=100, got {score}"),
            Self::Metrics(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PaymentServiceError {}

impl From<MetricsError> for PaymentServiceError {
    fn from(value: MetricsError) -> Self {
        Self::Metrics(value)
    }
}

pub fn decide_payment(event: &PaymentEvent) -> Result<PaymentAction, PaymentServiceError> {
    if event.risk_score > 100 {
        return Err(PaymentServiceError::InvalidRiskScore(event.risk_score));
    }
    let notification_id = || format!("risk-notice:{}", event.payment_id);
    Ok(match event.risk_score {
        90..=100 => PaymentAction::Decline { notification_id: notification_id() },
        70..=89 => PaymentAction::ManualReview { notification_id: notification_id() },
        _ => PaymentAction::Settle,
    })
}

pub async fn observe_payment(
    metrics: &InfraiMetrics,
    event: &PaymentEvent,
) -> Result<PaymentAction, PaymentServiceError> {
    let action = decide_payment(event)?;
    let tags = BTreeMap::from([
        ("merchant_id".into(), event.merchant_id.clone()),
        ("currency".into(), event.currency.clone()),
        ("action".into(), action.metric_tag().into()),
    ]);

    metrics.report(
        "counter",
        "payments.events",
        1.0,
        &tags,
        &format!("payment:{}:event", event.payment_id),
    ).await?;
    metrics.report(
        "gauge",
        "payments.amount_minor",
        event.amount_minor as f64,
        &tags,
        &format!("payment:{}:amount", event.payment_id),
    ).await?;
    metrics.report(
        "gauge",
        "payments.risk_score",
        f64::from(event.risk_score),
        &tags,
        &format!("payment:{}:risk", event.payment_id),
    ).await?;

    Ok(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elevated_risk_creates_a_traceable_manual_review() {
        let event = PaymentEvent {
            payment_id: "pay_1042".into(),
            merchant_id: "merchant_tools".into(),
            amount_minor: 12_500,
            currency: "USD".into(),
            risk_score: 82,
        };

        assert_eq!(
            decide_payment(&event).unwrap(),
            PaymentAction::ManualReview {
                notification_id: "risk-notice:pay_1042".into(),
            }
        );
    }
}

