use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use sea_orm::*;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::app_state::AppState;
use crate::modules::auth::helpers::rbac::check_admin_access_api;
use crate::modules::content::models::{content::Column as ContentColumn, Content};
use crate::modules::taxonomy::models::term::Entity as Term;

#[derive(Deserialize)]
pub struct SearchQueryParams {
    pub q: String,
    pub module: Option<String>, // content | term | all
    pub lang: Option<String>,
    pub content_type: Option<String>,
}

/// URL oluşturulurken kullanılan kelime dağarcığı (vocabulary) ID'leri için ayarlar
struct VocabularySettings {
    product_categories: i64,
    blog_categories: i64,
    news_categories: i64,
    page_categories: i64,
    tags_categories: i64,
}

pub async fn search_absolute_url(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
    Query(params): Query<SearchQueryParams>,
) -> Response {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    // 1. Dilleri Hazırla
    // Konfigürasyondan desteklenen dilleri topla
    let supported_languages = state
        .config
        .supported_languages
        .keys()
        .cloned()
        .collect::<Vec<String>>();

    // İstenen dilleri belirle. Eğer lang=all ise, desteklenen tüm dillerde ara; aksi takdirde istenen veya varsayılan dili kullan
    let requested_lang_opt = params.lang.clone();
    let search_langs: Vec<String> = if requested_lang_opt.as_deref() == Some("all") {
        supported_languages.clone()
    } else {
        vec![requested_lang_opt.unwrap_or_else(|| state.config.default_language.clone())]
    };

    // 2. Ayarları Hazırla
    let current_settings =
        match crate::modules::admin::services::settings_service::get_settings(&state.db).await {
            Ok(settings) => settings,
            Err(e) => {
                eprintln!("Settings error: {:?}", e);
                crate::modules::admin::models::settings::SettingsData::default()
            }
        };

    let vocab_settings = VocabularySettings {
        product_categories: current_settings.vocab_product_categories.unwrap_or(1),
        blog_categories: current_settings.vocab_blog_categories.unwrap_or(5),
        news_categories: current_settings.vocab_news_categories.unwrap_or(6),
        page_categories: current_settings.vocab_page_categories.unwrap_or(3),
        tags_categories: current_settings.vocab_tags_categories.unwrap_or(4),
    };

    let module_filter = params.module.clone().unwrap_or_else(|| "all".to_string());
    let mut results: Vec<Value> = Vec::new();

    // 3. İçerikleri Ara (eğer istenmişse)
    if module_filter == "all" || module_filter == "content" {
        let content_results =
            search_contents(&state, &params, &search_langs, &supported_languages).await;
        results.extend(content_results);
    }

    // 4. Terimleri Ara (eğer istenmişse)
    if module_filter == "all" || module_filter == "term" {
        let term_results = search_terms(
            &state,
            &params,
            &search_langs,
            &supported_languages,
            &vocab_settings,
        )
        .await;
        results.extend(term_results);
    }

    (StatusCode::OK, Json(json!({ "results": results }))).into_response()
}

/// İçerikleri aramak için yardımcı fonksiyon
async fn search_contents(
    state: &AppState,
    params: &SearchQueryParams,
    search_langs: &[String],
    supported_languages: &[String],
) -> Vec<Value> {
    use sea_orm::sea_query::Expr as SeaExpr;

    let like_query = format!("%{}%", params.q.to_lowercase());

    let mut select = Content::find()
        .filter(ContentColumn::Publish.eq(true))
        .filter(ContentColumn::DeletedAt.is_null());

    // content_type filtresi
    if let Some(ct) = &params.content_type {
        if !ct.is_empty() && ct != "all" {
            select = select.filter(ContentColumn::ContentType.eq(ct.clone()));
        }
    }

    // Arama koşulunu oluştur
    let mut cond = Condition::any();
    for s_lang in search_langs {
        cond = cond.add(SeaExpr::cust_with_values(
            "lower(data->'langs'->$1->>'title') LIKE $2",
            vec![
                sea_orm::Value::from(s_lang.clone()),
                sea_orm::Value::from(like_query.clone()),
            ],
        ));
        cond = cond.add(SeaExpr::cust_with_values(
            "lower(data->'langs'->$1->>'description') LIKE $2",
            vec![
                sea_orm::Value::from(s_lang.clone()),
                sea_orm::Value::from(like_query.clone()),
            ],
        ));
    }
    // Yedek (Fallback): ham JSON metni içinde ara
    cond = cond.add(SeaExpr::cust_with_values(
        "lower(data::text) LIKE $1",
        vec![sea_orm::Value::from(like_query.clone())],
    ));

    select = select.filter(cond);

    match select
        .order_by_desc(ContentColumn::CreatedAt)
        .limit(20)
        .all(&state.db)
        .await
    {
        Ok(contents) => contents
            .into_iter()
            .map(|c| {
                // Ana başlık/url gösterimi için ilk arama dilini birincil olarak kullan
                let primary_lang = &search_langs[0];

                let title = get_json_string(&c.data, primary_lang, "title");
                let description = get_json_string(&c.data, primary_lang, "short_description")
                    .or_else(|| get_json_string(&c.data, primary_lang, "description"));
                let url = c.get_absolute_url(primary_lang);

                // Tüm diller için haritaları (maps) oluştur
                let mut titles_map = serde_json::Map::new();
                let mut descriptions_map = serde_json::Map::new();
                let mut urls_map = serde_json::Map::new();

                for lang in supported_languages {
                    titles_map.insert(lang.clone(), json!(get_json_string(&c.data, lang, "title")));

                    let desc_val = get_json_string(&c.data, lang, "short_description")
                        .or_else(|| get_json_string(&c.data, lang, "description"));
                    descriptions_map.insert(lang.clone(), json!(desc_val));

                    let abs_url = c.get_absolute_url(lang).map(|s| s.to_string());
                    urls_map.insert(lang.clone(), json!(abs_url));
                }

                json!({
                    "id": c.id,
                    "title": title.unwrap_or_default(),
                    "titles": titles_map,
                    "description": description,
                    "descriptions": descriptions_map,
                    "url": url,
                    "content_type": c.content_type,
                    "absolute_urls": urls_map
                })
            })
            .collect(),
        Err(e) => {
            eprintln!("Content search error: {}", e);
            Vec::new()
        }
    }
}

/// Terimleri aramak için yardımcı fonksiyon
async fn search_terms(
    state: &AppState,
    params: &SearchQueryParams,
    search_langs: &[String],
    supported_languages: &[String],
    vocab_settings: &VocabularySettings,
) -> Vec<Value> {
    use crate::modules::taxonomy::helpers::term_helper::TermExtensions;
    use sea_orm::sea_query::Expr as SeaExpr;

    let like_query = format!("%{}%", params.q.to_lowercase());

    let mut cond = Condition::any();
    for s_lang in search_langs {
        // Başlık, açıklama ve kısa açıklama içinde ara
        let fields = ["title", "description", "short_description"];
        for field in fields {
            cond = cond.add(SeaExpr::cust_with_values(
                format!("lower(data->'langs'->$1->>'{}') LIKE $2", field).as_str(),
                vec![
                    sea_orm::Value::from(s_lang.clone()),
                    sea_orm::Value::from(like_query.clone()),
                ],
            ));
        }
    }
    // Yedek ham arama
    cond = cond.add(SeaExpr::cust_with_values(
        "lower(data::text) LIKE $1",
        vec![sea_orm::Value::from(like_query.clone())],
    ));

    match Term::find().filter(cond).limit(20).all(&state.db).await {
        Ok(terms) => terms
            .into_iter()
            .map(|t| {
                let primary_lang = &search_langs[0];

                let title = t.get_title(primary_lang);
                let description = t.get_description(primary_lang);
                let slug = t.get_slug(primary_lang);

                let url = if let Some(s) = slug {
                    Some(build_term_url(
                        primary_lang,
                        &s,
                        t.id,
                        t.vocabulary_id,
                        vocab_settings,
                    ))
                } else {
                    None
                };

                let mut titles_map = serde_json::Map::new();
                let mut descriptions_map = serde_json::Map::new();
                let mut urls_map = serde_json::Map::new();

                for lang in supported_languages {
                    titles_map.insert(lang.clone(), json!(non_empty(t.get_title(lang))));
                    descriptions_map.insert(
                        lang.clone(),
                        json!(t.get_description(lang).and_then(non_empty)),
                    );

                    let built_url = t
                        .get_slug(lang)
                        .map(|s| build_term_url(lang, &s, t.id, t.vocabulary_id, vocab_settings));
                    urls_map.insert(lang.clone(), json!(built_url));
                }

                json!({
                    "id": t.id,
                    "title": title,
                    "titles": titles_map,
                    "description": description,
                    "descriptions": descriptions_map,
                    "url": url,
                    "content_type": "term",
                    "vocabulary_id": t.vocabulary_id,
                    "absolute_urls": urls_map
                })
            })
            .collect(),
        Err(e) => {
            eprintln!("Term search error: {}", e);
            Vec::new()
        }
    }
}

/// Vocabulary ID'sine dayalı olarak Terim URL'lerini oluşturmak için merkezi mantık
fn build_term_url(
    lang: &str,
    slug: &str,
    id: i64,
    vocab_id: i64,
    settings: &VocabularySettings,
) -> String {
    // Merkezi URL oluşturma mantığı.
    // İleride rota yapısı değişirse sadece burayı güncellemek yeterli olacaktır.
    if vocab_id == settings.product_categories {
        format!("/{}/products/category/{}-{}", lang, slug, id)
    } else if vocab_id == settings.blog_categories {
        format!("/{}/blog/category/{}-{}", lang, slug, id)
    } else if vocab_id == settings.news_categories {
        format!("/{}/news/category/{}-{}", lang, slug, id)
    } else if vocab_id == settings.page_categories {
        format!("/{}/page/category/{}-{}", lang, slug, id)
    } else if vocab_id == settings.tags_categories {
        format!("/{}/tag/{}-{}", lang, slug, id)
    } else {
        // Diğer sınıflandırmalar (taxonomies) için yedek
        format!("/{}/{}-{}", lang, slug, id)
    }
}

/// Bir JSON Value yapısından boş olmayan bir dizeyi güvenli bir şekilde çıkarmak için yardımcı
fn get_json_string(data: &serde_json::Value, lang: &str, key: &str) -> Option<String> {
    data.get("langs")
        .and_then(|l| l.get(lang))
        .and_then(|d| d.get(key))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
}

/// Boş dizeleri None değerine dönüştürmek için yardımcı
fn non_empty(s: String) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s)
    }
}
