use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonRequest {
    pub course_id: String,
    pub learner_id: String,
    pub deadline: String,
    pub question: String,
    pub estimated_usd: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpendDecision { Allow, Reject }

pub fn decide_spend(spent_usd: f64, hard_cap_usd: f64, request_usd: f64) -> SpendDecision {
    if spent_usd + request_usd <= hard_cap_usd { SpendDecision::Allow } else { SpendDecision::Reject }
}

#[derive(Debug)]
pub enum InfraiError { Transport(String), Api(String), Rejected(String) }

#[derive(Debug, Deserialize)]
pub struct Envelope<T> { pub ok: bool, pub data: Option<T>, pub error: Option<serde_json::Value> }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_request_that_would_cross_cap() {
        assert_eq!(decide_spend(9.0, 10.0, 1.01), SpendDecision::Reject);
        assert_eq!(decide_spend(9.0, 10.0, 1.0), SpendDecision::Allow);
    }
}
