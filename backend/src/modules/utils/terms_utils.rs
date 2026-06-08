use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Clone)]
pub struct NestedTerm {
    pub id: i64,
    pub publish: bool,

    // Backward compatibility - tek dilli erişim (deprecated)
    pub title: String,
    pub slug: Option<String>,
    pub description: Option<String>,

    // Yeni çok dilli erişim
    pub titles: std::collections::HashMap<String, String>,
    pub slugs: std::collections::HashMap<String, String>,
    pub descriptions: std::collections::HashMap<String, String>,

    pub term_icon: Option<String>,
    pub data: serde_json::Value,
    pub children: Vec<NestedTerm>,
    pub order_id: Option<i32>,
    pub vocabulary_id: i64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

pub fn build_term_hierarchy(
    terms: &[crate::modules::taxonomy::models::term::Model],
    lang: &str,
    parent_id: Option<i64>,
) -> Vec<NestedTerm> {
    use crate::modules::taxonomy::helpers::term_helper::TermExtensions;

    if terms.is_empty() {
        return Vec::new();
    }

    // Önce tüm term'leri map'e ekle - TermExtensions kullanarak doğru title'ları al
    let mut term_map: HashMap<i64, NestedTerm> = HashMap::new();

    for term in terms {
        // TermExtensions trait'i ile doğru title ve description'ı al
        let title = term.get_title(lang);
        let description = term.get_description(lang);
        let slug = term.get_slug(lang);

        // Tek dilli için sadece mevcut dili kullan
        let mut titles = std::collections::HashMap::new();
        let mut slugs = std::collections::HashMap::new();
        let mut descriptions = std::collections::HashMap::new();

        titles.insert(lang.to_string(), title.clone());
        if let Some(slug_val) = &slug {
            slugs.insert(lang.to_string(), slug_val.clone());
        }
        if let Some(desc_val) = &description {
            descriptions.insert(lang.to_string(), desc_val.clone());
        }

        term_map.insert(
            term.id,
            NestedTerm {
                id: term.id,
                publish: term.publish,
                // Backward compatibility fields
                title,
                slug,
                description,
                // New multilingual fields
                titles,
                slugs,
                descriptions,
                term_icon: term.get_term_icon(),
                data: term.data.clone(),
                children: Vec::new(),
                order_id: term.order_id,
                vocabulary_id: term.vocabulary_id,
                created_at: term.created_at.map(|dt| dt.to_string()),
                updated_at: term.updated_at.map(|dt| dt.to_string()),
            },
        );
    }

    // Parent-child ilişkilerini kur (bottom-up yaklaşım)
    // Önce en derin seviyeden başlayarak yukarı çık
    let mut children_map: HashMap<i64, Vec<i64>> = HashMap::new();

    for term in terms {
        if let Some(parent_id) = term.parent_id {
            children_map
                .entry(parent_id)
                .or_insert_with(Vec::new)
                .push(term.id);
        }
    }

    // Recursive olarak children'ları doldur
    fn build_children(
        term_id: i64,
        term_map: &HashMap<i64, NestedTerm>,
        children_map: &HashMap<i64, Vec<i64>>,
    ) -> NestedTerm {
        let mut term = term_map.get(&term_id).unwrap().clone();

        if let Some(child_ids) = children_map.get(&term_id) {
            term.children = child_ids
                .iter()
                .map(|child_id| build_children(*child_id, term_map, children_map))
                .collect();
        }

        term
    }

    // Root veya parent_id'ye göre term'leri bul ve recursive olarak oluştur
    let mut root_terms: Vec<&_> = match parent_id {
        Some(pid) => terms
            .iter()
            .filter(|term| term.parent_id == Some(pid))
            .collect(),
        None => terms
            .iter()
            .filter(|term| term.parent_id.is_none())
            .collect(),
    };

    // Eğer parent_id ile çekilenler boşsa ve parent_id varsa, bir üst parent'a çık
    if root_terms.is_empty() {
        if let Some(pid) = parent_id {
            if let Some(parent_term) = terms.iter().find(|term| term.id == pid) {
                let upper_parent_id = parent_term.parent_id;
                root_terms = match upper_parent_id {
                    Some(upid) => terms
                        .iter()
                        .filter(|term| term.parent_id == Some(upid))
                        .collect(),
                    None => terms
                        .iter()
                        .filter(|term| term.parent_id.is_none())
                        .collect(),
                };
            }
        }
    }

    let mut result: Vec<NestedTerm> = root_terms
        .into_iter()
        .map(|term| build_children(term.id, &term_map, &children_map))
        .collect();

    // Eğer parent_id verilmişse ve childlar boş değilse, geri elemanı ekle
    if let Some(pid) = parent_id {
        if !result.is_empty() {
            if let Some(parent_term) = terms.iter().find(|term| term.id == pid) {
                let upper_parent_id = parent_term.parent_id;
                let back_title = if let Some(upid) = upper_parent_id {
                    if let Some(upper_parent) = terms.iter().find(|term| term.id == upid) {
                        format!("⬅ Geri ({})", upper_parent.get_title(lang))
                    } else {
                        "⬅ Geri".to_string()
                    }
                } else {
                    "⬅ Geri (Tüm Ürünler)".to_string()
                };
                let (slug, description) = if let Some(upid) = upper_parent_id {
                    if let Some(upper_parent) = terms.iter().find(|term| term.id == upid) {
                        (
                            upper_parent.get_slug(lang),
                            upper_parent.get_description(lang),
                        )
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };

                // Geri butonu için multilingual fields
                let mut titles = std::collections::HashMap::new();
                let mut slugs = std::collections::HashMap::new();
                let mut descriptions = std::collections::HashMap::new();

                titles.insert(lang.to_string(), back_title.clone());
                if let Some(slug_val) = &slug {
                    slugs.insert(lang.to_string(), slug_val.clone());
                }
                if let Some(desc_val) = &description {
                    descriptions.insert(lang.to_string(), desc_val.clone());
                }

                result.insert(
                    0,
                    NestedTerm {
                        id: upper_parent_id.unwrap_or(0),
                        publish: true,
                        // Backward compatibility fields
                        title: back_title,
                        slug,
                        description,
                        // New multilingual fields
                        titles,
                        slugs,
                        descriptions,
                        term_icon: None, // Geri butonu için icon yok
                        data: serde_json::Value::Object(serde_json::Map::new()),
                        children: Vec::new(),
                        order_id: None,
                        vocabulary_id: parent_term.vocabulary_id,
                        created_at: None,
                        updated_at: None,
                    },
                );
            }
        }
    }

    result
}

pub fn build_multilingual_term_hierarchy(
    terms: &[crate::modules::taxonomy::models::term::Model],
    languages: &[String],
    parent_id: Option<i64>,
) -> Vec<NestedTerm> {
    use crate::modules::taxonomy::helpers::term_helper::TermExtensions;

    if terms.is_empty() {
        return Vec::new();
    }

    // Önce tüm term'leri map'e ekle - her dil için title/slug/description al
    let mut term_map: std::collections::HashMap<i64, NestedTerm> = std::collections::HashMap::new();

    for term in terms {
        let mut titles = std::collections::HashMap::new();
        let mut slugs = std::collections::HashMap::new();
        let mut descriptions = std::collections::HashMap::new();

        // Her dil için title, slug, description al
        for lang in languages {
            titles.insert(lang.clone(), term.get_title(lang));
            if let Some(slug) = term.get_slug(lang) {
                slugs.insert(lang.clone(), slug);
            }
            if let Some(desc) = term.get_description(lang) {
                descriptions.insert(lang.clone(), desc);
            }
        }

        // Backward compatibility için ilk dili kullan
        let default_lang = "tr".to_string();
        let first_lang = languages.first().unwrap_or(&default_lang);
        let title = term.get_title(first_lang);
        let slug = term.get_slug(first_lang);
        let description = term.get_description(first_lang);

        term_map.insert(
            term.id,
            NestedTerm {
                id: term.id,
                publish: term.publish,
                // Backward compatibility fields
                title,
                slug,
                description,
                // New multilingual fields
                titles,
                slugs,
                descriptions,
                term_icon: term.get_term_icon(),
                data: term.data.clone(),
                children: Vec::new(),
                order_id: term.order_id,
                vocabulary_id: term.vocabulary_id,
                created_at: term.created_at.map(|dt| dt.to_string()),
                updated_at: term.updated_at.map(|dt| dt.to_string()),
            },
        );
    }

    // Parent-child ilişkilerini kur
    let mut children_map: std::collections::HashMap<i64, Vec<i64>> =
        std::collections::HashMap::new();

    for term in terms {
        if let Some(parent_id) = term.parent_id {
            children_map
                .entry(parent_id)
                .or_insert_with(Vec::new)
                .push(term.id);
        }
    }

    // Recursive olarak children'ları doldur
    fn build_multilingual_children(
        term_id: i64,
        term_map: &std::collections::HashMap<i64, NestedTerm>,
        children_map: &std::collections::HashMap<i64, Vec<i64>>,
    ) -> NestedTerm {
        let mut term = term_map.get(&term_id).unwrap().clone();

        if let Some(child_ids) = children_map.get(&term_id) {
            // Children'ları order_id'ye göre sırala
            let mut sorted_child_ids = child_ids.clone();
            sorted_child_ids.sort_by(|a, b| {
                let order_a = term_map.get(a).and_then(|t| t.order_id).unwrap_or(999999);
                let order_b = term_map.get(b).and_then(|t| t.order_id).unwrap_or(999999);
                order_a.cmp(&order_b)
            });

            term.children = sorted_child_ids
                .iter()
                .map(|child_id| build_multilingual_children(*child_id, term_map, children_map))
                .collect();
        }

        term
    }

    // Root term'leri bul ve recursive olarak oluştur
    let mut root_terms: Vec<&_> = match parent_id {
        Some(pid) => terms
            .iter()
            .filter(|term| term.parent_id == Some(pid))
            .collect(),
        None => terms
            .iter()
            .filter(|term| term.parent_id.is_none())
            .collect(),
    };

    // Order_id'ye göre sırala
    root_terms.sort_by(|a, b| {
        let order_a = a.order_id.unwrap_or(999999);
        let order_b = b.order_id.unwrap_or(999999);
        order_a.cmp(&order_b)
    });

    let result: Vec<NestedTerm> = root_terms
        .into_iter()
        .map(|term| build_multilingual_children(term.id, &term_map, &children_map))
        .collect();

    result
}
