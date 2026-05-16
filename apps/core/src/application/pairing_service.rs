use std::sync::Arc;

use serde_json::json;
use tracing::{info, warn};

use crate::application::state::AppState;

/// Register this unpaired Core in Convex so a remote dashboard/app can claim its code.
pub async fn register_pairing_core(state: Arc<AppState>) {
    let Some(convex_url) = state.config.convex_url.as_deref() else {
        return;
    };
    let code = state.pairing_code.lock().await.clone();
    let Some(code) = code else {
        return;
    };
    let core_id = state.core_id.clone();
    let public_url = state.config.public_url.clone();
    let bind_host = state.config.bind_host.clone();
    let port = state.config.port;

    let endpoint = format!("{}/api/mutation", convex_url.trim_end_matches('/'));
    let mut args = json!({
        "code": code,
        "coreId": core_id,
        "metadata": {
            "bindHost": bind_host,
            "port": port,
        },
    });
    if let Some(public_url) = public_url {
        args["connectionUrl"] = json!(public_url);
    }
    let body = json!({
        "path": "presence:registerPairingCore",
        "format": "json",
        "args": args,
    });

    match state.http.post(endpoint).json(&body).send().await {
        Ok(response) if response.status().is_success() => {
            let body: serde_json::Value =
                response.json().await.unwrap_or_else(|_| json!({}));
            if body.get("status").and_then(|value| value.as_str())
                == Some("success")
            {
                info!(%core_id, "registered Core pairing code with Convex");
            } else {
                warn!(response = %body, "Convex rejected Core pairing registration");
            }
        }
        Ok(response) => {
            warn!(status = %response.status(), "failed to register Core pairing code with Convex");
        }
        Err(error) => {
            warn!(%error, "failed to reach Convex for Core pairing registration");
        }
    }
}
