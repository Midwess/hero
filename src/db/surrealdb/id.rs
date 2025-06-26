use crate::db::repository::abstraction::id::DbId;
use surreal_devl::proxy::default::{SurrealDeserializer, SurrealSerializer};
use surrealdb::sql::{Id, Thing, Value};

pub trait SurrealDbId: DbId + SurrealSerializer + SurrealDeserializer {
    fn id(self, tb: &str) -> Thing {
        let value: Value = self.serialize();
        let compound_id = match value {
            Value::Array(arr) => arr,
            _ => panic!("Standard must be array")
        };

        Thing::from((tb, Id::Array(compound_id)))
    }

    fn from_thing(thing: Thing) -> Self {
        match thing.id {
            Id::Array(arr) => SurrealDeserializer::deserialize(&Value::Array(arr)).unwrap(),
            _ => panic!("Standard must be array")
        }
    }
}

impl SurrealDbId for Thing {}

impl DbId for Thing {
    fn soft_deleted(&self) -> bool {
        false
    }

    fn soft_delete(&mut self) {}

    fn soft_restore(&mut self) {}
}

impl SurrealDbId for i64 {}

impl DbId for i64 {
    fn soft_deleted(&self) -> bool {
        false
    }

    fn soft_delete(&mut self) {}

    fn soft_restore(&mut self) {}
}
