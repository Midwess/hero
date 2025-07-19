use crate::db::repository::abstraction::errors::RepositoryError;
use crate::db::repository::abstraction::id::DbId;
use std::fmt::Debug;
use wasm_bindgen::JsValue;
use super::table::convert_large_numbers_to_strings;
use crate::db::repository::abstraction::repository::SendSync;

pub trait IdbId: DbId + SendSync + Debug + serde::Serialize + for<'de> serde::Deserialize<'de> {
    // The default implementation will serialize your struct
    // into an array in the same order of you defining them.
    fn serialize(&self) -> Result<JsValue, RepositoryError> {
        let json_value = serde_json::to_value(self)?;
        let array_json_value = Self::normalize_to_array(json_value);
        let js_value = serde_wasm_bindgen::to_value(&array_json_value)?;
        Ok(js_value)
    }

    fn deserialize(value: JsValue) -> Result<Self, RepositoryError>;

    fn normalize_to_array(value: serde_json::Value) -> serde_json::Value {
        let value  = convert_large_numbers_to_strings(value);
        match value {
            serde_json::Value::Array(arr) => serde_json::Value::Array(arr),
            serde_json::Value::Object(obj) => {
                let values = obj.into_iter().map(|(_, v)| v).collect();
                serde_json::Value::Array(values)
            }
            other => serde_json::Value::Array(vec![other]),
        }
    }
}