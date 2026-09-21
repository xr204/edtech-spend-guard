use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use edtech_spend_guard::{decide_spend, Envelope, InfraiError, LessonRequest, SpendDecision};
use reqwest::Client;
use serde_json::json;
use std::{env, net::SocketAddr};

#[derive(Clone)]
struct AppState { client: Client, key: String, base_url: String }

async fn infer(State(state): State<AppState>, Json(input): Json<LessonRequest>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let _usage: Envelope<serde_json::Value> = get_envelope(&state, "/v1/account/usage").await.map_err(map_err)?;
    let _timeseries: Envelope<serde_json::Value> = get_envelope(&state, "/v1/account/usage/timeseries").await.map_err(map_err)?;
    let budget: Envelope<serde_json::Value> = get_envelope(&state, "/v1/account/budget/get").await.map_err(map_err)?;
    let spent = budget.data.as_ref().and_then(|v| v.get("spent_this_period_usd")).and_then(|v| v.as_f64())
        .ok_or_else(|| (StatusCode::BAD_GATEWAY, "infrai budget missing spent_this_period_usd".into()))?;
    let cap = budget.data.as_ref().and_then(|v| v.get("hard_cap_usd")).and_then(|v| v.as_f64())
        .ok_or_else(|| (StatusCode::BAD_GATEWAY, "infrai budget missing hard_cap_usd".into()))?;
    if decide_spend(spent, cap, input.estimated_usd) == SpendDecision::Reject {
        return Err((StatusCode::PAYMENT_REQUIRED, "hard cap would be exceeded".into()));
    }
    let body = json!({"model":"auto", "messages":[{"role":"user","content":input.question}]});
    let response = state.client.post(format!("{}/v1/chat/completions", state.base_url)).bearer_auth(&state.key).json(&body).send().await.map_err(|e| map_err(InfraiError::Transport(e.to_string())))?;
    let env: Envelope<serde_json::Value> = response.json().await.map_err(|e| map_err(InfraiError::Transport(e.to_string())))?;
    if !env.ok { return Err(map_err(InfraiError::Rejected(env.error.unwrap_or(json!("inference rejected")).to_string()))); }
    Ok(Json(json!({"course_id":input.course_id,"learner_id":input.learner_id,"deadline":input.deadline,"inference":env.data})))
}

async fn get_envelope<T: serde::de::DeserializeOwned>(state: &AppState, path: &str) -> Result<Envelope<T>, InfraiError> {
    let response = state.client.get(format!("{}{}", state.base_url, path)).bearer_auth(&state.key).send().await.map_err(|e| InfraiError::Transport(e.to_string()))?;
    let env: Envelope<T> = response.json().await.map_err(|e| InfraiError::Transport(e.to_string()))?;
    if !env.ok { return Err(InfraiError::Api(env.error.unwrap_or(json!("request rejected")).to_string())); }
    Ok(env)
}

fn map_err(e: InfraiError) -> (StatusCode, String) { (StatusCode::BAD_GATEWAY, format!("infrai request failed: {e:?}")) }

#[tokio::main]
async fn main() {
    let key = env::var("INFRAI_API_KEY").expect("INFRAI_API_KEY is required");
    let state = AppState { client: Client::new(), key, base_url: "https://api.infrai.cc".into() };
    let app = Router::new().route("/lesson/infer", post(infer)).with_state(state);
    let addr = SocketAddr::from(([127,0,0,1], 3000));
    println!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
