// SEO Settings Controller
use crate::app_state::AppState;
// use crate::modules::admin::models::settings::SettingsData;
// use crate::modules::admin::services::settings_service;
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect, Response},
};
// use axum_extra::extract::Multipart;
use crate::modules::admin::models::settings::SettingsData;
use crate::modules::admin::services::settings_service;
use sea_orm::EntityTrait;
use sea_orm::QueryOrder;
use tera::Context;
use tower_sessions::Session;
// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;
use crate::modules::ecommerce::models::kargo_sirketleri;
use crate::modules::ecommerce::models::KargoSirketleriEntity;

// kargo şirketlerini listelei db tablo kargo_sirketleri ekleme silme güncelleme işlemleri yapacağız
// kargo şirketlerinin türlü türlü ayarları olduğu için her birinin edit detail sayfası farklı olacağından her birinin edit html sayfası farklı olacak
// bu yüzden kargo şirketleri için ayrı bir controller ve service yazacağız, burası sadece listeleme, ekleme silme işlemleri yapacak, edit işlemi için ayrı bir controller yazacağız

pub async fn kargo_sirketleri_list(State(state): State<AppState>, session: Session) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let current_settings = match settings_service::get_settings(&state.db).await {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("Settings error: {:?}", e);
            SettingsData::default()
        }
    };

    let config = crate::config::get_config();
    let supported_languages = &config.supported_languages;
    println!("Support Languages {:?}", supported_languages);

    let mut context = Context::new();
    context.insert(
        "supported_languages_yalandan_derleyici_kudurmasin_diye",
        supported_languages,
    );

    // Fetch kargo şirketleri from database
    let kargo_sirketleri_list = match KargoSirketleriEntity::find()
        .order_by_asc(kargo_sirketleri::Column::Title)
        .all(&state.db)
        .await
    {
        Ok(list) => list,
        Err(err) => {
            eprintln!("Error fetching kargo şirketleri: {}", err);
            vec![]
        }
    };

    println!("kargo şirketleri : {:?}", kargo_sirketleri_list);
    context.insert("kargo_sirketleri_list", &kargo_sirketleri_list);
    context.insert(
        "free_shipping_threshold",
        &current_settings.free_shipping_threshold,
    );
    context.insert(
        "default_currency",
        &current_settings
            .default_currency
            .clone()
            .unwrap_or_else(|| "TRY".to_string()),
    );

    match super::render_settings_page(
        &state,
        "kargo",
        "Kargo Şirketleri",
        "admin/kargo/list.tera.html",
        context,
        None,
    )
    .await
    {
        Ok(html) => html.into_response(),
        Err(response) => response,
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct KargoSirketiResponse {
    pub id: i32,
    pub data: serde_json::Value,
    pub title: String,
    pub logo: String,
    pub publish: Option<bool>,
    pub default: Option<bool>,
    pub template: String,
}

pub async fn kargo_sirketi_settings(
    State(state): State<AppState>,
    session: Session,
    Path(kargo_sirketi_id): Path<i32>,
) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let config = crate::config::get_config();
    let supported_languages = &config.supported_languages;
    println!("Support Languages {:?}", supported_languages);

    let mut context = Context::new();
    context.insert(
        "supported_languages_yalandan_derleyici_kudurmasin_diye",
        supported_languages,
    );

    // Fetch kargo şirketleri from database
    let kargo_sirketi = match KargoSirketleriEntity::find_by_id(kargo_sirketi_id)
        .all(&state.db)
        .await
    {
        Ok(list) => list,
        Err(err) => {
            eprintln!("Error fetching kargo şirketleri: {}", err);
            vec![]
        }
    };

    let template = if kargo_sirketi.is_empty() {
        "default.html".to_string()
    } else {
        kargo_sirketi[0].template.clone()
    };

    context.insert("kargo_sirketi", &kargo_sirketi);
    context.insert("kargo_sirketi_id", &kargo_sirketi_id);

    let sirket_response = KargoSirketiResponse {
        id: kargo_sirketi_id,
        data: if kargo_sirketi.is_empty() {
            serde_json::json!({})
        } else {
            kargo_sirketi[0].data.clone()
        },
        title: if kargo_sirketi.is_empty() {
            "Yeni Kargo Şirketi".to_string()
        } else {
            kargo_sirketi[0].title.clone()
        },
        logo: if kargo_sirketi.is_empty() {
            "".to_string()
        } else {
            kargo_sirketi[0].logo.clone()
        },
        publish: if kargo_sirketi.is_empty() {
            None
        } else {
            Some(kargo_sirketi[0].publish)
        },
        default: if kargo_sirketi.is_empty() {
            None
        } else {
            Some(kargo_sirketi[0].default)
        },
        template: template.clone(),
    };

    context.insert(
        "sirket",
        serde_json::to_value(&sirket_response)
            .unwrap()
            .as_object()
            .unwrap(),
    );

    match super::render_settings_page(
        &state,
        "kargo",
        &format!("{}", kargo_sirketi[0].title),
        &format!("admin/kargo/{}", sirket_response.template),
        context,
        None,
    )
    .await
    {
        Ok(html) => html.into_response(),
        Err(response) => response,
    }
}

use axum::extract::Json;
use axum::http::StatusCode;
use sea_orm::{ActiveModelTrait, Set};

pub async fn kargo_sirketi_settings_post(
    State(state): State<AppState>,
    session: Session,
    Path(kargo_sirketi_id): Path<i32>,
    Json(data): Json<serde_json::Value>,
) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    println!("POST JSON data: {:?}", data);

    // Fetch kargo şirketi from database
    let kargo_sirketi = match KargoSirketleriEntity::find_by_id(kargo_sirketi_id)
        .one(&state.db)
        .await
    {
        Ok(Some(sirket)) => sirket,
        Ok(None) => {
            eprintln!("Kargo şirketi bulunamadı: {}", kargo_sirketi_id);
            return (StatusCode::NOT_FOUND, "Kargo şirketi bulunamadı").into_response();
        }
        Err(err) => {
            eprintln!("Error fetching kargo şirketi: {}", err);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // Update data field
    let mut active_model: crate::modules::ecommerce::models::kargo_sirketleri::ActiveModel =
        kargo_sirketi.into();
    active_model.data = Set(data);

    // Save to database
    match active_model.update(&state.db).await {
        Ok(_) => {
            println!("Kargo şirketi ayarları güncellendi: {}", kargo_sirketi_id);
            (StatusCode::OK, "Ayarlar başarıyla kaydedildi").into_response()
        }
        Err(err) => {
            eprintln!("Error updating kargo şirketi: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Ayarlar kaydedilirken hata oluştu",
            )
                .into_response()
        }
    }
}

// minimum sepet tutarı vs. gibi ek ayarlar yani ortak ayarlar buradan kaydedilir
// nereden okunur? settings.free_shipping_threshold  diye okunur

pub async fn cargo_extra_settings_post(
    State(state): State<AppState>,
    session: Session,
    Json(data): Json<serde_json::Value>,
) -> impl IntoResponse {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let extra_settings = data
        .get("extra_settings")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    match settings_service::save_extra_settings(&state.db, &extra_settings).await {
        Ok(_) => (StatusCode::OK, "Ayarlar başarıyla kaydedildi").into_response(),
        Err(err) => {
            eprintln!("Save extra settings error: {:?}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Ayarlar kaydedilirken hata oluştu",
            )
                .into_response()
        }
    }
}
