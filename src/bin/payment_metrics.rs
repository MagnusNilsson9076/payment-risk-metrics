use payment_risk_metrics::{
    infrai_metrics::InfraiMetrics, observe_payment, PaymentAction, PaymentEvent,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event = PaymentEvent {
        payment_id: "pay_1042".into(),
        merchant_id: "merchant_tools".into(),
        amount_minor: 12_500,
        currency: "USD".into(),
        risk_score: 82,
    };
    let action = observe_payment(&InfraiMetrics::from_env()?, &event).await?;

    match action {
        PaymentAction::Settle => println!("payment=pay_1042 action=settle"),
        PaymentAction::ManualReview { notification_id } => {
            println!("payment=pay_1042 action=manual_review notification={notification_id}")
        }
        PaymentAction::Decline { notification_id } => {
            println!("payment=pay_1042 action=decline notification={notification_id}")
        }
    }
    Ok(())
}

