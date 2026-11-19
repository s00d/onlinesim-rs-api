//! Lenient serde helpers for server quirks: numbers sent as strings, PHP empty maps sent as `[]`.

use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Number from a number or a numeric string (`"1442.330"`).
pub(crate) fn num(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok().filter(|n| n.is_finite()),
        _ => None,
    }
}

/// Integer from a number or a numeric string.
pub(crate) fn int(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// String from a string or a number.
pub(crate) fn text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Money: number or decimal string; anything else is `0`.
pub(crate) fn money<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(num(&Value::deserialize(d)?).unwrap_or(0.0))
}

/// Optional number: number or numeric string; `false`, `null` and garbage are `None`.
pub(crate) fn opt_num<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(num(&Value::deserialize(d)?))
}

/// Optional integer: number or numeric string; anything else is `None`.
pub(crate) fn opt_int<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    Ok(int(&Value::deserialize(d)?))
}

/// Integer: number or numeric string; anything else is `0`.
pub(crate) fn int0<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(int(&Value::deserialize(d)?).unwrap_or(0))
}

/// Optional string: string or number.
pub(crate) fn opt_text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(text(&Value::deserialize(d)?))
}

/// String: string or number; anything else is empty.
pub(crate) fn text0<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(text(&Value::deserialize(d)?).unwrap_or_default())
}

/// Map from an object; `[]` (PHP empty map) and `null` are an empty map.
pub(crate) fn map<'de, D, T>(d: D) -> Result<HashMap<String, T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    match Value::deserialize(d)? {
        Value::Object(map) => map
            .into_iter()
            .map(|(k, v)| {
                serde_json::from_value(v)
                    .map(|v| (k, v))
                    .map_err(serde::de::Error::custom)
            })
            .collect(),
        _ => Ok(HashMap::new()),
    }
}

/// Map of numbers (numbers or numeric strings); `[]` and `null` are an empty map.
pub(crate) fn num_map<'de, D: Deserializer<'de>>(d: D) -> Result<HashMap<String, f64>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Object(map) => map
            .into_iter()
            .filter_map(|(k, v)| num(&v).map(|n| (k, n)))
            .collect(),
        _ => HashMap::new(),
    })
}

/// Map of integers (numbers or numeric strings); `[]` and `null` are an empty map.
pub(crate) fn int_map<'de, D: Deserializer<'de>>(d: D) -> Result<HashMap<String, i64>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Object(map) => map
            .into_iter()
            .filter_map(|(k, v)| int(&v).map(|n| (k, n)))
            .collect(),
        _ => HashMap::new(),
    })
}

/// List from an array; anything else is empty.
pub(crate) fn list<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    match Value::deserialize(d)? {
        Value::Array(items) => items
            .into_iter()
            .map(|v| serde_json::from_value(v).map_err(serde::de::Error::custom))
            .collect(),
        _ => Ok(Vec::new()),
    }
}
