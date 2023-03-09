use lite_json::json::JsonValue;
use serde_json::Value as SerdeValue;
use sp_std::{str, vec::Vec};

// NOTE: 当币价低于 0.00001时，将返回None（serde_json其转为科学计数法）
pub fn parse_price(price_str: &str) -> Option<u64> {
    let serde_result: SerdeValue = serde_json::from_str(price_str).ok()?;
    let price = &serde_result["content"]["dbc_price"];
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

#[cfg(test)]
mod test {
    #[test]
    fn parse_price_works() {
        // 将返回None
        let price_str = r#"{"status":1,"code":"10502","msg":"dbc价格获取成功","content":{"dbc_price":0.000009954211,"update_time":null,"percent_change_24h":-17.94}}"#;
        assert_eq!(super::parse_price(price_str), None);

        // OK
        let price_str = r#"{"status":1,"code":"10502","msg":"dbc价格获取成功","content":{"dbc_price":0.00001354211,"update_time":null,"percent_change_24h":-17.94}}"#;
        assert_eq!(super::parse_price(price_str), Some(13));

        let price_str = r#"{"status":1,"code":"10502","msg":"dbc价格获取成功","content":{"dbc_price":0.000063542,"update_time":null,"percent_change_24h":-17.94}}"#;
        assert_eq!(super::parse_price(price_str), Some(63));

        let price_str = r#"{"status":1,"code":"10502","msg":"dbc价格获取成功","content":{"dbc_price":0.006354266,"update_time":null,"percent_change_24h":-17.94}}"#;
        assert_eq!(super::parse_price(price_str), Some(6354));
        let price_str = r#"{"status":1,"code":"10502","msg":"dbc价格获取成功","content":{"dbc_price":0.6354266,"update_time":null,"percent_change_24h":-17.94}}"#;
        assert_eq!(super::parse_price(price_str), Some(635426));

        let price_str = r#"{"status":1,"code":"10502","msg":"dbc价格获取成功","content":{"dbc_price":100.006354266,"update_time":null,"percent_change_24h":-17.94}}"#;
        assert_eq!(super::parse_price(price_str), Some(100006354));
        let price_str = r#"{"status":1,"code":"10502","msg":"dbc价格获取成功","content":{"dbc_price":1000000.006354266,"update_time":null,"percent_change_24h":-17.94}}"#;
        assert_eq!(super::parse_price(price_str), Some(1000000006354));
    }
}
