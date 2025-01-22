use serde::{Deserialize, Serialize};
use surreal_devl::proxy::default::{SurrealDeserializer, SurrealSerializer};
use surreal_devl::surreal_qr::SurrealResponseError;
use surrealdb::sql::Value;

use super::repository::abstraction::id::DbId;

/// === Index ===
/// Standard id will indexing via soft deleted then the order id (snowflake id) then your unique custom key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardID<T>(pub bool, pub u64, pub T)
where
    T: SurrealSerializer + SurrealDeserializer;

impl<T> StandardID<T>
where
    T: SurrealSerializer + SurrealDeserializer
{
    pub fn new(soft_deleted: bool, order_id: u64, custom_key: T) -> Self {
        Self(soft_deleted, order_id, custom_key)
    }

    pub fn gen(custom_key: T) -> Self {
        Self(false, 0, custom_key)
    }
}

impl<T> SurrealSerializer for StandardID<T>
where
    T: SurrealSerializer + SurrealDeserializer
{
    fn serialize(self) -> Value {
        let serialized = self.2.serialize();
        let arr = vec![
            Value::from(self.0),
            Value::from(self.1),
            serialized,
        ];

        Value::from(arr)
    }
}

impl<T> SurrealDeserializer for StandardID<T>
where
    T: SurrealSerializer + SurrealDeserializer
{
    fn deserialize(value: &Value) -> Result<Self, SurrealResponseError> {
        let arr = match value {
            Value::Array(ref arr) => &arr.0,
            _ => panic!("Standard must be array")
        };

        let soft_deleted = arr.first();
        if soft_deleted.is_none() {
            return Err(SurrealResponseError::ExpectedABool);
        }

        let soft_deleted: bool = SurrealDeserializer::deserialize(soft_deleted.unwrap())?;

        let order_id = arr.get(1);
        if order_id.is_none() {
            return Err(SurrealResponseError::ExpectedANumber);
        }

        let order_id: u64 = SurrealDeserializer::deserialize(order_id.unwrap())?;

        let custom_key = arr.get(2);
        if custom_key.is_none() {
            return Err(SurrealResponseError::ExpectedAnObject);
        }

        let custom_key: T = SurrealDeserializer::deserialize(custom_key.unwrap())?;

        Ok(Self(soft_deleted, order_id, custom_key))
    }
}

impl<T> DbId for StandardID<T>
where
    T: SurrealSerializer + SurrealDeserializer
{
    fn soft_deleted(&self) -> bool {
        self.0
    }

    fn soft_delete(&mut self) {
        self.0 = true;
    }

    fn soft_restore(&mut self) {
        self.0 = false;
    }
}

/// This id will not force the #2 to be unique
/// It will be used to identify the category of the record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryID<T>(pub bool, pub T, pub u64)
where
    T: SurrealSerializer + SurrealDeserializer;

impl<T> SurrealSerializer for CategoryID<T>
where
    T: SurrealSerializer + SurrealDeserializer
{
    fn serialize(self) -> Value {
        let serialized = self.2.serialize();
        let arr = vec![
            Value::from(self.0),
            self.1.serialize(),
            serialized,
        ];

        Value::from(arr)
    }
}

impl<T> SurrealDeserializer for CategoryID<T>
where
    T: SurrealSerializer + SurrealDeserializer
{
    fn deserialize(value: &Value) -> Result<Self, SurrealResponseError> {
        match value {
            Value::Array(ref arr) => {
                if arr.0.len() != 3 {
                    return Err(SurrealResponseError::NumberOfFieldOfLengthOfDbValueNotMatchLengthOfEnum);
                }

                let soft_deleted = arr.first();
                if soft_deleted.is_none() {
                    return Err(SurrealResponseError::ExpectedABool);
                }

                let soft_deleted: bool = SurrealDeserializer::deserialize(soft_deleted.unwrap())?;

                let custom_key = arr.get(1);
                if custom_key.is_none() {
                    return Err(SurrealResponseError::ExpectedAnObject);
                }

                let custom_key: T = SurrealDeserializer::deserialize(custom_key.unwrap())?;

                let order_id = arr.get(2);
                if order_id.is_none() {
                    return Err(SurrealResponseError::ExpectedANumber);
                }

                let order_id: u64 = SurrealDeserializer::deserialize(order_id.unwrap())?;

                Ok(Self(soft_deleted, custom_key, order_id))
            }
            _ => Err(SurrealResponseError::ExpectedAnArray)
        }
    }
}

impl<T> DbId for CategoryID<T>
where
    T: SurrealSerializer + SurrealDeserializer
{
    fn soft_deleted(&self) -> bool {
        self.0
    }

    fn soft_delete(&mut self) {
        self.0 = true;
    }

    fn soft_restore(&mut self) {
        self.0 = false;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpecializeID<T, E>(pub bool, pub T, pub E)
where
    T: SurrealSerializer + SurrealDeserializer,
    E: SurrealSerializer + SurrealDeserializer;

impl<T, E> SurrealSerializer for SpecializeID<T, E>
where
    T: SurrealSerializer + SurrealDeserializer,
    E: SurrealSerializer + SurrealDeserializer
{
    fn serialize(self) -> Value {
        let serialized = self.2.serialize();
        let arr = vec![
            Value::from(self.0),
            self.1.serialize(),
            serialized,
        ];

        Value::from(arr)
    }
}

impl<T, E> SurrealDeserializer for SpecializeID<T, E>
where
    T: SurrealSerializer + SurrealDeserializer,
    E: SurrealSerializer + SurrealDeserializer
{
    fn deserialize(value: &Value) -> Result<Self, SurrealResponseError> {
        match &value {
            Value::Array(ref arr) => {
                let soft_deleted = arr.first();
                if soft_deleted.is_none() {
                    return Err(SurrealResponseError::ExpectedABool);
                }

                let soft_deleted: bool = SurrealDeserializer::deserialize(soft_deleted.unwrap())?;

                let custom_key_1 = arr.get(1);
                if custom_key_1.is_none() {
                    return Err(SurrealResponseError::ExpectedAnObject);
                }

                let custom_key_1: T = SurrealDeserializer::deserialize(custom_key_1.unwrap())?;

                let custom_key_2 = arr.get(2);
                if custom_key_2.is_none() {
                    return Err(SurrealResponseError::ExpectedAnObject);
                }

                let custom_key_2: E = SurrealDeserializer::deserialize(custom_key_2.unwrap())?;

                Ok(Self(soft_deleted, custom_key_1, custom_key_2))
            }
            _ => Err(SurrealResponseError::ExpectedAnArray)
        }
    }
}

impl<T, E> DbId for SpecializeID<T, E>
where
    T: SurrealSerializer + SurrealDeserializer,
    E: SurrealSerializer + SurrealDeserializer
{
    fn soft_deleted(&self) -> bool {
        self.0
    }

    fn soft_delete(&mut self) {
        self.0 = true;
    }

    fn soft_restore(&mut self) {
        self.0 = false;
    }
}
