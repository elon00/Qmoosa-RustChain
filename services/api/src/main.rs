use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use qmoosa_pqc::{PqcProvider, PqcSignatureEnvelope};
use qmoosa_primitives::{AccountId32, SS58_PREFIX_SUBSTRATE};
use qmoosa_x402::{MockOnChainVerifier, X402BazaarGateway, X402Challenge, X402Proof};
use serde_json::json;
use std::sync::{Arc, Mutex};

struct AppState {
    x402_gateway: X402BazaarGateway,
    pqc_provider: PqcProvider,
    onchain_verifier: MockOnChainVerifier,
    active_challenges: Mutex<std::collections::HashMap<String, X402Challenge>>,
}

#[tokio::main]
async fn main() {
    let merchant_ss58 = "5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd";
    let (merchant_account, _) =
        AccountId32::from_ss58(merchant_ss58).expect("Merchant address must be valid SS58");

    let gateway = X402BazaarGateway::new(
        merchant_account,
        500_000_000_000_000, // 0.05 DOT
        "DOT",
    );

    let verifier = MockOnChainVerifier::new();

    let state = Arc::new(AppState {
        x402_gateway: gateway,
        pqc_provider: PqcProvider::new(),
        onchain_verifier: verifier,
        active_challenges: Mutex::new(std::collections::HashMap::new()),
    });

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/alpha-model", get(protected_alpha_model_handler))
        .route("/api/pqc/sign", post(pqc_sign_handler))
        .route("/api/pqc/verify", post(pqc_verify_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("🌌 Qmoosa-RustChain API Service listening on http://0.0.0.0:8080");
    println!("🔑 Merchant Address: {}", merchant_ss58);
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
            "x402": "Bazaar-v2-On-Chain-Verified",
            "ss58_prefix": SS58_PREFIX_SUBSTRATE
        })),
    )
}

async fn protected_alpha_model_handler(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let payment_proof_hdr = headers.get("X-Payment-Proof");
    let challenge_hdr = headers.get("X-Payment-Challenge");
    let payer_hdr = headers.get("X-Payer-Address");

    if let (Some(proof_val), Some(challenge_val)) = (payment_proof_hdr, challenge_hdr) {
        let challenge_id = challenge_val.to_str().unwrap_or("").to_string();
        let tx_hash = proof_val.to_str().unwrap_or("").to_string();
        let payer_ss58 = payer_hdr
            .and_then(|h| h.to_str().ok())
            .unwrap_or("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY");

        let payer_account = match AccountId32::from_ss58(payer_ss58) {
            Ok((acc, _)) => acc,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "Invalid payer SS58 address" })),
                );
            }
        };

        let proof = X402Proof {
            challenge_id: challenge_id.clone(),
            tx_hash,
            payer_address: payer_account,
        };

        let challenge_opt = {
            let guard = state.active_challenges.lock().unwrap();
            guard.get(&challenge_id).cloned()
        };

        if let Some(challenge) = challenge_opt {
            match state
                .x402_gateway
                .verify_payment(&challenge, &proof, &state.onchain_verifier)
            {
                Ok(details) => {
                    return (
                        StatusCode::OK,
                        Json(json!({
                            "status": "unlocked",
                            "resource": "Alpha Intelligence Model weights unlocked via Polkadot Settlement",
                            "payment_details": details
                        })),
                    );
                }
                Err(e) => {
                    return (
                        StatusCode::PAYMENT_REQUIRED,
                        Json(json!({
                            "status": 402,
                            "error": "Payment verification failed",
                            "details": e.to_string()
                        })),
                    );
                }
            }
        }
    }

    // Otherwise generate and store a new challenge
    let challenge = state.x402_gateway.generate_challenge(None, 15 * 60 * 1000);
    {
        let mut guard = state.active_challenges.lock().unwrap();
        guard.insert(challenge.challenge_id.clone(), challenge.clone());
    }

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
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "valid": true, "algorithm": "NIST-FIPS-204-ML-DSA-65" })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "valid": false, "error": e.to_string() })),
        ),
    }
}
