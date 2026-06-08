// Stock Management Service
use crate::modules::content::models::{content, Content};
use sea_orm::*;
use serde_json::Value;

#[derive(Debug)]
pub enum StockError {
    ProductNotFound,
    VariantNotFound,
    InsufficientStock,
    #[allow(dead_code)]
    DatabaseError(DbErr),
    #[allow(dead_code)]
    InvalidData,
}

impl From<DbErr> for StockError {
    fn from(err: DbErr) -> Self {
        StockError::DatabaseError(err)
    }
}

/// Stok düşürme servisi
///
/// # Parametreler
/// - `db`: Veritabanı bağlantısı
/// - `product_id`: Ürün ID'si
/// - `variant_key`: Varyant anahtarı (option_values_display) - None ise ana ürün
/// - `quantity`: Düşürülecek miktar
///
/// # Dönen Değer
/// - `Ok(remaining_stock)`: Başarılı, kalan stok miktarı
/// - `Err(StockError)`: Hata durumu
pub async fn reduce_stock(
    db: &DatabaseConnection,
    product_id: i64,
    variant_key: Option<&str>,
    quantity: i32,
) -> Result<i32, StockError> {
    // Ürünü bul
    let product = Content::find_by_id(product_id)
        .filter(content::Column::ContentType.eq("product"))
        .one(db)
        .await?
        .ok_or(StockError::ProductNotFound)?;

    let mut product_data = product.data.clone();

    // Debug: Ürün ve varyant bilgilerini yazdır
    println!("🔍 Stok düşürme işlemi:");
    println!("   - Ürün ID: {}", product_id);
    println!("   - Aranan variant_key: {:?}", variant_key);

    // Varyant var mı kontrol et
    let remaining_stock = if let Some(variant_key) = variant_key {
        // Önce product.variants altında ara (doğru yol)
        if let Some(product_section) = product_data.get_mut("product") {
            if let Some(variants) = product_section.get("variants").and_then(|v| v.as_array()) {
                if variants.is_empty() {
                    println!("   - Product.variants array boş, ana ürün stoğuna geçiliyor");
                    // Varyant array'i boşsa ana ürün stoğunu kullan
                    reduce_product_stock_in_section(&mut product_data, quantity)?
                } else {
                    println!(
                        "   - Product.variants içinde {} adet varyant bulundu",
                        variants.len()
                    );
                    // Varyant stoğunu düşür
                    reduce_variant_stock_in_section(&mut product_data, variant_key, quantity)?
                }
            } else {
                println!("   - Product.variants field yok, ana ürün stoğuna geçiliyor");
                // Variants field yoksa ana ürün stoğunu kullan
                reduce_product_stock_in_section(&mut product_data, quantity)?
            }
        } else {
            println!("   - Product section yok, ana ürün stoğuna geçiliyor");
            // Product section yoksa ana ürün stoğunu kullan
            reduce_product_stock(&mut product_data, quantity)?
        }
    } else {
        println!("   - Variant key yok, ana ürün stoğu kullanılıyor");
        // Ana ürün stoğunu düşür - önce product section'da ara
        if product_data.get("product").is_some() {
            reduce_product_stock_in_section(&mut product_data, quantity)?
        } else {
            reduce_product_stock(&mut product_data, quantity)?
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
            remaining_stock,
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
            remaining_stock,
            now.format("%Y-%m-%d %H:%M:%S%.6f"),
            product_id
        ))
        .await?;
    }

    Ok(remaining_stock)
}

/// Ana ürün stoğunu düşür
fn reduce_product_stock(product_data: &mut Value, quantity: i32) -> Result<i32, StockError> {
    // Mevcut stoğu al
    let current_stock = product_data
        .get("stock")
        .and_then(|s| s.as_i64())
        .unwrap_or(0) as i32;

    println!(
        "   - Ana ürün stoğu: {}, düşürülecek: {}",
        current_stock, quantity
    );

    // Stok kontrolü
    if current_stock < quantity {
        println!("   - ❌ Ana ürün stoğu yetersiz!");
        return Err(StockError::InsufficientStock);
    }

    // Yeni stok miktarı
    let new_stock = current_stock - quantity;

    // Stoğu güncelle
    product_data["stock"] = Value::Number(serde_json::Number::from(new_stock));

    // Kritik stok uyarısı
    if new_stock <= 3 {
        println!(
            "🚨 KRİTİK STOK UYARISI: Ürün ID {} - Kalan stok: {} adet",
            product_data
                .get("id")
                .and_then(|id| id.as_i64())
                .unwrap_or(0),
            new_stock
        );
    }

    println!(
        "   - ✅ Ana ürün stoğu güncellendi: {} -> {}",
        current_stock, new_stock
    );
    Ok(new_stock)
}

/// Product section içindeki ana ürün stoğunu düşür
fn reduce_product_stock_in_section(
    product_data: &mut Value,
    quantity: i32,
) -> Result<i32, StockError> {
    let product_section = product_data
        .get_mut("product")
        .ok_or(StockError::ProductNotFound)?;

    // Mevcut stoğu al
    let current_stock = product_section
        .get("stock")
        .and_then(|s| s.as_i64())
        .unwrap_or(0) as i32;

    println!(
        "   - Product section stoğu: {}, düşürülecek: {}",
        current_stock, quantity
    );

    // Stok kontrolü
    if current_stock < quantity {
        println!("   - ❌ Product section stoğu yetersiz!");
        return Err(StockError::InsufficientStock);
    }

    // Yeni stok miktarı
    let new_stock = current_stock - quantity;

    // Stoğu güncelle
    product_section["stock"] = Value::Number(serde_json::Number::from(new_stock));

    // Kritik stok uyarısı
    if new_stock <= 3 {
        println!(
            "🚨 KRİTİK STOK UYARISI: Ürün ID {} - Kalan stok: {} adet",
            product_data
                .get("id")
                .and_then(|id| id.as_i64())
                .unwrap_or(0),
            new_stock
        );
    }

    println!(
        "   - ✅ Product section stoğu güncellendi: {} -> {}",
        current_stock, new_stock
    );
    Ok(new_stock)
}

/// Product section içindeki varyant stoğunu düşür
fn reduce_variant_stock_in_section(
    product_data: &mut Value,
    variant_key: &str,
    quantity: i32,
) -> Result<i32, StockError> {
    // Product ID'yi önceden al
    let product_id = product_data.get("id").and_then(|id| id.as_i64());

    let product_section = product_data
        .get_mut("product")
        .ok_or(StockError::ProductNotFound)?;

    // Varyantları al
    let variants = product_section
        .get_mut("variants")
        .and_then(|v| v.as_array_mut())
        .ok_or(StockError::VariantNotFound)?;

    println!(
        "   - Product section varyant arama: '{}' anahtarı ile {} varyant içinde aranıyor",
        variant_key,
        variants.len()
    );

    // İlgili varyantı bul - option_values_display ile
    for (i, variant) in variants.iter_mut().enumerate() {
        if let Some(option_values_display) = variant.get("option_values_display") {
            let display_value = option_values_display.as_str().unwrap_or("");
            println!(
                "     {}. varyant: option_values_display = '{}'",
                i + 1,
                display_value
            );

            // Trim whitespace and compare case-insensitively
            if display_value
                .trim()
                .eq_ignore_ascii_case(variant_key.trim())
            {
                println!(
                    "   - ✅ Option_values_display ile eşleşme bulundu! Stok kontrol ediliyor..."
                );
                return process_variant_stock_reduction(variant, variant_key, quantity, product_id);
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
                    return process_variant_stock_reduction(
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
                return process_variant_stock_reduction(variant, variant_key, quantity, product_id);
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
                "       {}. '{}' (length: {})",
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
        return process_variant_stock_reduction(variant, variant_key, quantity, product_id);
    }

    Err(StockError::VariantNotFound)
}

/// Varyant stok düşürme işlemini gerçekleştir (ortak fonksiyon)
fn process_variant_stock_reduction(
    variant: &mut Value,
    variant_key: &str,
    quantity: i32,
    product_id: Option<i64>,
) -> Result<i32, StockError> {
    // Mevcut stoğu al
    let current_stock = variant.get("stock").and_then(|s| s.as_i64()).unwrap_or(0) as i32;

    println!(
        "   - Mevcut stok: {}, düşürülecek: {}",
        current_stock, quantity
    );

    // Stok kontrolü
    if current_stock < quantity {
        println!("   - ❌ Yetersiz stok!");
        return Err(StockError::InsufficientStock);
    }

    // Yeni stok miktarı
    let new_stock = current_stock - quantity;

    // Stoğu güncelle
    variant["stock"] = Value::Number(serde_json::Number::from(new_stock));

    // Kritik stok uyarısı
    if new_stock <= 3 {
        println!(
            "🚨 KRİTİK STOK UYARISI: Ürün ID {} - Varyant '{}' - Kalan stok: {} adet",
            product_id.unwrap_or(0),
            variant_key,
            new_stock
        );
    }

    println!(
        "   - ✅ Varyant stoğu güncellendi: {} -> {}",
        current_stock, new_stock
    );
    Ok(new_stock)
}

/// Stok kontrolü (satın alma öncesi)
///
/// # Parametreler
/// - `db`: Veritabanı bağlantısı
/// - `product_id`: Ürün ID'si
/// - `variant_key`: Varyant anahtarı - None ise ana ürün
/// - `requested_quantity`: İstenen miktar
///
/// # Dönen Değer
/// - `Ok(available_stock)`: Mevcut stok miktarı
/// - `Err(StockError)`: Hata durumu
#[allow(dead_code)]
pub async fn check_stock(
    db: &DatabaseConnection,
    product_id: i64,
    variant_key: Option<&str>,
    requested_quantity: i32,
) -> Result<i32, StockError> {
    // Ürünü bul
    let product = Content::find_by_id(product_id)
        .filter(content::Column::ContentType.eq("product"))
        .one(db)
        .await?
        .ok_or(StockError::ProductNotFound)?;

    let product_data = &product.data;

    // Varyant var mı kontrol et
    if let Some(variant_key) = variant_key {
        // Varyant stoğunu kontrol et
        check_variant_stock(product_data, variant_key, requested_quantity)
    } else {
        // Ana ürün stoğunu kontrol et
        check_product_stock(product_data, requested_quantity)
    }
}

/// Ana ürün stok kontrolü
#[allow(dead_code)]
fn check_product_stock(product_data: &Value, requested_quantity: i32) -> Result<i32, StockError> {
    let current_stock = product_data
        .get("stock")
        .and_then(|s| s.as_i64())
        .unwrap_or(0) as i32;

    if current_stock < requested_quantity {
        Err(StockError::InsufficientStock)
    } else {
        Ok(current_stock)
    }
}

/// Varyant stok kontrolü
#[allow(dead_code)]
fn check_variant_stock(
    product_data: &Value,
    variant_key: &str,
    requested_quantity: i32,
) -> Result<i32, StockError> {
    // Varyantları al
    let variants = product_data
        .get("variants")
        .and_then(|v| v.as_array())
        .ok_or(StockError::VariantNotFound)?;

    // İlgili varyantı bul
    for variant in variants {
        if let Some(option_values_display) = variant.get("option_values_display") {
            if option_values_display.as_str() == Some(variant_key) {
                let current_stock =
                    variant.get("stock").and_then(|s| s.as_i64()).unwrap_or(0) as i32;

                if current_stock < requested_quantity {
                    return Err(StockError::InsufficientStock);
                } else {
                    return Ok(current_stock);
                }
            }
        }
    }

    Err(StockError::VariantNotFound)
}

/// Sepetteki tüm ürünlerin stoğunu düşür
///
/// # Parametreler
/// - `db`: Veritabanı bağlantısı
/// - `cart_items`: Sepet öğeleri listesi
///
/// # Dönen Değer
/// - `Ok(())`: Başarılı
/// - `Err(StockError)`: Hata durumu
pub async fn reduce_cart_stock(
    db: &DatabaseConnection,
    cart_items: &[crate::modules::ecommerce::services::cart_service::CartItemResponse],
) -> Result<(), StockError> {
    for item in cart_items {
        // Debug: Sepet öğesi bilgilerini yazdır
        println!(
            "🔍 Stok düşürülecek ürün: ID={}, variant_key={:?}, quantity={}",
            item.product_id, item.variant_key, item.quantity
        );

        // Her ürün için stok düşür
        let variant_key = item.variant_key.as_deref();

        // Debug: variant_key detayları
        if let Some(vkey) = variant_key {
            println!(
                "   - Variant key detayları: '{}' (length: {}, bytes: {:?})",
                vkey,
                vkey.len(),
                vkey.as_bytes()
            );
        } else {
            println!("   - Variant key yok, ana ürün stoğu kullanılacak");
        }

        match reduce_stock(db, item.product_id, variant_key, item.quantity).await {
            Ok(remaining_stock) => {
                println!(
                    "✅ Stok düşürüldü: Ürün ID {} - Miktar: {} - Kalan: {}",
                    item.product_id, item.quantity, remaining_stock
                );
            }
            Err(e) => {
                println!(
                    "❌ Stok düşürme hatası: Ürün ID {} - Hata: {:?}",
                    item.product_id, e
                );

                // Debug: Ürünün gerçek verilerini kontrol et
                if let Ok(Some(product)) = Content::find_by_id(item.product_id)
                    .filter(content::Column::ContentType.eq("product"))
                    .one(db)
                    .await
                {
                    println!("🔍 Ürün verisi debug:");
                    println!("   - Ürün ID: {}", product.id);
                    println!("   - Aranan variant_key: {:?}", variant_key);

                    if let Some(product_section) = product.data.get("product") {
                        if let Some(variants) =
                            product_section.get("variants").and_then(|v| v.as_array())
                        {
                            println!("   - Mevcut varyantlar ({} adet):", variants.len());
                            for (i, variant) in variants.iter().enumerate() {
                                let option_values_display = variant
                                    .get("option_values_display")
                                    .and_then(|d| d.as_str());
                                let stock = variant.get("stock").and_then(|s| s.as_i64());
                                if let Some(display) = option_values_display {
                                    println!("     {}. option_values_display: '{}' (length: {}, bytes: {:?}), stock: {:?}", 
                                             i+1, display, display.len(), display.as_bytes(), stock);
                                } else {
                                    println!(
                                        "     {}. option_values_display: None, stock: {:?}",
                                        i + 1,
                                        stock
                                    );
                                }
                            }
                        } else {
                            println!("   - Product section'da variants array yok");
                        }
                    } else if let Some(variants) =
                        product.data.get("variants").and_then(|v| v.as_array())
                    {
                        println!("   - Root level varyantlar ({} adet):", variants.len());
                        for (i, variant) in variants.iter().enumerate() {
                            let option_values_display = variant
                                .get("option_values_display")
                                .and_then(|d| d.as_str());
                            let stock = variant.get("stock").and_then(|s| s.as_i64());
                            if let Some(display) = option_values_display {
                                println!("     {}. option_values_display: '{}' (length: {}, bytes: {:?}), stock: {:?}", 
                                         i+1, display, display.len(), display.as_bytes(), stock);
                            } else {
                                println!(
                                    "     {}. option_values_display: None, stock: {:?}",
                                    i + 1,
                                    stock
                                );
                            }
                        }
                    } else {
                        println!("   - Hiçbir yerde variants array bulunamadı");
                        println!(
                            "   - Ana ürün stoğu: {:?}",
                            product.data.get("stock").and_then(|s| s.as_i64())
                        );
                    }
                } else {
                    println!("❌ Ürün bulunamadı: ID {}", item.product_id);
                }

                return Err(e);
            }
        }
    }

    Ok(())
}

/// Stok miktarını al (sadece görüntüleme için)
///
/// # Parametreler
/// - `db`: Veritabanı bağlantısı
/// - `product_id`: Ürün ID'si
/// - `variant_key`: Varyant anahtarı - None ise ana ürün
///
/// # Dönen Değer
/// - `Ok(stock_amount)`: Mevcut stok miktarı
/// - `Err(StockError)`: Hata durumu
#[allow(dead_code)]
pub async fn get_stock(
    db: &DatabaseConnection,
    product_id: i64,
    variant_key: Option<&str>,
) -> Result<i32, StockError> {
    // Ürünü bul
    let product = Content::find_by_id(product_id)
        .filter(content::Column::ContentType.eq("product"))
        .one(db)
        .await?
        .ok_or(StockError::ProductNotFound)?;

    let product_data = &product.data;

    // Varyant var mı kontrol et
    if let Some(variant_key) = variant_key {
        // Varyant stoğunu al
        get_variant_stock(product_data, variant_key)
    } else {
        // Ana ürün stoğunu al
        get_product_stock(product_data)
    }
}

/// Ana ürün stok miktarını al
fn get_product_stock(product_data: &Value) -> Result<i32, StockError> {
    let stock = product_data
        .get("stock")
        .and_then(|s| s.as_i64())
        .unwrap_or(0) as i32;

    Ok(stock)
}

/// Varyant stok miktarını al
fn get_variant_stock(product_data: &Value, variant_key: &str) -> Result<i32, StockError> {
    // Varyantları al
    let variants = product_data
        .get("variants")
        .and_then(|v| v.as_array())
        .ok_or(StockError::VariantNotFound)?;

    // İlgili varyantı bul
    for variant in variants {
        if let Some(option_values_display) = variant.get("option_values_display") {
            if option_values_display.as_str() == Some(variant_key) {
                let stock = variant.get("stock").and_then(|s| s.as_i64()).unwrap_or(0) as i32;

                return Ok(stock);
            }
        }
    }

    Err(StockError::VariantNotFound)
}
