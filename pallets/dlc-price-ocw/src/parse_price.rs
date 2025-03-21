use lite_json::json::JsonValue;
use serde_json::Value as SerdeValue;
use sp_std::{str, vec::Vec};

// NOTE: 当币价低于 0.00001时，将返回None（serde_json其转为科学计数法）
pub fn parse_price(price_str: &str) -> Option<u64> {
    let serde_result: SerdeValue = serde_json::from_str(price_str).ok()?;
    let price = &serde_result["content"]["dlc_price"];
    if let SerdeValue::Null = price {
        return None
    }

    // 构造price_json: {"a": 0.0123}
    let mut price_json: Vec<u8> = Vec::new();

    let head: Vec<u8> = r#"{"a":"#.into();
    price_json.extend(head);

    let price: Vec<u8> = serde_json::to_string(price).ok()?.into();

    for i in price.clone() {
        if b'e' == i {
            return None
        }
    }

    price_json.extend(price);

    let tail: Vec<u8> = r#"}"#.into();
    price_json.extend(tail);

    let price_json = str::from_utf8(&price_json).ok()?;
    let price_json = lite_json::parse_json(price_json).ok()?;

    if let JsonValue::Object(obj) = price_json {
        if obj.is_empty() {
            return None
        }

        if let JsonValue::Number(price) = obj[0].1.clone() {
            if price.fraction_length >= 15 {
                return None
            }
            return Some(
                (price.integer as u64).saturating_mul(10_u64.pow(6)).saturating_add(
                    price
                        .fraction
                        .saturating_mul(10_u64.pow(6))
                        .saturating_div(10_u64.pow(price.fraction_length)),
                ),
            )
        }
    }

    None
}