//! Kampanya Motoru — E-ticaret indirim/kampanya değerlendirme sisteminin çekirdeği.
//!
//! Bu modül, sepet kampanyalarının ve kuponların değerlendirilmesini sağlayan merkezi motoru içerir.
//! Bir sepet üzerindeki tüm aktif kampanyaları sırayla işler, her senaryo tipine göre indirim hesaplar
//! ve çakışma/yığınlama kurallarına göre sonuca ulaşır.
//!
//! # Kritik Uyarılar
//!
//! - **`dry_run` ayrımı çok önemlidir:** `dry_run=true` olduğunda motor yalnızca hesaplama yapar,
//!   `cart_discounts` tablosuna **yazmaz**. `dry_run=false` olduğunda ise eski kayıtları silip
//!   yenilerini yazar. `apply_coupon`, `remove_coupon` ve `cart_summary` handler'ları
//!   `dry_run=false` ile çağırmak **zorundadır**; aksi halde indirimler veritabanına kaydedilmez.
//!
//! - **Kupon `usage_count` artırımı:** Kupon `usage_count` değeri sepete uygulama sırasında
//!   artırılmaz; yalnızca sipariş tamamlandığında artırılır.
//!
//! - **Para birimi dönüşümü:** Tüm değerlendiriciler `exchange_rates` alır. Kampanya parametrelerindeki
//!   `currency` ile kullanıcının görüntülediği `display_currency` farklı olabilir; bu durumda
//!   döviz kuru üzerinden dönüşüm yapılır.
//!
//! - **cart_summary akışı:** `cart_summary` handler'ı, `evaluate` çağırmadan **önce** `cart_discounts`
//!   tablosundan uygulanan kupon kodunu okur; çünkü `evaluate` her çağrıldığında `cart_discounts`
//!   kayıtlarını silip yeniden yazar.

pub mod evaluators;
pub mod helpers;

use rust_decimal::Decimal;
use sea_orm::TransactionTrait;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use sea_orm::sea_query::LockType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// use crate::modules::currency::models::exchange_rate::Model as ExchangeRateModel;
use crate::modules::currency::services::exchange_rate_service::{
    convert_currency, get_cached_rates,
};
use crate::modules::ecommerce::campaign::scenario::{
    BuyXGetYFreeParams, CartTotalDiscountParams, CategorySpendGetDiscountParams, CouponCodeParams,
    FirstOrderDiscountParams, FreeShippingParams, QuantityDiscountPercentParams, ScenarioType,
};
use crate::modules::ecommerce::models::cart_discount::discount_type;
use crate::modules::ecommerce::models::{
    campaign, campaign_usage, cart, cart_discount, cart_item, coupon,
};

use self::evaluators::*;
use self::evaluators::free_shipping::convert_amount;
use self::helpers::get_product_price_in_currency;

/// Sepet ürünü hakkında gerekli tüm bilgileri taşıyan yapı.
///
/// Her `CartItemInfo`, sepetteki bir ürünün liste fiyatını, indirim yüzdesini,
/// birim fiyatını ve satır toplamını içerir. Tutarlar `display_currency` cinsinden
/// tutulur; ürün kaydındaki `original_price`/`currency` farklıysa döviz kuru
/// üzerinden dönüştürülerek oluşturulur.
#[derive(Debug, Clone)]
pub struct CartItemInfo {
    pub product_id: i64,
    pub cart_item_id: i64,
    pub quantity: i32,
    /// Birim fiyat üzerinden hesaplanmış satır toplamı (`display_currency` cinsinden).
    /// Kampanya motoru tüm hesaplamaları bu tutar üzerinden yapar.
    pub line_total: Decimal,
    /// Fiyatların dönüştürüldüğü hedef para birimi kodu (ör. "TRY", "USD").
    pub currency: String,
}

/// Tek bir indirim sonucunu temsil eden yapı.
///
/// Her başarıyla değerlendirilen senaryo, bir `DiscountResult` üretir.
/// Bu yapı `cart_discounts` tablosuna yazılacak veriyi ve istemciye
/// döndürülecek açıklamayı barındırır.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountResult {
    pub campaign_id: i64,
    pub coupon_id: Option<i64>,
    pub scenario_type: ScenarioType,
    pub discount_type: String,
    /// İndirimin kapsamı: `"cart"` (sepetteki tüm ürünler) veya `"item"` (belirli bir ürün).
    pub scope: String,
    /// `scope == "item"` olduğunda indirimin uygulandığı sepetteki ürün satırı ID'si.
    pub cart_item_id: Option<i64>,
    pub amount: Decimal,
    pub currency: String,
    pub description: String,
    pub cart_id: i64,
}

/// Sepet özetini tutan yapı; istemciye döndürülür.
///
/// Alt toplam, toplam indirim, ödeme tutarı ve ücretsiz kargo bilgilerinin
/// yanı sıra her indirimin insan-okunabilir açıklamasını da barındırır.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartSummary {
    pub subtotal: Decimal,
    pub total_discount: Decimal,
    pub total: Decimal,
    pub currency: String,
    pub free_shipping: bool,
    pub cargo_fee: Decimal,
    pub cargo_fee_formatted: String,
    pub remaining_amount_for_free_shipping: Decimal,
    pub remaining_amount_for_free_shipping_formatted: String,
    pub free_shipping_threshold: Decimal,
    pub free_shipping_threshold_formatted: String,
    pub discounts: Vec<DiscountDescription>,
    /// Sepete uygulanmış kupon kodu (varsa).
    pub applied_coupon: Option<String>,
    pub subtotal_formatted: String,
    pub total_discount_formatted: String,
    pub total_formatted: String,
}

/// Tek bir indirimin insan-okunabilir açıklaması.
///
/// `CartSummary` içindeki `discounts` vektöründe yer alır ve
/// istemci arayüzünde gösterilecek biçimlendirilmiş tutarı içerir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscountDescription {
    pub campaign_id: i64,
    pub scenario_type: String,
    pub description: String,
    pub amount: Decimal,
    pub currency: String,
    /// Para birimi sembolü ve binlik ayracı ile biçimlendirilmiş tutar.
    pub amount_formatted: String,
}

/// `CampaignEngine::evaluate` metodunun döndürdüğü sonuç.
///
/// `dry_run` alanı, değerlendirmenin yalnızca hesaplama modunda
/// mı yoksa veritabanına yazan modda mı yapıldığını belirtir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateResult {
    pub dry_run: bool,
    pub summary: CartSummary,
    /// Yalnızca `dry_run=true` olduğunda dolu olur; hangi kampanyaların
    /// değerlendirildiğini, hangilerinin atlandığını ve veritabanına
    /// yazılacak olası `cart_discounts` kayıtlarını gösterir.
    pub dry_run_report: Option<DryRunReport>,
}

/// `dry_run=true` modunda detaylı rapor; hangi kampanyaların
/// değerlendirildiğini, uygulandığını ve atlandığını listeler.
///
/// Bu rapor, istemcinin indirim hesaplamasını canlı veritabanı
/// değişikliği olmadan önizlemesini sağlar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunReport {
    pub evaluated_campaigns: Vec<CampaignEvalDetail>,
    pub applied: Vec<CampaignEvalDetail>,
    pub skipped: Vec<CampaignEvalDetail>,
    /// `dry_run=false` olsaydı `cart_discounts` tablosuna yazılacak kayıtların önizlemesi.
    pub would_write: Vec<CartDiscountPreview>,
}

/// Tek bir kampanyanın değerlendirme detayı; eligible/atlanma nedeni bilgisini taşır.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignEvalDetail {
    pub campaign_id: i64,
    pub campaign_name: String,
    pub scenario_type: String,
    pub eligible: bool,
    /// Kampanya uygun değilse veya atlandıysa nedenini belirten açıklama.
    pub skip_reason: Option<String>,
    pub discount_amount: Option<Decimal>,
}

/// `dry_run=true` modunda, veritabanına yazılacak indirim kayıtlarının önizlemesi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartDiscountPreview {
    pub campaign_id: i64,
    pub scenario_type: String,
    pub discount_type: String,
    pub scope: String,
    pub cart_item_id: Option<i64>,
    pub amount: Decimal,
    pub currency: String,
    pub description: String,
}

/// Kampanya değerlendirme motoru; sepet üzerinden tüm aktif kampanyaları
/// değerlendirir, çakışma kurallarını uygular ve sonuçları `cart_discounts`
/// tablosuna yazar (veya `dry_run=true` ise yalnızca hesaplar).
pub struct CampaignEngine {
    pub db: DatabaseConnection,
}

impl CampaignEngine {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Sepet üzerindeki tüm aktif kampanyaları değerlendirir ve indirimleri hesaplar.
    ///
    /// # Akış (Adım Adım)
    ///
    /// 1. **Döviz kurlarını yükle:** `get_cached_rates` ile önbelleğe alınmış kurları alır.
    ///
    /// 2. **Sepet ve ürünleri yükle:** `cart_id` ile sepeti ve sepetteki ürünleri getirir.
    ///
    /// 3. **Ürün fiyatlarını `display_currency`'e dönüştür:**
    ///    - Ürünün `original_price` ve `currency` alanları varsa, para birimi
    ///      `display_currency`'den farklıysa döviz kuru üzerinden dönüştürülür.
    ///    - Bu alanlar yoksa `get_product_price_in_currency` ile ürün tablosundan
    ///      fiyat okunur ve dönüştürülür.
    ///
    /// 4. **`discount_percentage` uygula:** Her ürün satırının `discount_percentage`
    ///    değeri varsa birim fiyat `list_price * (1 - discount_percentage / 100)`
    ///    formülüyle hesaplanır; yoksa `unit_price = list_price` kabul edilir.
    ///
    /// 5. **Sepet toplamını hesapla:** Tüm `line_total` değerleri toplanır.
    ///
    /// 6. **Aktif kampanyaları filtrele:**
    ///    - `is_active = true` olanlar,
    ///    - `starts_at <= şimdi` olanlar,
    ///    - `ends_at >= şimdi` veya `ends_at IS NULL` olanlar,
    ///    - `priority` azalan sırayla sıralanmış olanlar.
    ///
    /// 7. **Her kampanya için:**
    ///    a. **Genel kullanım limiti kontrolü:** `usage_count >= max_uses` ise atla.
    ///    b. **Kullanıcı bazlı limit kontrolü:** Kullanıcının `campaign_usage`
    ///       kayıtlarındaki kullanım sayısı `max_uses_per_user`'ı geçiyorsa atla.
    ///    c. **Senaryo tipini çözümle:** `scenario_type` bilinmiyorsa atla.
    ///    d. **Senaryoyu değerlendir:** İlgili `eval_*` fonksiyonunu çağır;
    ///       koşullar sağlanmıyorsa atla.
    ///    e. **Ürün-seviyesi çakışma kontrolü:** Eğer sonuç `scope = "item"`
    ///       ise ve aynı `cart_item_id` zaten başka bir kampanya tarafından
    ///       kapsanıyorsa bu indirim atlanır (öne çıkan kampanya önceliğe saiptir).
    ///    f. **Yığınlama (stackability) kontrolü:** Kampanya `stackable = false`
    ///       ise, önceden uygulanmış yığınlama yapılmayan (`stackable=false`)
    ///       indirimlerle karşılaştırılır; en yüksek tutarlı olan korunur,
    ///       düşük tutarlı olan çıkarılır.
    ///    g. Yeni indirim `discounts` vektörüne eklenir.
    ///
    /// 8. **Sonuçları derle:** `free_shipping` bayrağı, `total_discount` ve `total`
    ///    hesaplanır; `CartSummary` oluşturulur.
    ///
    /// 9. **Veritabanı yazma/loading:**
    ///    - `dry_run=true`: Herhangi bir DB yazma işlemi yapılmaz;
    ///      `DryRunReport` ile detaylı rapor döndürülür.
    ///    - `dry_run=false`: Sepetin eski `cart_discounts` kayıtları silinir,
    ///      yeni indirimler satır satır yazılır.
    ///
    /// # Kritik Uyarılar
    ///
    /// - **`dry_run` ayarı:** `config.toml` dosyasında varsayılan olarak `true` olabilir;
    ///   ancak `apply_coupon`, `remove_coupon` ve `cart_summary` handler'ları
    ///   `dry_run=false` ile çağırmak **zorundadır**. Aksi takdirde indirimler
    ///   veritabanına kaydedilmez ve sonraki isteklerde getirilemez.
    ///
    /// - **Kupon `usage_count`:** Kupon uygulandığında `usage_count` artırılmaz;
    ///   yalnızca sipariş tamamlandığında artırılmalıdır.
    ///
    /// - **Para birimi:** `params.currency` ile `display_currency` farklı olabilir;
    ///   tüm değerlendiriciler `exchange_rates` alır ve dönüşümü kendileri yapar.
    ///
    /// - **cart_summary ve kupon kodu:** `cart_summary` handler'ı, `evaluate`'i
    ///   çağırmadan **önce** `cart_discounts` tablosundan kupon kodunu okumalıdır;
    ///   çünkü `evaluate` her çağrıldığında `cart_discounts` kayıtlarını silip
    ///   yeniden yazar.
    pub async fn evaluate(
        &self,
        cart_id: i64,
        user_id: i64,
        applied_coupon_code: Option<&str>,
        dry_run: bool,
        display_currency: &str,
        standard_cargo_fee: Decimal,
    ) -> Result<EvaluateResult, String> {
        let db_conn = &self.db;

        // Adım 0: Transaction (İşlem) başlatıyoruz. Bu, tüm sürecin (silme + hesaplama + ekleme) 
        // atomik olmasını sağlar ve diğer isteklerin yarım kalmış veriyi görmesini engeller.
        let txn = db_conn.begin().await.map_err(|e| format!("Transaction başlatılamadı: {}", e))?;

        // Adım 1: Önbelleğe alınmış döviz kurlarını yükle
        let exchange_rates = get_cached_rates(&txn).await;

        // Adım 2: Sepeti ve sepetteki ürünleri getir
        // dry_run=false ise sepeti FOR UPDATE ile kilitliyoruz (PostgreSQL seviyesinde). 
        // Bu, aynı sepet üzerinde başka bir evaluate() çalışmasını ve yarış durumlarını (race condition) engeller.
        let mut cart_query = cart::Entity::find_by_id(cart_id);
        if !dry_run {
            cart_query = cart_query.lock(LockType::Update);
        }

        let cart_model = cart_query
            .one(&txn)
            .await
            .map_err(|e| format!("DB error: {}", e))?
            .ok_or("Sepet bulunamadı")?;

        let cart_items_models = cart_item::Entity::find()
            .filter(cart_item::Column::CartId.eq(cart_id))
            .all(&txn)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

        // Adım 3-4: Her ürün için fiyatı display_currency cinsinden hesapla
        let mut cart_items_data: Vec<CartItemInfo> = Vec::new();
        for item in &cart_items_models {
            // Para birimi dönüşümü: Ürünün kendi original_price/currency alanları
            // varsa display_currency'e dönüştür; yoksa ürün tablosundan fiyat oku
            let list_price = if let (Some(orig_price), Some(orig_currency)) =
                (item.original_price, &item.currency)
            {
                let orig_currency_upper = orig_currency.to_uppercase();
                if orig_currency_upper == display_currency.to_uppercase() {
                    // Aynı para birimi — dönüştürmeye gerek yok
                    orig_price
                } else if let Some(rates) = exchange_rates.as_ref() {
                    // Farklı para birimi — döviz kuru ile dönüştür
                    convert_currency(
                        orig_price.to_string().parse::<f64>().unwrap_or(0.0),
                        &orig_currency_upper,
                        display_currency,
                        rates,
                    )
                    .and_then(|v| Decimal::try_from(v).ok())
                    .unwrap_or(orig_price) // Dönüşüm başarısız olursa orijinal fiyatı kullan
                } else {
                    // Döviz kuru yoksa orijinal fiyatı olduğu gibi kullan
                    orig_price
                }
            } else {
                // Üründe original_price/currency yoksa ürün tablosundan getir
                get_product_price_in_currency(
                    &txn,
                    item.product_id,
                    display_currency,
                    exchange_rates.as_ref(),
                )
                .await
                .unwrap_or(Decimal::ZERO)
            };

            // discount_percentage: Üründe tanımlı yüzde indirimi uygula
            let discount_pct = item.discount_percentage.unwrap_or(Decimal::ZERO);
            let unit_price = if discount_pct > Decimal::ZERO {
                // Birim fiyat = liste fiyatı * (1 - yüzde / 100)
                let discount_multiplier = Decimal::ONE - (discount_pct / Decimal::from(100));
                list_price * discount_multiplier
            } else {
                list_price
            };
            let line_total = unit_price * Decimal::from(item.quantity);
            cart_items_data.push(CartItemInfo {
                product_id: item.product_id,
                cart_item_id: item.id,
                quantity: item.quantity,
                line_total,
                currency: display_currency.to_string(),
            });
        }

        // Adım 5: Sepet toplamı = tüm satır toplamlarının sum'ı
        let cart_total: Decimal = cart_items_data.iter().map(|i| i.line_total).sum();
        let display_currency_str = display_currency.to_string();

        let now = chrono::Utc::now();

        // Adım 6: Aktif kampanyaları filtrele — is_active, tarih aralığı, priority sıralı
        let mut campaign_query = campaign::Entity::find()
            .filter(campaign::Column::IsActive.eq(true))
            .filter(campaign::Column::StartsAt.lte(now))
            .filter(
                campaign::Column::EndsAt
                    .gte(now)
                    .or(campaign::Column::EndsAt.is_null()),
            )
            .filter(
                campaign::Column::TargetCartType
                    .eq("both")
                    .or(campaign::Column::TargetCartType.eq(cart_model.cart_type.clone())),
            );

        // campaign_type filtresi: Kupon yoksa sadece otomatik olanları getir.
        // Bu, yüzlerce kuponlu kampanya varken performansı ciddi oranda artırır.
        if let Some(coupon_code) = applied_coupon_code {
            // Eğer kupon girilmişse, bu kuponun hangi kampanyaya ait olduğunu bulalım
            let coupon_campaign_id = coupon::Entity::find()
                .filter(coupon::Column::Code.eq(coupon_code.to_uppercase()))
                .filter(coupon::Column::IsActive.eq(true))
                .one(&txn)
                .await
                .ok()
                .flatten()
                .map(|c| c.campaign_id);

            if let Some(campaign_id) = coupon_campaign_id {
                // Hem otomatik kampanyaları hem de bu kuponun bağlı olduğu kampanyayı çek
                campaign_query = campaign_query.filter(
                    campaign::Column::CampaignType
                        .eq(campaign::campaign_type::AUTOMATIC)
                        .or(campaign::Column::Id.eq(campaign_id)),
                );
            } else {
                // Kupon geçersizse sadece otomatik olanları çek
                campaign_query = campaign_query
                    .filter(campaign::Column::CampaignType.eq(campaign::campaign_type::AUTOMATIC));
            }
        } else {
            // Kupon belirtilmemişse sadece otomatik kampanyaları çek
            campaign_query = campaign_query
                .filter(campaign::Column::CampaignType.eq(campaign::campaign_type::AUTOMATIC));
        }

        let active_campaigns = campaign_query
            .order_by_desc(campaign::Column::Priority)
            .all(&txn)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

        let mut discounts: Vec<DiscountResult> = Vec::new();
        let mut applied_coupon_string: Option<String> = None;

        let mut eval_details: Vec<CampaignEvalDetail> = Vec::new();
        let mut applied_details: Vec<CampaignEvalDetail> = Vec::new();
        let mut skipped_details: Vec<CampaignEvalDetail> = Vec::new();

        // Ürün-seviyesi çakışma takibi: cart_item_id → bu ürünü kapsayan ilk kampanya_id
        // Aynı cart_item_id üzerinde yalnızca bir ürün-seviye indirim olabilir
        let mut applied_item_scopes: HashMap<i64, i64> = HashMap::new();

        // Adım 7: Her aktif kampanyayı sırayla değerlendir
        for campaign_model in &active_campaigns {
            // 7a: Genel kullanım limiti kontrolü
            if let Some(max_uses) = campaign_model.max_uses {
                if campaign_model.usage_count >= max_uses {
                    let detail = CampaignEvalDetail {
                        campaign_id: campaign_model.id,
                        campaign_name: campaign_model.name.clone(),
                        scenario_type: campaign_model.scenario_type.clone(),
                        eligible: false,
                        skip_reason: Some("Kullanım limiti doldu".to_string()),
                        discount_amount: None,
                    };
                    eval_details.push(detail.clone());
                    skipped_details.push(detail);
                    continue;
                }
            }

            // 7b: Kullanıcı bazlı kullanım limiti kontrolü
            if let Some(max_per_user) = campaign_model.max_uses_per_user {
                let user_usage = campaign_usage::Entity::find()
                    .filter(campaign_usage::Column::CampaignId.eq(campaign_model.id))
                    .filter(campaign_usage::Column::UserId.eq(user_id))
                    .count(&txn)
                    .await
                    .unwrap_or(0);
                if user_usage >= max_per_user as u64 {
                    let detail = CampaignEvalDetail {
                        campaign_id: campaign_model.id,
                        campaign_name: campaign_model.name.clone(),
                        scenario_type: campaign_model.scenario_type.clone(),
                        eligible: false,
                        skip_reason: Some("Kullanıcı kullanım limiti doldu".to_string()),
                        discount_amount: None,
                    };
                    eval_details.push(detail.clone());
                    skipped_details.push(detail);
                    continue;
                }
            }

            // 7c: Senaryo tipini çözümle — bilinmeyen tipler atlanır
            let scenario_type = match ScenarioType::all()
                .into_iter()
                .find(|s| s.as_str() == campaign_model.scenario_type)
            {
                Some(st) => st,
                None => {
                    let detail = CampaignEvalDetail {
                        campaign_id: campaign_model.id,
                        campaign_name: campaign_model.name.clone(),
                        scenario_type: campaign_model.scenario_type.clone(),
                        eligible: false,
                        skip_reason: Some("Bilinmeyen senaryo tipi".to_string()),
                        discount_amount: None,
                    };
                    eval_details.push(detail.clone());
                    skipped_details.push(detail);
                    continue;
                }
            };

            // 7d: Senaryo tipine göre ilgili değerlendiriciyi çağır
            let result = match scenario_type {
                ScenarioType::BuyXGetYFree => {
                    let params: BuyXGetYFreeParams =
                        match serde_json::from_value(campaign_model.params.clone()) {
                            Ok(p) => p,
                            Err(e) => {
                                tracing::error!(
                                    "Invalid params for campaign {}: {}",
                                    campaign_model.id,
                                    e
                                );
                                continue;
                            }
                        };
                    let mut r = match eval_buy_x_get_y_free(
                        &txn,
                        &params,
                        &cart_items_data,
                        &display_currency_str,
                        exchange_rates.as_ref(),
                    )
                    .await
                    {
                        Some(r) => r,
                        None => {
                            let detail = CampaignEvalDetail {
                                campaign_id: campaign_model.id,
                                campaign_name: campaign_model.name.clone(),
                                scenario_type: campaign_model.scenario_type.clone(),
                                eligible: false,
                                skip_reason: Some("Koşullar sağlanmadı".to_string()),
                                discount_amount: None,
                            };
                            eval_details.push(detail.clone());
                            skipped_details.push(detail);
                            continue;
                        }
                    };
                    r.campaign_id = campaign_model.id;
                    r.cart_id = cart_id;
                    r
                }
                ScenarioType::QuantityDiscountPercent => {
                    let params: QuantityDiscountPercentParams =
                        match serde_json::from_value(campaign_model.params.clone()) {
                            Ok(p) => p,
                            Err(e) => {
                                tracing::error!(
                                    "Invalid params for campaign {}: {}",
                                    campaign_model.id,
                                    e
                                );
                                continue;
                            }
                        };
                    let mut r =
                        match eval_quantity_discount_percent(&txn, &params, &cart_items_data).await {
                            Some(r) => r,
                            None => {
                                let detail = CampaignEvalDetail {
                                    campaign_id: campaign_model.id,
                                    campaign_name: campaign_model.name.clone(),
                                    scenario_type: campaign_model.scenario_type.clone(),
                                    eligible: false,
                                    skip_reason: Some("Koşullar sağlanmadı".to_string()),
                                    discount_amount: None,
                                };
                                eval_details.push(detail.clone());
                                skipped_details.push(detail);
                                continue;
                            }
                        };
                    r.campaign_id = campaign_model.id;
                    r.cart_id = cart_id;
                    r
                }
                ScenarioType::CategorySpendGetDiscount => {
                    let params: CategorySpendGetDiscountParams =
                        match serde_json::from_value(campaign_model.params.clone()) {
                            Ok(p) => p,
                            Err(e) => {
                                tracing::error!(
                                    "Invalid params for campaign {}: {}",
                                    campaign_model.id,
                                    e
                                );
                                continue;
                            }
                        };
                    let mut r = match eval_category_spend_get_discount(
                        &txn,
                        &params,
                        &cart_items_data,
                        &display_currency_str,
                        exchange_rates.as_ref(),
                    )
                    .await
                    {
                        Some(r) => r,
                        None => {
                            let detail = CampaignEvalDetail {
                                campaign_id: campaign_model.id,
                                campaign_name: campaign_model.name.clone(),
                                scenario_type: campaign_model.scenario_type.clone(),
                                eligible: false,
                                skip_reason: Some("Koşullar sağlanmadı".to_string()),
                                discount_amount: None,
                            };
                            eval_details.push(detail.clone());
                            skipped_details.push(detail);
                            continue;
                        }
                    };
                    r.campaign_id = campaign_model.id;
                    r.cart_id = cart_id;
                    r
                }
                ScenarioType::CartTotalDiscount => {
                    let params: CartTotalDiscountParams =
                        match serde_json::from_value(campaign_model.params.clone()) {
                            Ok(p) => p,
                            Err(e) => {
                                tracing::error!(
                                    "Invalid params for campaign {}: {}",
                                    campaign_model.id,
                                    e
                                );
                                continue;
                            }
                        };
                    let mut r = match eval_cart_total_discount(
                        &params,
                        cart_total,
                        &display_currency_str,
                        exchange_rates.as_ref(),
                    ) {
                        Some(r) => r,
                        None => {
                            let detail = CampaignEvalDetail {
                                campaign_id: campaign_model.id,
                                campaign_name: campaign_model.name.clone(),
                                scenario_type: campaign_model.scenario_type.clone(),
                                eligible: false,
                                skip_reason: Some("Koşullar sağlanmadı".to_string()),
                                discount_amount: None,
                            };
                            eval_details.push(detail.clone());
                            skipped_details.push(detail);
                            continue;
                        }
                    };
                    r.campaign_id = campaign_model.id;
                    r.cart_id = cart_id;
                    r
                }
                ScenarioType::CouponCode => {
                    // Kupon kodu belirtilmemişse bu senaryoyu atla
                    let coupon_code_str = match applied_coupon_code {
                        Some(code) => code,
                        None => {
                            let detail = CampaignEvalDetail {
                                campaign_id: campaign_model.id,
                                campaign_name: campaign_model.name.clone(),
                                scenario_type: campaign_model.scenario_type.clone(),
                                eligible: false,
                                skip_reason: Some("Kupon kodu belirtilmedi".to_string()),
                                discount_amount: None,
                            };
                            eval_details.push(detail.clone());
                            skipped_details.push(detail);
                            continue;
                        }
                    };

                    // Kupon kodunu veritabanında doğrula — aktif ve bu kampanyaya ait olmalı
                    let coupon_model = coupon::Entity::find()
                        .filter(coupon::Column::CampaignId.eq(campaign_model.id))
                        .filter(coupon::Column::Code.eq(coupon_code_str.to_uppercase()))
                        .filter(coupon::Column::IsActive.eq(true))
                        .one(&txn)
                        .await
                        .ok()
                        .flatten();

                    let coupon_model = match coupon_model {
                        Some(c) => c,
                        None => {
                            let detail = CampaignEvalDetail {
                                campaign_id: campaign_model.id,
                                campaign_name: campaign_model.name.clone(),
                                scenario_type: campaign_model.scenario_type.clone(),
                                eligible: false,
                                skip_reason: Some("Kupon kodu geçersiz".to_string()),
                                discount_amount: None,
                            };
                            eval_details.push(detail.clone());
                            skipped_details.push(detail);
                            continue;
                        }
                    };

                    // Kupon kullanım limiti kontrolü
                    // NOT: usage_count burada artırılmaz; yalnızca sipariş tamamlandığında artırılır
                    if let Some(max_usage) = coupon_model.max_usage {
                        if coupon_model.usage_count >= max_usage {
                            let detail = CampaignEvalDetail {
                                campaign_id: campaign_model.id,
                                campaign_name: campaign_model.name.clone(),
                                scenario_type: campaign_model.scenario_type.clone(),
                                eligible: false,
                                skip_reason: Some("Kupon kullanım limiti doldu".to_string()),
                                discount_amount: None,
                            };
                            eval_details.push(detail.clone());
                            skipped_details.push(detail);
                            continue;
                        }
                    }

                    // Kupon geçerlilik tarihi kontrolü
                    if let Some(valid_until) = coupon_model.valid_until {
                        if chrono::Utc::now() > valid_until {
                            let detail = CampaignEvalDetail {
                                campaign_id: campaign_model.id,
                                campaign_name: campaign_model.name.clone(),
                                scenario_type: campaign_model.scenario_type.clone(),
                                eligible: false,
                                skip_reason: Some("Kupon süresi dolmuş".to_string()),
                                discount_amount: None,
                            };
                            eval_details.push(detail.clone());
                            skipped_details.push(detail);
                            continue;
                        }
                    }

                    let params: CouponCodeParams =
                        match serde_json::from_value(campaign_model.params.clone()) {
                            Ok(p) => p,
                            Err(e) => {
                                tracing::error!(
                                    "Invalid params for campaign {}: {}",
                                    campaign_model.id,
                                    e
                                );
                                continue;
                            }
                        };

                    let mut r = match eval_coupon_code(
                        &txn,
                        &params,
                        &coupon_model,
                        cart_total,
                        &display_currency_str,
                        &cart_items_data,
                        exchange_rates.as_ref(),
                    )
                    .await
                    {
                        Some(r) => r,
                        None => {
                            let detail = CampaignEvalDetail {
                                campaign_id: campaign_model.id,
                                campaign_name: campaign_model.name.clone(),
                                scenario_type: campaign_model.scenario_type.clone(),
                                eligible: false,
                                skip_reason: Some("Koşullar sağlanmadı".to_string()),
                                discount_amount: None,
                            };
                            eval_details.push(detail.clone());
                            skipped_details.push(detail);
                            continue;
                        }
                    };
                    r.campaign_id = campaign_model.id;
                    r.coupon_id = Some(coupon_model.id);
                    r.cart_id = cart_id;
                    // Kupon kodunu sakla — cart_summary'de applied_coupon olarak döndürülecek
                    applied_coupon_string = Some(coupon_model.code.clone());
                    r
                }
                ScenarioType::FreeShipping => {
                    let params: FreeShippingParams =
                        match serde_json::from_value(campaign_model.params.clone()) {
                            Ok(p) => p,
                            Err(e) => {
                                tracing::error!(
                                    "Invalid params for campaign {}: {}",
                                    campaign_model.id,
                                    e
                                );
                                continue;
                            }
                        };
                    let mut r = match eval_free_shipping(
                        &params,
                        cart_total,
                        &display_currency_str,
                        exchange_rates.as_ref(),
                    ) {
                        Some(r) => r,
                        None => {
                            let detail = CampaignEvalDetail {
                                campaign_id: campaign_model.id,
                                campaign_name: campaign_model.name.clone(),
                                scenario_type: campaign_model.scenario_type.clone(),
                                eligible: false,
                                skip_reason: Some("Koşullar sağlanmadı".to_string()),
                                discount_amount: None,
                            };
                            eval_details.push(detail.clone());
                            skipped_details.push(detail);
                            continue;
                        }
                    };
                    r.campaign_id = campaign_model.id;
                    r.cart_id = cart_id;
                    r
                }
                ScenarioType::FirstOrderDiscount => {
                    let params: FirstOrderDiscountParams =
                        match serde_json::from_value(campaign_model.params.clone()) {
                            Ok(p) => p,
                            Err(e) => {
                                tracing::error!(
                                    "Invalid params for campaign {}: {}",
                                    campaign_model.id,
                                    e
                                );
                                continue;
                            }
                        };
                    let mut r = match eval_first_order_discount(
                        &txn,
                        &params,
                        user_id,
                        cart_total,
                        &display_currency_str,
                        exchange_rates.as_ref(),
                    )
                    .await
                    {
                        Some(r) => r,
                        None => {
                            let detail = CampaignEvalDetail {
                                campaign_id: campaign_model.id,
                                campaign_name: campaign_model.name.clone(),
                                scenario_type: campaign_model.scenario_type.clone(),
                                eligible: false,
                                skip_reason: Some("Koşullar sağlanmadı".to_string()),
                                discount_amount: None,
                            };
                            eval_details.push(detail.clone());
                            skipped_details.push(detail);
                            continue;
                        }
                    };
                    r.campaign_id = campaign_model.id;
                    r.cart_id = cart_id;
                    r
                }
            };

            // 7e: Ürün-seviyesi çakışma kontrolü
            // Aynı cart_item_id üzerinde iki farklı ürün-seviye indirim uygulanamaz;
            // önce gelen (önceliği yüksek) kampanya kazanan olur
            if let Some(item_id) = result.cart_item_id {
                if let Some(existing_campaign_id) = applied_item_scopes.get(&item_id) {
                    let detail = CampaignEvalDetail {
                        campaign_id: campaign_model.id,
                        campaign_name: campaign_model.name.clone(),
                        scenario_type: campaign_model.scenario_type.clone(),
                        eligible: true,
                        skip_reason: Some(format!(
                            "Ürün seviyesi indirim çakışması: ürün {} zaten kampanya {} tarafından kapsanıyor",
                            item_id, existing_campaign_id
                        )),
                        discount_amount: None,
                    };
                    eval_details.push(detail.clone());
                    skipped_details.push(detail);
                    continue;
                }
            }

            // Değerlendirme başarılı — detayları kaydet
            let detail = CampaignEvalDetail {
                campaign_id: campaign_model.id,
                campaign_name: campaign_model.name.clone(),
                scenario_type: campaign_model.scenario_type.clone(),
                eligible: true,
                skip_reason: None,
                discount_amount: Some(result.amount),
            };
            eval_details.push(detail.clone());
            applied_details.push(detail);

            // 7f: Yığınlama (stackability) kontrolü
            // stackable=false olan kampanyalar, diğer yığınlama yapılmayan kampanyalarla çakışır;
            // aralarından en yüksek indirimli olan korunur, düşük olan çıkarılır
            if !campaign_model.stackable {
                let non_stackable_existing: Vec<&DiscountResult> = discounts
                    .iter()
                    .filter(|d| {
                        let existing_campaign =
                            active_campaigns.iter().find(|c| c.id == d.campaign_id);
                        existing_campaign.map_or(true, |c| !c.stackable)
                    })
                    .collect();

                if let Some(best) = non_stackable_existing.into_iter().max_by_key(|d| d.amount) {
                    if result.amount <= best.amount {
                        // Yeni indirim daha düşük veya eşit — mevcut kazanan korunur, yeni atlanır
                        let detail = CampaignEvalDetail {
                            campaign_id: campaign_model.id,
                            campaign_name: campaign_model.name.clone(),
                            scenario_type: campaign_model.scenario_type.clone(),
                            eligible: true,
                            skip_reason: Some(format!(
                                "Daha yüksek indirimli kampanya ile çakışıyor: {}",
                                best.campaign_id
                            )),
                            discount_amount: Some(result.amount),
                        };
                        skipped_details.push(detail);
                        continue;
                    }

                    // Yeni indirim daha yüksek — eskisini listeden çıkar
                    let best_campaign_id = best.campaign_id;
                    discounts.retain(|d| d.campaign_id != best_campaign_id);
                }
            }

            // Ürün-seviye indirimi varsa, bu ürünü kapsananlar arasına ekle
            // (sonraki kampanyalarda aynı ürün üzerinde çakışma olmaması için)
            if let Some(item_id) = result.cart_item_id {
                applied_item_scopes.insert(item_id, campaign_model.id);
            }

            discounts.push(result);
        }

        // Adım 8: Sonuçları derle
        let mut free_shipping = discounts
            .iter()
            .any(|d| d.discount_type == discount_type::FREE_SHIPPING);
        
        let mut free_shipping_threshold = Decimal::ZERO;
        let mut remaining_amount_for_free_shipping = Decimal::ZERO;

        // "Senaryo yoksa kargo ücretsiz" kuralı uygulaması
        let has_free_shipping_scenario = active_campaigns.iter().any(|c| c.scenario_type == ScenarioType::FreeShipping.as_str());
        
        if !has_free_shipping_scenario {
            // Hiç ücretsiz kargo senaryosu tanımlanmamışsa kargo ücretsizdir.
            free_shipping = true;
        } else {
            // Eğer senaryolar varsa ama hiçbiri uygulanmamışsa (limit altı), 
            // en düşük limiti bulup ne kadar kaldığını hesaplayalım.
            if !free_shipping {
                let mut lowest_threshold: Option<Decimal> = None;
                
                for campaign_model in &active_campaigns {
                    if campaign_model.scenario_type == ScenarioType::FreeShipping.as_str() {
                        if let Ok(params) = serde_json::from_value::<FreeShippingParams>(campaign_model.params.clone()) {
                            // Threshold'u display_currency'e çevir
                            let threshold_converted = convert_amount(
                                params.min_cart_total,
                                &params.currency,
                                &display_currency_str,
                                exchange_rates.as_ref()
                            );
                            
                            if lowest_threshold.is_none() || threshold_converted < lowest_threshold.unwrap() {
                                lowest_threshold = Some(threshold_converted);
                            }
                        }
                    }
                }
                
                if let Some(threshold) = lowest_threshold {
                    free_shipping_threshold = threshold;
                    if cart_total < threshold {
                        remaining_amount_for_free_shipping = threshold - cart_total;
                    }
                }
            }
        }

        let total_discount: Decimal = discounts.iter().map(|d| d.amount).sum();
        let mut total = (cart_total - total_discount).max(Decimal::ZERO);
        
        let cargo_fee = if free_shipping {
            Decimal::ZERO
        } else {
            standard_cargo_fee
        };
        
        // Final toplam kargo ücretini içerir
        total += cargo_fee;

        let discount_descriptions: Vec<DiscountDescription> = discounts
            .iter()
            .map(|d| DiscountDescription {
                campaign_id: d.campaign_id,
                scenario_type: d.scenario_type.to_string(),
                description: d.description.clone(),
                amount: d.amount,
                currency: d.currency.clone(),
                amount_formatted: crate::modules::utils::format_price::format_price(
                    d.amount.to_string().parse::<f64>().unwrap_or(0.0),
                    &d.currency,
                ),
            })
            .collect();

        let summary = CartSummary {
            subtotal: cart_total,
            total_discount,
            total,
            currency: display_currency_str.clone(),
            free_shipping,
            cargo_fee,
            cargo_fee_formatted: crate::modules::utils::format_price::format_price(
                cargo_fee.to_string().parse::<f64>().unwrap_or(0.0),
                &display_currency_str,
            ),
            remaining_amount_for_free_shipping,
            remaining_amount_for_free_shipping_formatted: crate::modules::utils::format_price::format_price(
                remaining_amount_for_free_shipping.to_string().parse::<f64>().unwrap_or(0.0),
                &display_currency_str,
            ),
            free_shipping_threshold,
            free_shipping_threshold_formatted: crate::modules::utils::format_price::format_price(
                free_shipping_threshold.to_string().parse::<f64>().unwrap_or(0.0),
                &display_currency_str,
            ),
            discounts: discount_descriptions,
            applied_coupon: applied_coupon_string,
            subtotal_formatted: crate::modules::utils::format_price::format_price(
                cart_total.to_string().parse::<f64>().unwrap_or(0.0),
                &display_currency_str,
            ),
            total_discount_formatted: crate::modules::utils::format_price::format_price(
                total_discount.to_string().parse::<f64>().unwrap_or(0.0),
                &display_currency_str,
            ),
            total_formatted: crate::modules::utils::format_price::format_price(
                total.to_string().parse::<f64>().unwrap_or(0.0),
                &display_currency_str,
            ),
        };

        // Adım 9: dry_run moduna göre DB yazma veya raporlama
        if dry_run {
            // dry_run=true: Yalnızca hesaplama yapılır, cart_discounts tablosuna yazılmaz!
            let previews: Vec<CartDiscountPreview> = discounts
                .iter()
                .map(|d| CartDiscountPreview {
                    campaign_id: d.campaign_id,
                    scenario_type: d.scenario_type.to_string(),
                    discount_type: d.discount_type.clone(),
                    scope: d.scope.clone(),
                    cart_item_id: d.cart_item_id,
                    amount: d.amount,
                    currency: d.currency.clone(),
                    description: d.description.clone(),
                })
                .collect();

            let report = DryRunReport {
                evaluated_campaigns: eval_details,
                applied: applied_details,
                skipped: skipped_details,
                would_write: previews,
            };

            // dry_run=true olduğu için değişiklikleri geri alıyoruz (gerçi bir şey değişmedi ama transaction'ı kapatmak için)
            txn.rollback().await.map_err(|e| format!("Transaction rollback hatası: {}", e))?;

            Ok(EvaluateResult {
                dry_run: true,
                summary,
                dry_run_report: Some(report),
            })
        } else {
            // dry_run=false: Sepetin eski cart_discounts kayıtlarını sil, yenilerini yaz
            // Adım 9.1: Mevcut indirimleri temizle (transaction içinde güvenli)
            let _delete_result = cart_discount::Entity::delete_many()
                .filter(cart_discount::Column::CartId.eq(cart_id))
                .exec(&txn)
                .await
                .map_err(|e| format!("DB error: {}", e))?;

            // Adım 9.2: Yeni hesaplanan indirimleri ekle
            for _j in 0..discounts.len() {
                let d = &discounts[_j];
                let active_model = cart_discount::ActiveModel {
                    cart_id: sea_orm::Set(d.cart_id),
                    campaign_id: sea_orm::Set(d.campaign_id),
                    coupon_id: sea_orm::Set(d.coupon_id),
                    scenario_type: sea_orm::Set(d.scenario_type.to_string()),
                    discount_type: sea_orm::Set(d.discount_type.clone()),
                    scope: sea_orm::Set(d.scope.clone()),
                    cart_item_id: sea_orm::Set(d.cart_item_id),
                    amount: sea_orm::Set(d.amount),
                    currency: sea_orm::Set(d.currency.clone()),
                    description: sea_orm::Set(d.description.clone()),
                    ..Default::default()
                };
                cart_discount::Entity::insert(active_model)
                    .exec(&txn)
                    .await
                    .map_err(|e| format!("DB error: {}", e))?;
            }

            // Adım 10: Tüm değişiklikleri onayla (Commit)
            // Eğer buraya kadar bir hata oluşmadıysa transaction tamamlanır.
            txn.commit().await.map_err(|e| format!("Transaction commit hatası: {}", e))?;

            Ok(EvaluateResult {
                dry_run: false,
                summary,
                dry_run_report: None,
            })
        }
    }
}
