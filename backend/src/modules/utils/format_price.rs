use std::collections::HashMap;

/// Para birimi formatı bilgisi
#[derive(Clone)]
pub struct CurrencyFormat {
    pub symbol: &'static str,
    pub symbol_position: SymbolPosition,
    pub decimal_separator: char,
    pub thousands_separator: char,
    pub decimal_places: usize,
}

#[derive(Clone, Copy)]
pub enum SymbolPosition {
    Before,  // $100.00
    After,   // 100,00 ₺
}

/// Para birimi formatlarını döndür
fn get_currency_formats() -> HashMap<&'static str, CurrencyFormat> {
    let mut formats = HashMap::new();
    
    // Türk Lirası - 1.234,56 ₺
    formats.insert("TRY", CurrencyFormat {
        symbol: "₺",
        symbol_position: SymbolPosition::After,
        decimal_separator: ',',
        thousands_separator: '.',
        decimal_places: 2,
    });
    
    // Amerikan Doları - $1,234.56
    formats.insert("USD", CurrencyFormat {
        symbol: "$",
        symbol_position: SymbolPosition::Before,
        decimal_separator: '.',
        thousands_separator: ',',
        decimal_places: 2,
    });
    
    // Euro - €1.234,56 veya 1.234,56 €
    formats.insert("EUR", CurrencyFormat {
        symbol: "€",
        symbol_position: SymbolPosition::After,
        decimal_separator: ',',
        thousands_separator: '.',
        decimal_places: 2,
    });
    
    // İngiliz Sterlini - £1,234.56
    formats.insert("GBP", CurrencyFormat {
        symbol: "£",
        symbol_position: SymbolPosition::Before,
        decimal_separator: '.',
        thousands_separator: ',',
        decimal_places: 2,
    });
    
    // İsviçre Frangı - CHF 1'234.56
    formats.insert("CHF", CurrencyFormat {
        symbol: "CHF",
        symbol_position: SymbolPosition::Before,
        decimal_separator: '.',
        thousands_separator: '\'',
        decimal_places: 2,
    });
    
    // Avustralya Doları - A$1,234.56
    formats.insert("AUD", CurrencyFormat {
        symbol: "A$",
        symbol_position: SymbolPosition::Before,
        decimal_separator: '.',
        thousands_separator: ',',
        decimal_places: 2,
    });
    
    // Kanada Doları - C$1,234.56
    formats.insert("CAD", CurrencyFormat {
        symbol: "C$",
        symbol_position: SymbolPosition::Before,
        decimal_separator: '.',
        thousands_separator: ',',
        decimal_places: 2,
    });
    
    formats
}

/// Fiyatı belirtilen para birimine göre formatla
/// Örnek: format_price(1234.56, "TRY") -> "1.234,56 ₺"
/// Örnek: format_price(1234.56, "USD") -> "$1,234.56"
pub fn format_price(price: f64, currency: &str) -> String {
    let formats = get_currency_formats();
    
    if let Some(format) = formats.get(currency.to_uppercase().as_str()) {
        format_price_with_format(price, format)
    } else {
        // Bilinmeyen para birimi için varsayılan format
        let default_format = CurrencyFormat {
            symbol: "?",
            symbol_position: SymbolPosition::After,
            decimal_separator: ',',
            thousands_separator: '.',
            decimal_places: 2,
        };
        let formatted = format_price_with_format(price, &default_format);
        // Sembolü para birimi koduyla değiştir
        formatted.replace("?", currency)
    }
}

/// Fiyatı sadece sembol ile formatla (para birimi kodu yerine)
/// Örnek: format_price_symbol(1234.56, "TRY") -> "1.234,56 ₺"
#[allow(dead_code)]
pub fn format_price_symbol(price: f64, currency: &str) -> String {
    format_price(price, currency)
}

/// Fiyatı sembol olmadan formatla
/// Örnek: format_price_no_symbol(1234.56, "TRY") -> "1.234,56"
pub fn format_price_no_symbol(price: f64, currency: &str) -> String {
    let formats = get_currency_formats();
    let format = formats.get(currency.to_uppercase().as_str())
        .cloned()
        .unwrap_or_else(|| {
            CurrencyFormat {
                symbol: "",
                symbol_position: SymbolPosition::After,
                decimal_separator: ',',
                thousands_separator: '.',
                decimal_places: 2,
            }
        });
    
    let is_negative = price < 0.0;
    let abs_price = price.abs();
    let integer_part = abs_price.trunc() as i64;
    let decimal_part = ((abs_price.fract() * 10_f64.powi(format.decimal_places as i32)).round() as i64)
        .min(10_i64.pow(format.decimal_places as u32) - 1);

    // Binlik ayırıcı ile formatla
    let formatted_integer = format_with_thousands(integer_part, format.thousands_separator);

    // Sonuç
    let result = if format.decimal_places > 0 {
        format!("{}{}{:0width$}", formatted_integer, format.decimal_separator, decimal_part, width = format.decimal_places)
    } else {
        formatted_integer
    };
    
    if is_negative {
        format!("-{}", result)
    } else {
        result
    }
}

/// Verilen format ile fiyatı formatla
fn format_price_with_format(price: f64, format: &CurrencyFormat) -> String {
    let is_negative = price < 0.0;
    let abs_price = price.abs();
    let integer_part = abs_price.trunc() as i64;
    let decimal_part = ((abs_price.fract() * 10_f64.powi(format.decimal_places as i32)).round() as i64)
        .min(10_i64.pow(format.decimal_places as u32) - 1);

    // Binlik ayırıcı ile formatla
    let formatted_integer = format_with_thousands(integer_part, format.thousands_separator);

    // Sayı kısmı
    let number_part = if format.decimal_places > 0 {
        format!("{}{}{:0width$}", formatted_integer, format.decimal_separator, decimal_part, width = format.decimal_places)
    } else {
        formatted_integer
    };

    // Sembol pozisyonuna göre birleştir
    let result = match format.symbol_position {
        SymbolPosition::Before => {
            if format.symbol.len() > 1 {
                // CHF gibi uzun semboller için boşluk ekle
                format!("{} {}", format.symbol, number_part)
            } else {
                format!("{}{}", format.symbol, number_part)
            }
        },
        SymbolPosition::After => format!("{} {}", number_part, format.symbol),
    };
    
    if is_negative {
        format!("-{}", result)
    } else {
        result
    }
}

/// Sayıyı binlik ayırıcı ile formatla
fn format_with_thousands(number: i64, separator: char) -> String {
    let mut result = String::new();
    let mut count = 0;
    let number_str = number.to_string();

    for c in number_str.chars().rev() {
        if count % 3 == 0 && count != 0 {
            result.push(separator);
        }
        result.push(c);
        count += 1;
    }
    
    result.chars().rev().collect()
}

/// Eski fonksiyon - geriye uyumluluk için
// pub fn format_price_tl(price: f64) -> String {
//     format_price(price, "TRY")
// }

/// Para birimi sembolünü al
#[allow(dead_code)]
pub fn get_currency_symbol(currency: &str) -> String {
    match currency.to_uppercase().as_str() {
        "TRY" => "₺".to_string(),
        "USD" => "$".to_string(),
        "EUR" => "€".to_string(),
        "GBP" => "£".to_string(),
        "CHF" => "CHF".to_string(),
        "AUD" => "A$".to_string(),
        "CAD" => "C$".to_string(),
        _ => currency.to_string(), // Bilinmeyen para birimi için kodu döndür
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_price_try() {
        assert_eq!(format_price(1234.56, "TRY"), "1.234,56 ₺");
        assert_eq!(format_price(1000000.0, "TRY"), "1.000.000,00 ₺");
        assert_eq!(format_price(0.99, "TRY"), "0,99 ₺");
    }

    #[test]
    fn test_format_price_usd() {
        assert_eq!(format_price(1234.56, "USD"), "$1,234.56");
        assert_eq!(format_price(1000000.0, "USD"), "$1,000,000.00");
    }

    #[test]
    fn test_format_price_eur() {
        assert_eq!(format_price(1234.56, "EUR"), "1.234,56 €");
    }

    #[test]
    fn test_format_price_negative() {
        assert_eq!(format_price(-1234.56, "TRY"), "-1.234,56 ₺");
    }
}
