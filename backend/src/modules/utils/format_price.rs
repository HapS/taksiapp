/// Fiyatı TRY formatında formatla
/// Örnek: format_price(1234.56) -> "1.234,56 ₺"
pub fn format_price(price: f64, _currency: &str) -> String {
    let is_negative = price < 0.0;
    let abs_price = price.abs();
    let integer_part = abs_price.trunc() as i64;
    let decimal_part = ((abs_price.fract() * 100.0).round() as i64).min(99);

    let formatted_integer = format_with_thousands(integer_part, '.');
    let number_part = format!("{},{:02}", formatted_integer, decimal_part);

    let result = format!("{} ₺", number_part);
    if is_negative {
        format!("-{}", result)
    } else {
        result
    }
}

/// Fiyatı sembol olmadan formatla
pub fn format_price_no_symbol(price: f64, _currency: &str) -> String {
    let is_negative = price < 0.0;
    let abs_price = price.abs();
    let integer_part = abs_price.trunc() as i64;
    let decimal_part = ((abs_price.fract() * 100.0).round() as i64).min(99);

    let formatted_integer = format_with_thousands(integer_part, '.');
    let result = format!("{},{:02}", formatted_integer, decimal_part);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_price() {
        assert_eq!(format_price(1234.56, "TRY"), "1.234,56 ₺");
        assert_eq!(format_price(1000000.0, "TRY"), "1.000.000,00 ₺");
        assert_eq!(format_price(0.99, "TRY"), "0,99 ₺");
    }

    #[test]
    fn test_format_price_negative() {
        assert_eq!(format_price(-1234.56, "TRY"), "-1.234,56 ₺");
    }
}
