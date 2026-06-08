// Stok Geri Yükleme Servisi - İadeler için reduce_stock mantığını yansıtır
use crate::modules::content::models::{content, Content};
use sea_orm::*;
use serde_json::Value;

#[derive(Debug)]
#[allow(dead_code)]
pub enum StockRestorationError {
    ProductNotFound,
    VariantNotFound,
    DatabaseError(DbErr),
    InvalidData,
}

impl From<DbErr> for StockRestorationError {
    fn from(err: DbErr) -> Self {
        StockRestorationError::DatabaseError(err)
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct StockRestorationResult {
    pub product_id: i64,
    pub variant_key: Option<String>,
    pub quantity: i32,
    pub success: bool,
    pub error: Option<String>,
}

/// İade edilen ürün için stoğu geri yükler
///
/// Bu fonksiyon reduce_stock mantığını tam olarak yansıtır ancak stok azaltmak yerine artırır.
/// Aynı varyant eşleştirme stratejisini ve JSON path işlemlerini kullanır.
///
/// # Parametreler
/// - `db`: Veritabanı bağlantısı
/// - `product_id`: Ürün ID'si
/// - `variant_key`: Varyant anahtarı (option_values_display) - None ise ana ürün
/// - `quantity`: Geri yüklenecek miktar
///
/// # Dönen Değer
/// - `Ok(new_stock)`: Başarılı, yeni stok miktarını döndürür
/// - `Err(StockRestorationError)`: Hata oluştu
pub async fn restore_stock(
    db: &DatabaseConnection,
    product_id: i64,
    variant_key: Option<&str>,
    quantity: i32,
) -> Result<i32, StockRestorationError> {
    // Ürünü bul
    let product = Content::find_by_id(product_id)
        .filter(content::Column::ContentType.eq("product"))
        .one(db)
        .await?
        .ok_or(StockRestorationError::ProductNotFound)?;

    let mut product_data = product.data.clone();

    // Debug: Geri yükleme detaylarını yazdır
    println!("🔄 Stok geri yükleme:");
    println!("   - Ürün ID: {}", product_id);
    println!("   - Varyant anahtarı: {:?}", variant_key);
    println!("   - Geri yüklenecek miktar: {}", quantity);

    // Varyant var mı kontrol et
    let new_stock = if let Some(variant_key) = variant_key {
        // Önce product.variants yolunu kontrol et (doğru yol)
        if let Some(product_section) = product_data.get_mut("product") {
            if let Some(variants) = product_section.get("variants").and_then(|v| v.as_array()) {
                if variants.is_empty() {
                    println!("   - Product.variants array boş, ana ürün stoğu kullanılıyor");
                    // Varyant array'i boşsa ana ürün stoğunu kullan
                    restore_product_stock_in_section(&mut product_data, quantity)?
                } else {
                    println!(
                        "   - Product.variants içinde {} adet varyant bulundu",
                        variants.len()
                    );
                    // Varyant stoğunu geri yükle
                    restore_variant_stock_in_section(&mut product_data, variant_key, quantity)?
                }
            } else {
                println!("   - Product.variants field yok, ana ürün stoğu kullanılıyor");
                // Variants field yoksa ana ürün stoğunu kullan
                restore_product_stock_in_section(&mut product_data, quantity)?
            }
        } else {
            println!("   - Product section yok, ana ürün stoğu kullanılıyor");
            // Product section yoksa ana ürün stoğunu kullan
            restore_product_stock(&mut product_data, quantity)?
        }
    } else {
        println!("   - Varyant anahtarı yok, ana ürün stoğu kullanılıyor");
        // Ana ürün stoğunu geri yükle - önce product section'da ara
        if product_data.get("product").is_some() {
            restore_product_stock_in_section(&mut product_data, quantity)?
        } else {
            restore_product_stock(&mut product_data, quantity)?
        }
    };

    // Sadece stok field'ını güncelle - race condition'dan kaçınmak için
    use sea_orm::ConnectionTrait;

    let product_id = product.id;
    let now = chrono::Utc::now();

    if let Some(variant_key) = variant_key {
        // Varyant stoğunu güncelle - PostgreSQL jsonb_set kullan
        db.execute_unprepared(&format!(
            "UPDATE contents SET 
             data = jsonb_set(
                 data, 
                 '{{product,variants}}', 
                 (SELECT jsonb_agg(
                     CASE 
                         WHEN elem->>'option_values_display' = '{}' 
                         THEN jsonb_set(elem, '{{stock}}', '{}'::jsonb)
                         ELSE elem 
                     END
                 ) FROM jsonb_array_elements(data->'product'->'variants') AS elem)
             ),
             updated_at = '{}' 
             WHERE id = {}",
            variant_key.replace("'", "''"), // SQL injection koruması
            new_stock,
            now.format("%Y-%m-%d %H:%M:%S%.6f"),
            product_id
        ))
        .await?;
    } else {
        // Ana ürün stoğunu güncelle
        let path = if product_data.get("product").is_some() {
            "{product,stock}"
        } else {
            "{stock}"
        };

        db.execute_unprepared(&format!(
            "UPDATE contents SET 
             data = jsonb_set(data, '{}', '{}'::jsonb, true),
             updated_at = '{}' 
             WHERE id = {}",
            path,
            new_stock,
            now.format("%Y-%m-%d %H:%M:%S%.6f"),
            product_id
        ))
        .await?;
    }

    println!("   - ✅ Stok başarıyla geri yüklendi: yeni stok = {}", new_stock);
    Ok(new_stock)
}

/// Ana ürün stoğunu geri yükle
fn restore_product_stock(product_data: &mut Value, quantity: i32) -> Result<i32, StockRestorationError> {
    // Mevcut stoğu al
    let current_stock = product_data
        .get("stock")
        .and_then(|s| s.as_i64())
        .unwrap_or(0) as i32;

    println!(
        "   - Ana ürün stoğu: {}, geri yüklenecek: {}",
        current_stock, quantity
    );

    // Yeni stok miktarını hesapla (artış)
    let new_stock = current_stock + quantity;

    // Stoğu güncelle
    product_data["stock"] = Value::Number(serde_json::Number::from(new_stock));

    println!(
        "   - ✅ Ana ürün stoğu güncellendi: {} -> {}",
        current_stock, new_stock
    );
    Ok(new_stock)
}

/// Product section içindeki ana ürün stoğunu geri yükle
fn restore_product_stock_in_section(
    product_data: &mut Value,
    quantity: i32,
) -> Result<i32, StockRestorationError> {
    let product_section = product_data
        .get_mut("product")
        .ok_or(StockRestorationError::ProductNotFound)?;

    // Mevcut stoğu al
    let current_stock = product_section
        .get("stock")
        .and_then(|s| s.as_i64())
        .unwrap_or(0) as i32;

    println!(
        "   - Product section stoğu: {}, geri yüklenecek: {}",
        current_stock, quantity
    );

    // Yeni stok miktarını hesapla (artış)
    let new_stock = current_stock + quantity;

    // Stoğu güncelle
    product_section["stock"] = Value::Number(serde_json::Number::from(new_stock));

    println!(
        "   - ✅ Product section stoğu güncellendi: {} -> {}",
        current_stock, new_stock
    );
    Ok(new_stock)
}

/// Product section içindeki varyant stoğunu geri yükle
fn restore_variant_stock_in_section(
    product_data: &mut Value,
    variant_key: &str,
    quantity: i32,
) -> Result<i32, StockRestorationError> {
    // Product ID'yi önceden al
    let product_id = product_data.get("id").and_then(|id| id.as_i64());

    let product_section = product_data
        .get_mut("product")
        .ok_or(StockRestorationError::ProductNotFound)?;

    // Varyantları al
    let variants = product_section
        .get_mut("variants")
        .and_then(|v| v.as_array_mut())
        .ok_or(StockRestorationError::VariantNotFound)?;

    println!(
        "   - Varyant aranıyor: '{}' anahtarı ile {} varyant içinde",
        variant_key,
        variants.len()
    );

    // İlgili varyantı bul - option_values_display kullanarak
    for (i, variant) in variants.iter_mut().enumerate() {
        if let Some(option_values_display) = variant.get("option_values_display") {
            let display_value = option_values_display.as_str().unwrap_or("");
            println!(
                "     {}. varyant: option_values_display = '{}'",
                i + 1,
                display_value
            );

            // Boşlukları temizle ve büyük/küçük harf duyarsız karşılaştır
            if display_value
                .trim()
                .eq_ignore_ascii_case(variant_key.trim())
            {
                println!(
                    "   - ✅ option_values_display ile eşleşme bulundu! Stok geri yükleniyor..."
                );
                return process_variant_stock_restoration(variant, variant_key, quantity, product_id);
            }
        } else {
            println!("     {}. varyant: option_values_display field yok", i + 1);
        }
    }

    // Eğer tam eşleşme bulunamazsa, alternatif eşleşme stratejileri dene
    println!("   - 🔄 Tam eşleşme bulunamadı, alternatif eşleşme stratejileri deneniyor...");

    // Strategi 1: Sadece sayısal değerleri karşılaştır
    let variant_key_numeric = variant_key
        .chars()
        .filter(|c| c.is_numeric() || *c == '.')
        .collect::<String>();
    if !variant_key_numeric.is_empty() {
        for (i, variant) in variants.iter_mut().enumerate() {
            if let Some(option_values_display) = variant.get("option_values_display") {
                let display_value = option_values_display.as_str().unwrap_or("");
                let display_numeric = display_value
                    .chars()
                    .filter(|c| c.is_numeric() || *c == '.')
                    .collect::<String>();

                if !display_numeric.is_empty() && display_numeric == variant_key_numeric {
                    println!(
                        "   - ✅ Sayısal eşleşme bulundu: '{}' -> '{}' (varyant {})",
                        variant_key,
                        display_value,
                        i + 1
                    );
                    return process_variant_stock_restoration(
                        variant,
                        variant_key,
                        quantity,
                        product_id,
                    );
                }
            }
        }
    }

    // Strategi 2: Substring eşleşmesi (variant_key, display_value içinde geçiyorsa)
    for (i, variant) in variants.iter_mut().enumerate() {
        if let Some(option_values_display) = variant.get("option_values_display") {
            let display_value = option_values_display.as_str().unwrap_or("");

            if !variant_key.trim().is_empty()
                && (display_value.contains(variant_key.trim())
                    || variant_key.trim().contains(display_value))
            {
                println!(
                    "   - ✅ Substring eşleşmesi bulundu: '{}' <-> '{}' (varyant {})",
                    variant_key,
                    display_value,
                    i + 1
                );
                return process_variant_stock_restoration(variant, variant_key, quantity, product_id);
            }
        }
    }

    println!(
        "   - ❌ Product section varyantı bulunamadı: '{}'",
        variant_key
    );
    println!("   - 🔍 Mevcut option_values_display değerleri:");
    for (i, variant) in variants.iter().enumerate() {
        if let Some(display_value) = variant
            .get("option_values_display")
            .and_then(|v| v.as_str())
        {
            println!(
                "       {}. '{}' (uzunluk: {})",
                i + 1,
                display_value,
                display_value.len()
            );
        }
    }

    // Eğer varyant bulunamazsa ve sadece 1 varyant varsa, o varyantı kullan (fallback)
    if variants.len() == 1 {
        println!("   - 🔄 Fallback: Product section'da tek varyant var, onu kullanıyoruz");
        let variant = &mut variants[0];
        return process_variant_stock_restoration(variant, variant_key, quantity, product_id);
    }

    Err(StockRestorationError::VariantNotFound)
}

/// Varyant stok geri yükleme işlemini gerçekleştir (ortak fonksiyon)
fn process_variant_stock_restoration(
    variant: &mut Value,
    variant_key: &str,
    quantity: i32,
    product_id: Option<i64>,
) -> Result<i32, StockRestorationError> {
    // Mevcut stoğu al
    let current_stock = variant.get("stock").and_then(|s| s.as_i64()).unwrap_or(0) as i32;

    println!(
        "   - Mevcut stok: {}, geri yüklenecek: {}",
        current_stock, quantity
    );

    // Yeni stok miktarını hesapla (artış)
    let new_stock = current_stock + quantity;

    // Stoğu güncelle
    variant["stock"] = Value::Number(serde_json::Number::from(new_stock));

    println!(
        "   - ✅ Varyant stoğu güncellendi: {} -> {} (Ürün ID: {}, Varyant: '{}')",
        current_stock, new_stock, product_id.unwrap_or(0), variant_key
    );
    Ok(new_stock)
}

/// Birden fazla sepet öğesi için stoğu geri yükle (toplu geri yükleme)
///
/// Bu fonksiyon birden fazla sepet öğesini işler ve her biri için stoğu geri yükler.
/// Hatalar kaydedilir ancak iade işlemini engellemez (engelleyici olmayan hata yönetimi).
///
/// # Parametreler
/// - `db`: Veritabanı bağlantısı
/// - `cart_items`: Stoku geri yüklenecek sepet öğeleri listesi
///
/// # Dönen Değer
/// - `Ok(results)`: Geri yükleme sonuçları vektörü (her öğe için başarı veya hata)
#[allow(dead_code)]
pub async fn restore_cart_items_stock(
    db: &DatabaseConnection,
    cart_items: Vec<(i64, Option<String>, i32)>, // (product_id, variant_key, quantity)
) -> Result<Vec<StockRestorationResult>, StockRestorationError> {
    let mut results = Vec::new();

    for (product_id, variant_key, quantity) in cart_items {
        println!(
            "🔄 Stok geri yüklüyor: Ürün ID={}, varyant_anahtarı={:?}, miktar={}",
            product_id, variant_key, quantity
        );

        let variant_key_ref = variant_key.as_deref();

        match restore_stock(db, product_id, variant_key_ref, quantity).await {
            Ok(_new_stock) => {
                println!(
                    "✅ Stok geri yüklendi: Ürün ID {} - Miktar: {}",
                    product_id, quantity
                );
                results.push(StockRestorationResult {
                    product_id,
                    variant_key: variant_key.clone(),
                    quantity,
                    success: true,
                    error: None,
                });
            }
            Err(e) => {
                let error_msg = format!("{:?}", e);
                println!(
                    "⚠️ Stok geri yükleme hatası (engelleyici değil): Ürün ID {} - Hata: {}",
                    product_id, error_msg
                );

                // Hatayı kaydet ancak işlemeye devam et (engelleyici olmayan)
                results.push(StockRestorationResult {
                    product_id,
                    variant_key: variant_key.clone(),
                    quantity,
                    success: false,
                    error: Some(error_msg),
                });
            }
        }
    }

    Ok(results)
}
