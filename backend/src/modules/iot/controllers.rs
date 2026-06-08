use axum::extract::Json;

pub async fn esp32c6(Json(payload): Json<serde_json::Value>) -> Json<serde_json::Value> {
    println!("Post Data : {}", payload);
    Json(serde_json::json!({
        "status": "ok",
        "made_in": "teo",
        "received": payload
    }))
}

pub async fn iot_config() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "config": {
            "device": "esp32c6",
            "version": "1.0.0"
        }
    }))
}
