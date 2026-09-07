//! HTTP helpers: query encoding and OnlineSim response parsing.

use serde::de::{self, Deserialize, Deserializer, Visitor};
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
            v.trim().parse().map_err(E::custom)
        }

        fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<i64, E> {
            self.visit_str(&v)
        }
    }

    deserializer.deserialize_any(Flex)
}

fn parse_f64_str<E: de::Error>(v: &str) -> std::result::Result<f64, E> {
    let s = v.trim();
    if s.is_empty() {
        return Ok(0.0);
    }
    s.parse().map_err(E::custom)
}

/// Deserialize `f64` from a JSON number **or** decimal string.
///
/// OnlineSim frequently returns money as PDO DECIMAL strings (`"1674.540"`)
/// while mocks / other endpoints use JSON numbers.
pub fn de_f64_flexible<'de, D>(deserializer: D) -> std::result::Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    struct Flex;

    impl<'de> Visitor<'de> for Flex {
        type Value = f64;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("f64 or stringified decimal")
        }

        fn visit_unit<E: de::Error>(self) -> std::result::Result<f64, E> {
            Ok(0.0)
        }

        fn visit_none<E: de::Error>(self) -> std::result::Result<f64, E> {
            Ok(0.0)
        }

        fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<f64, E> {
            Ok(v)
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<f64, E> {
            Ok(v as f64)
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<f64, E> {
            Ok(v as f64)
        }

        fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<f64, E> {
            parse_f64_str(v)
        }

        fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<f64, E> {
            self.visit_str(&v)
        }
    }

    deserializer.deserialize_any(Flex)
}

/// Like [`de_f64_flexible`], but accepts JSON `null` → `None`.
pub fn de_opt_f64_flexible<'de, D>(deserializer: D) -> std::result::Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    struct Flex;

    impl<'de> Visitor<'de> for Flex {
        type Value = Option<f64>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("optional f64 or stringified decimal")
        }

        fn visit_unit<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_none<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D2: Deserializer<'de>>(
            self,
            deserializer: D2,
        ) -> std::result::Result<Self::Value, D2::Error> {
            de_f64_flexible(deserializer).map(Some)
        }

        fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Self::Value, E> {
            Ok(Some(v))
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Self::Value, E> {
            Ok(Some(v as f64))
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Self::Value, E> {
            Ok(Some(v as f64))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Self::Value, E> {
            parse_f64_str(v).map(Some)
        }

        fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Self::Value, E> {
            self.visit_str(&v)
        }
    }

    deserializer.deserialize_any(Flex)
}

fn f64_from_value<E: de::Error>(key: &str, raw: Value) -> std::result::Result<f64, E> {
    match raw {
        Value::Null => Ok(0.0),
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| E::custom(format!("non-finite number for key {key}"))),
        Value::String(s) => parse_f64_str(&s),
        other => Err(E::custom(format!(
            "expected number or string for key {key}, got {other}"
        ))),
    }
}

/// `HashMap<String, f64>` where each value may be a number or decimal string
/// (tariff `days` / `count` / `currency` maps from `number_format`).
///
/// Live API occasionally sends `[]` instead of `{}` for empty maps.
pub fn de_hashmap_f64_flexible<'de, D>(
    deserializer: D,
) -> std::result::Result<std::collections::HashMap<String, f64>, D::Error>
where
    D: Deserializer<'de>,
{
    use std::collections::HashMap;

    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Null => Ok(HashMap::new()),
        Value::Array(_) => Ok(HashMap::new()),
        Value::Object(map) => {
            let mut out = HashMap::new();
            for (key, raw) in map {
                out.insert(key.clone(), f64_from_value(&key, raw)?);
            }
            Ok(out)
        }
        other => Err(de::Error::custom(format!(
            "expected object or array for f64 map, got {other}"
        ))),
    }
}

/// Object map of `T`, or empty JSON array `[]` (PHP empty assoc arrays).
///
/// Live `getNumbersStats` returns `services: []` for some countries (e.g. 356).
pub fn de_hashmap_or_empty_array<'de, T, D>(
    deserializer: D,
) -> std::result::Result<std::collections::HashMap<String, T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    use std::collections::HashMap;
    use serde::de::IntoDeserializer;

    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Null => Ok(HashMap::new()),
        Value::Array(_) => Ok(HashMap::new()),
        Value::Object(map) => {
            let mut out = HashMap::new();
            for (key, raw) in map {
                let item = T::deserialize(raw.into_deserializer()).map_err(de::Error::custom)?;
                out.insert(key, item);
            }
            Ok(out)
        }
        other => Err(de::Error::custom(format!(
            "expected object or array for map, got {other}"
        ))),
    }
}

/// Rent `extend` field: live API sends a price **object** when extension is on,
/// otherwise an empty **array** `[]`.
pub fn de_rent_extend_map<'de, D>(
    deserializer: D,
) -> std::result::Result<std::collections::HashMap<String, f64>, D::Error>
where
    D: Deserializer<'de>,
{
    use std::collections::HashMap;

    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Null => Ok(HashMap::new()),
        Value::Array(_) => Ok(HashMap::new()),
        Value::Object(map) => {
            let mut out = HashMap::new();
            for (k, v) in map {
                let n = match v {
                    Value::Null => 0.0,
                    Value::Number(n) => n.as_f64().ok_or_else(|| {
                        de::Error::custom(format!("non-finite number for key {k}"))
                    })?,
                    Value::String(s) => parse_f64_str::<D::Error>(&s)?,
                    other => {
                        return Err(de::Error::custom(format!(
                            "expected number or string for rent extend key {k}, got {other}"
                        )));
                    }
                };
                out.insert(k, n);
            }
            Ok(out)
        }
        other => Err(de::Error::custom(format!(
            "expected object or array for rent extend, got {other}"
        ))),
    }
}

/// `String` from JSON string or number (free phone lists).
pub fn de_string_flexible<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct Flex;

    impl<'de> Visitor<'de> for Flex {
        type Value = String;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("string or number")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<String, E> {
            Ok(v.to_string())
        }

        fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<String, E> {
            Ok(v)
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<String, E> {
            Ok(v.to_string())
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<String, E> {
            Ok(v.to_string())
        }

        fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<String, E> {
            Ok(v.to_string())
        }
    }

    deserializer.deserialize_any(Flex)
}

/// Optional string from JSON string/number/`null`.
pub fn de_opt_string_flexible<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Value>::deserialize(deserializer)?
        .filter(|v| !v.is_null())
        .map(|v| match v {
            Value::String(s) => Ok(s),
            Value::Number(n) => Ok(n.to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            other => Err(de::Error::custom(format!(
                "expected string or number, got {other}"
            ))),
        })
        .transpose()
}

/// Optional JSON value; treats missing/`null` as `None`, keeps `{}` / `[]` as `Some`.
pub fn de_opt_json_value<'de, D>(deserializer: D) -> std::result::Result<Option<Value>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<Value>::deserialize(deserializer)?.filter(|v| !v.is_null()))
}
