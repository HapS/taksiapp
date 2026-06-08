// Web Controllers - Frontend HTML pages
use crate::app_state::AppState;
use crate::config;
// use crate::modules::content::services::page_service;
use axum::{
    extract::{
        Path,
        // Request,
        State,
    },
    response::{Html, IntoResponse, Redirect},
    Extension,
};

// use migration::query;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
// use tera::Context;
// use tower_sessions::Session;
use crate::middleware::global_context::{GlobalContext, ViewContext};
use crate::modules::content::helpers::language_helper::{validate_language, LanguageValidation};
use crate::modules::content::models::Content;
use rust_i18n::t;

use crate::modules::admin::controllers::web::homepage;

/// Home page - shows recent published pages
pub async fn index(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    mut ctx: ViewContext,
    Path(language): Path<String>,
    uri: axum::http::Uri,
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
    // request: Request,
) -> impl IntoResponse {
    // let clang = current_language!(request);
    // println!("MAkRO Current Language {}", clang);

    let config = config::get_config();

    // Validate the language parameter
    let lang = match validate_language(&language, &config) {
        LanguageValidation::Valid(lang) => lang,
        LanguageValidation::ReservedPath => {
            // This is a reserved path (api, admin, etc.) - return 404 to let other routes handle
            return axum::http::StatusCode::NOT_FOUND.into_response();
        }
        LanguageValidation::Unsupported { redirect_to } => {
            // Language format is valid but not supported - redirect to default
            return Redirect::to(&redirect_to).into_response();
        }
    };

    ctx.0.insert("title", &t!("home", locale = lang));

    // Settings'ten default home content ID'sini al
    let default_home_content_id = {
        if let Ok(settings_cache) = state.settings_cache.read() {
            settings_cache.get_default_home_content_id()
        } else {
            70 // Fallback
        }
    };

    let mut index_page = match Content::find()
        .filter(crate::modules::content::models::content::Column::Id.eq(default_home_content_id))
        .one(&state.db)
        .await
    {
        Ok(Some(data)) => {
            if config.is_debug() {
                tracing::debug!("Front Page bulundu: ID {}", default_home_content_id);
            }
            Some(data)
        }
        Ok(None) => {
            tracing::warn!("Front Page bulunamadı: ID {}", default_home_content_id);
            None
        }
        Err(err) => {
            tracing::error!("Front Page DB hatası: {:?}", err);
            None
        }
    };

    // Eğer ana sayfa içeriği ürün gösteriyorsa, ürün verisini kaldır
    // Bu helper fonksiyon yapılaiblir ama elim alışsınsın diye buraya aldım
    if let Some(ref mut page) = index_page {
        if let Some(obj) = page.data.as_object_mut() {
            obj.remove("product");
            obj.remove("form_settings");
            obj.remove("featured_image");
        }
    }

    // Index page için sub content'leri yükle
    let sub_contents = if let Some(ref page) = index_page {
        crate::modules::content::helpers::sub_content_helper::load_sub_contents(
            &state.db,
            page.id,
            Some(&lang),
        )
        .await
        .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    // Kullanıcının seçtiği display_currency'yi global context'ten al
    let display_currency = global_ctx.display_currency.clone();

    //bu admin modülü içinde direkt oradan front home a getiriyoruz
    let homepage_render =
        match homepage::get_homepage_render_cached(&state, &lang, Some(&display_currency)).await {
            Ok(render_data) => render_data,
            Err(e) => {
                tracing::error!("Homepage render hatası: {:?}", e);
                homepage::HomepageRenderResponse {
                    sections: vec![],
                    language: lang.clone(),
                    total_sections: 0,
                }
            }
        };

    // Template'e ver
    ctx.0.insert("builder", &homepage_render);

    ctx.0.insert("page", &index_page); //admin panelde settings de  ana sayfa default content id var
    ctx.0.insert("sub_contents", &sub_contents); //page e iliştirilmiş sub contentler

    ctx.0.insert("request_path", uri.path());

    //print session data
    // println!("Session Data: {:?}", session);

    // ?json=true  Json response  tera ile aynı çıktığı verir  context i doğrudan json yapıyoruz
    if query.get("json").map(|v| v == "true").unwrap_or(false) {
        // Context'i serialize edilebilir bir map'e çevir (BTreeMap kullanarak alfabetik sıralama sağlıyoruz)
        let json_data: std::collections::BTreeMap<String, serde_json::Value> = ctx
            .0
            .into_json()
            .as_object()
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        return axum::Json(json_data).into_response();
    }

    match state.render_frontend_template("home/index.html", &ctx.0) {
        Ok(html) => {
            // Cache-control header ekle - browser cache'ini devre dışı bırak
            // let response = Html(html).into_response();
            let mut response = Html(html).into_response();
            response.headers_mut().insert(
                axum::http::header::CACHE_CONTROL,
                axum::http::HeaderValue::from_static("no-cache, no-store, must-revalidate"),
            );
            response.headers_mut().insert(
                axum::http::header::PRAGMA,
                axum::http::HeaderValue::from_static("no-cache"),
            );
            response.headers_mut().insert(
                axum::http::header::EXPIRES,
                axum::http::HeaderValue::from_static("0"),
            );
            response
        }
        Err(e) => {
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}
