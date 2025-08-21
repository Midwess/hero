use core_services::db::surrealdb::connection::SurrealDbConnection;
use core_services::services::errors::Errors;
use core_services::utils::pool::reponse::PoolResponse;
use core_services::utils::pool::request::PoolRequest;
use futures_util::StreamExt;
use std::sync::{Arc, Weak};
use surrealdb::method::Stream;
use surrealdb::opt::Resource;
use surrealdb::{Notification, RecordId, Value};
use surrealdb::sql::Thing;
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;

#[derive(Debug, PartialEq, Clone)]
pub enum LiveId {
    Record(Thing)
}

pub struct LiveStream {
    pub id: LiveId,
    pub rx: watch::Receiver<Option<Notification<Value>>>,
    pub join_handle: Option<JoinHandle<()>>
}

impl LiveStream {
    pub fn new(id: LiveId, mut stream: Stream<Value>) -> Self {
        let (tx, rx) = watch::channel(None);
        let id_cloned = id.clone();
        let join_handle = tokio::spawn(async move {
            log::info!("Live stream started {id_cloned:?}");
            while let Some(value) = stream.next().await {
                let _ = tx.send(Some(value));
            }

            log::info!("Live stream closed {id_cloned:?}");
        });

        Self {
            id,
            rx,
            join_handle: Some(join_handle)
        }
    }

    pub async fn subscribe(&self) -> watch::Receiver<Option<Notification<Value>>> {
        self.rx.clone()
    }
}

impl Drop for LiveStream {
    fn drop(&mut self) {
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.abort();
        }
    }
}

pub struct LiveQuery {
    db: PoolResponse<SurrealDbConnection>,
    streams: Mutex<Vec<Weak<LiveStream>>>
}

impl LiveQuery {
    pub async fn new(db: PoolRequest<SurrealDbConnection>) -> Self {
        Self {
            db: db.retrieve().await.unwrap(),
            streams: Mutex::new(vec![])
        }
    }

    pub async fn subscribe(&self, id: LiveId) -> Result<Arc<LiveStream>, Errors> {
        let mut streams = self.streams.lock().await;
        streams.retain(|stream| stream.upgrade().is_some());

        let existing_stream = streams
            .iter()
            .find_map(|stream| stream.upgrade().and_then(|s| if s.id == id { Some(s) } else { None }));

        if let Some(stream) = existing_stream {
            return Ok(stream);
        }

        drop(streams);

        let live_query_stream = match &id {
            LiveId::Record(id) => {
                let record = RecordId::from_inner(id.clone());
                self.db.select(Resource::from(record)).live().await?
            }
        };

        let mut streams = self.streams.lock().await;
        let live_stream = LiveStream::new(id, live_query_stream);
        let live_stream_arc = Arc::new(live_stream);
        streams.push(Arc::downgrade(&live_stream_arc));
        drop(streams);

        Ok(live_stream_arc)
    }
}
