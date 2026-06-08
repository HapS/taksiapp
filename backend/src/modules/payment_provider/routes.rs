use crate::app_state::AppState;
use crate::modules::payment_provider::controllers::web::{garanti, iyzico, paytr};
use axum::{
    routing::{get, post},
    Router,
};

pub fn payment_provider_routes() -> Router<AppState> {
    Router::new()
        // Garanti Bankası routes
        .route(
            "/payment-provider/garanti/callback/{payment_url}",
            post(garanti::garanti_payment_callback),
        )
        .route(
            "/payment-provider/garanti/success/{payment_url}",
            get(garanti::garanti_payment_success),
        )
        .route(
            "/payment-provider/garanti/failure/{payment_url}",
            get(garanti::garanti_payment_failure),
        )
        // İyzico routes
        .route(
            "/payment-provider/iyzico/callback/{payment_url}",
            post(iyzico::iyzico_payment_callback),
        )
        .route(
            "/payment-provider/iyzico/success/{payment_url}",
            get(iyzico::iyzico_payment_success),
        )
        .route(
            "/payment-provider/iyzico/failure/{payment_url}",
            get(iyzico::iyzico_payment_failure),
        )
        // PayTR routes
        .route(
            "/payment-provider/paytr/callback",
            post(paytr::paytr_callback),
        )
        .route(
            "/payment-provider/paytr/success/{payment_url}",
            get(paytr::paytr_payment_success),
        )
        .route(
            "/payment-provider/paytr/failure/{payment_url}",
            get(paytr::paytr_payment_failure),
        )
}