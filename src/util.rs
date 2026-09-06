//! HTTP helpers: query encoding and OnlineSim response parsing.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::error::{Error, Result};

/// Encode a serializable value as `application/x-www-form-urlencoded` query pairs.
pub fn to_query_pairs(params: impl Serialize) -> Result<Vec<(String, String)>> {
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
pub fn from_api_value<T: DeserializeOwned>(value: Value) -> Result<T> {
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
