use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use qmoosa_pqc::{PqcProvider, PqcSignatureEnvelope};
use qmoosa_x402::{X402BazaarGateway, X402Proof};
use serde_json::json;
use std::sync::Arc;

struct AppState {
    x402_gateway: X402BazaarGateway,
    pqc_provider: PqcProvider,
}

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState {
        x402_gateway: X402BazaarGateway::new(
            "5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd",
            "0.05",
            "DOT",
        ),
        pqc_provider: PqcProvider::new(),
    });

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/alpha-model", get(protected_alpha_model_handler))
        .route("/api/pqc/sign", post(pqc_sign_handler))
        .route("/api/pqc/verify", post(pqc_verify_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("🌌 Qmoosa-RustChain API Service listening on http://0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
}

async fn health_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "status": "online",
            "platform": "Qmoosa-RustChain",
            "architecture": "Polkadot-Native-PolkaVM",
            "pqc": "NIST-FIPS-204-ML-DSA-65",
            "x402": "Bazaar-v2-Native-Rust"
        })),
    )
}

async fn protected_alpha_model_handler(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let payment_proof_hdr = headers.get("X-Payment-Proof");
    let challenge_hdr = headers.get("X-Payment-Challenge");

    if let (Some(proof_val), Some(challenge_val)) = (payment_proof_hdr, challenge_hdr) {
        let proof = X402Proof {
            challenge_id: challenge_val.to_str().unwrap_or("").to_string(),
            tx_hash: proof_val.to_str().unwrap_or("").to_string(),
            payer_address: "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY".to_string(),
        };

        if let Ok(true) = state.x402_gateway.verify_proof(&proof) {
            return (
                StatusCode::OK,
                Json(json!({
                    "status": "unlocked",
                    "resource": "Alpha Intelligence Model weights unlocked via Native Polkadot Settlement",
                    "tx_hash": proof.tx_hash
                })),
            );
        }
    }

    // Otherwise return HTTP 402 Payment Required
    let challenge = state.x402_gateway.generate_challenge(None);
    (
        StatusCode::PAYMENT_REQUIRED,
        Json(json!({
            "status": 402,
            "message": "Payment Required via Polkadot Asset Hub",
            "challenge": challenge
        })),
    )
}

async fn pqc_sign_handler(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let raw = payload.to_string();
    let envelope = state.pqc_provider.sign_payload(&raw, 60000);
    (StatusCode::OK, Json(envelope))
}

#[derive(serde::Deserialize)]
struct VerifyRequest {
    payload: serde_json::Value,
    envelope: PqcSignatureEnvelope,
}

async fn pqc_verify_handler(Json(req): Json<VerifyRequest>) -> impl IntoResponse {
    let raw = req.payload.to_string();
    match PqcProvider::verify_envelope(&raw, &req.envelope, false) {
        Ok(_) => (StatusCode::OK, Json(json!({ "valid": true }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "valid": false, "error": e.to_string() }))),
    }
}
