use super::controllers::admin::api::forms::{get_form_data, list_forms_data};
use super::controllers::admin::html::forms_html::{form_details_page, form_list_page};

use super::controllers::{api as api_controllers, web as web_controllers};
use crate::app_state::AppState;
use axum::{routing::get, routing::post, Router};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/forms-data", get(form_list_page))
        .route("/admin/forms-data/{id}", get(form_details_page))
        .route("/admin/api/list-form-data", get(list_forms_data))
        .route("/admin/api/get-form-data/{id}", get(get_form_data))
        .route("/api/form", post(api_controllers::form::submit_form))
        .route("/{lang}/contact", get(web_controllers::contact::contact))
        .route("/{lang}/iletisim", get(web_controllers::contact::contact))
}
