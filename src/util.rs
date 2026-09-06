//! HTTP helpers: query encoding and OnlineSim response parsing.

use serde::de::{self, Deserializer, Visitor};
use serde_json::{Map, Value};
use std::fmt;

use crate::error::{Error, Result};

/// Encode a serializable value as `application/x-www-form-urlencoded` query pairs.
pub fn to_query_pairs(params: impl serde::Serialize) -> Result<Vec<(String, String)>> {
    let value = serde_json::to_value(params)?;
    let mut out = Vec::new();
    match value {
        Value::Object(map) => flatten_object(&mut out, &map, None),
        Value::Null => {}
        other => {
            return Err(Error::Unexpected(format!(
                "query params must be an object, got {other}"
            )));
        }
    }
    Ok(out)
}

fn flatten_object(out: &mut Vec<(String, String)>, map: &Map<String, Value>, prefix: Option<&str>) {
    for (key, value) in map {
        let full_key = match prefix {
            Some(p) => format!("{p}[{key}]"),
            None => key.clone(),
        };
        match value {
            Value::Null => {}
            Value::Bool(b) => out.push((full_key, if *b { "true" } else { "false" }.to_string())),
            Value::Number(n) => out.push((full_key, n.to_string())),
            Value::String(s) => out.push((full_key, s.clone())),
            Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    let k = format!("{full_key}[{i}]");
                    push_scalar(out, k, item);
                }
            }
            Value::Object(nested) => flatten_object(out, nested, Some(&full_key)),
        }
    }
}

fn push_scalar(out: &mut Vec<(String, String)>, key: String, value: &Value) {
    match value {
        Value::Null => {}
        Value::Bool(b) => out.push((key, if *b { "true" } else { "false" }.to_string())),
        Value::Number(n) => out.push((key, n.to_string())),
        Value::String(s) => out.push((key, s.clone())),
        other => out.push((key, other.to_string())),
    }
}

/// Interpret OnlineSim JSON body.
///
/// If the body is an object with a `response` field and it is not `"1"` / `1`,
/// returns an API error. Otherwise strips `response` and returns the rest.
/// Arrays and objects without `response` are returned as-is.
pub fn parse_api_value(mut value: Value) -> Result<Value> {
    let Value::Object(map) = &mut value else {
        return Ok(value);
    };

    let Some(resp) = map.get("response") else {
        return Ok(value);
    };

    let code = match resp {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    };

    if code != "1" {
        return Err(Error::from_api_code(code));
    }

    map.remove("response");
    Ok(value)
}

/// Deserialize a parsed API value into `T`.
pub fn from_api_value<T: serde::de::DeserializeOwned>(value: Value) -> Result<T> {
    Ok(serde_json::from_value(value)?)
}

/// Merge auth/lang/dev_id into a JSON object of request parameters.
pub fn with_auth_params(
    mut params: Value,
    apikey: Option<&str>,
    lang: &str,
    dev_id: Option<i64>,
) -> Value {
    let obj = match params.as_object_mut() {
        Some(o) => o,
        None => {
            params = Value::Object(Map::new());
            params.as_object_mut().expect("object")
        }
    };
    if let Some(key) = apikey {
        obj.insert("apikey".to_string(), Value::String(key.to_string()));
    }
    obj.insert("lang".to_string(), Value::String(lang.to_string()));
    if let Some(id) = dev_id {
        obj.insert("dev_id".to_string(), Value::Number(id.into()));
    }
    params
}

/// Deserialize `i64` from either a JSON number or a decimal string.
pub fn de_i64_flexible<'de, D>(deserializer: D) -> std::result::Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    struct Flex;

    impl<'de> Visitor<'de> for Flex {
        type Value = i64;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("i64 or stringified i64")
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<i64, E> {
            Ok(v)
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<i64, E> {
            i64::try_from(v).map_err(E::custom)
        }

        fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<i64, E> {
            v.parse().map_err(E::custom)
        }

        fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<i64, E> {
            self.visit_str(&v)
        }
    }

    deserializer.deserialize_any(Flex)
}
