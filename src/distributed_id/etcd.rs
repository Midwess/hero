use crate::distributed_id::generator::{DistributedIdError, Worker, MAX_WORKER_ID};
use chrono::Utc;
use etcd_client::{Client, Compare, CompareOp, PutOptions, Txn, TxnOp};
use serde_json::json;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;
use tokio::task::JoinHandle;

/// Configuration for reserving Snowflake worker IDs via etcd.
///
/// Environment variables used by [`EtcdWorkerOptions::from_env`]:
/// - `SNOWFLAKE_ETCD_ENDPOINTS`: comma-separated list of etcd endpoints
/// - `SNOWFLAKE_ETCD_NAMESPACE`: optional namespace prefix (default: `snowflake`)
/// - `SNOWFLAKE_ETCD_LEASE_TTL_SECS`: optional lease TTL override in seconds (default: 30)
#[derive(Clone, Debug)]
pub struct EtcdWorkerOptions {
    pub endpoints: Vec<String>,
    pub namespace: String,
    pub lease_ttl: Duration,
}

impl Default for EtcdWorkerOptions {
    fn default() -> Self {
        Self {
            endpoints: Vec::new(),
            namespace: "snowflake".to_string(),
            lease_ttl: Duration::from_secs(30),
        }
    }
}

impl EtcdWorkerOptions {
    pub fn new(endpoints: Vec<String>) -> Self {
        let mut options = Self { endpoints, ..Default::default() };
        options.normalize();
        options
    }

    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
        self.normalize();
        self
    }

    pub fn lease_ttl(mut self, ttl: Duration) -> Self {
        self.lease_ttl = ttl;
        self
    }

    pub fn from_env() -> Result<Self, DistributedIdError> {
        let endpoints = std::env::var("SNOWFLAKE_ETCD_ENDPOINTS")
            .map_err(|_| DistributedIdError::InvalidConfig("SNOWFLAKE_ETCD_ENDPOINTS must be set".into()))?;
        let endpoints = endpoints
            .split(',')
            .map(|endpoint| endpoint.trim())
            .filter(|endpoint| !endpoint.is_empty())
            .map(|endpoint| endpoint.to_string())
            .collect::<Vec<_>>();

        if endpoints.is_empty() {
            return Err(DistributedIdError::InvalidConfig(
                "SNOWFLAKE_ETCD_ENDPOINTS must include at least one http(s) endpoint".into(),
            ));
        }

        let mut options = Self::new(endpoints);

        if let Ok(namespace) = std::env::var("SNOWFLAKE_ETCD_NAMESPACE") {
            if !namespace.trim().is_empty() {
                options.namespace = namespace;
            }
        }

        if let Ok(ttl) = std::env::var("SNOWFLAKE_ETCD_LEASE_TTL_SECS") {
            let ttl_secs = ttl.parse::<u64>().map_err(|_| {
                DistributedIdError::InvalidConfig("SNOWFLAKE_ETCD_LEASE_TTL_SECS must be a positive integer".into())
            })?;
            options.lease_ttl = Duration::from_secs(ttl_secs.max(1));
        }

        options.normalize();
        options.validate()?;

        Ok(options)
    }

    pub fn keep_alive_interval(&self) -> Duration {
        let secs = (self.lease_ttl.as_secs() / 3).max(1);
        Duration::from_secs(secs)
    }

    pub fn validate(&self) -> Result<(), DistributedIdError> {
        if self.endpoints.is_empty() {
            return Err(DistributedIdError::InvalidConfig(
                "no etcd endpoints configured".into(),
            ));
        }

        if self.lease_ttl.is_zero() {
            return Err(DistributedIdError::InvalidConfig(
                "lease TTL must be greater than zero".into(),
            ));
        }

        // Validate TTL fits in i64 for etcd
        if self.lease_ttl.as_secs() > i64::MAX as u64 {
            return Err(DistributedIdError::InvalidConfig(
                format!("lease TTL too large, must be <= {} seconds", i64::MAX)
            ));
        }

        Ok(())
    }

    fn normalize(&mut self) {
        let endpoints = self
            .endpoints
            .iter()
            .map(|endpoint| endpoint.trim().to_string())
            .filter(|endpoint| !endpoint.is_empty())
            .collect::<Vec<_>>();
        self.endpoints = endpoints;

        let namespace = self.namespace.trim_matches('/').to_string();
        self.namespace = if namespace.is_empty() {
            "snowflake".to_string()
        } else {
            namespace
        };
    }

    fn namespace_prefix(&self) -> String {
        self.namespace.clone()
    }

    fn namespace_with_app(&self, app_name: &str) -> String {
        format!("{}/{}", self.namespace_prefix(), Self::app_segment(app_name))
    }

    fn workers_prefix(&self, app_name: &str) -> String {
        format!("{}/workers", self.namespace_with_app(app_name))
    }

    fn app_segment(app_name: &str) -> String {
        app_name.trim_matches('/').replace('/', "_")
    }
}

pub struct WorkerLeaseGuard {
    client: Client,
    lease_id: i64,
    key: String,
    keep_alive_task: Arc<TokioMutex<Option<JoinHandle<()>>>>
}

impl WorkerLeaseGuard {
    pub async fn new(
        client: Client,
        lease_id: i64,
        key: String,
        keep_alive_interval: Duration
    ) -> Result<Self, DistributedIdError> {
        let mut keep_alive_client = client.clone();
        let (_keeper, mut stream) = keep_alive_client
            .lease_keep_alive(lease_id)
            .await
            .map_err(|err| DistributedIdError::Coordinator(format!(
                "failed to start lease keep-alive: {err}")))?;

        let interval_duration = if keep_alive_interval.is_zero() {
            Duration::from_secs(1)
        } else {
            keep_alive_interval
        };

        let keep_alive_task = tokio::spawn(async move {
            use n0_future::StreamExt;
            
            if let Err(err) = stream.next().await.transpose() {
                log::warn!(target: "distributed-id", "initial etcd lease keep-alive failed: {err:?}");
                return;
            }

            let mut ticker = tokio::time::interval(interval_duration);
            loop {
                ticker.tick().await;

                match stream.next().await {
                    Some(Ok(_)) => {
                        // Keep-alive successful
                    }
                    Some(Err(err)) => {
                        log::warn!(target: "distributed-id", "failed to keep etcd lease alive: {err:?}");
                        break;
                    }
                    None => {
                        log::warn!(target: "distributed-id", "etcd lease keep-alive stream ended unexpectedly");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            client,
            lease_id,
            key,
            keep_alive_task: Arc::new(TokioMutex::new(Some(keep_alive_task)))
        })
    }
}

impl fmt::Debug for WorkerLeaseGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkerLeaseGuard")
            .field("key", &self.key)
            .field("lease_id", &self.lease_id)
            .finish()
    }
}

impl Drop for WorkerLeaseGuard {
    fn drop(&mut self) {
        // Force acquire lock to ensure task is properly aborted
        let handle = Arc::clone(&self.keep_alive_task);
        tokio::spawn(async move {
            if let Some(task) = handle.lock().await.take() {
                task.abort();
            }
        });

        let mut client = self.client.clone();
        let key = self.key.clone();
        let lease_id = self.lease_id;

        tokio::spawn(async move {
            if let Err(err) = client.delete(key.clone(), None).await {
                log::debug!(target: "distributed-id", "failed to delete etcd worker key {key}: {err:?}");
            }

            if let Err(err) = client.lease_revoke(lease_id).await {
                log::debug!(target: "distributed-id", "failed to revoke etcd lease {lease_id}: {err:?}");
            }
        });
    }
}

pub async fn register_worker(
    app_name: &str,
    mut options: EtcdWorkerOptions
) -> Result<(Worker, WorkerLeaseGuard), DistributedIdError> {
    options.normalize();
    options.validate()?;

    let mut client = Client::connect(options.endpoints.clone(), None)
        .await
        .map_err(|err| DistributedIdError::Coordinator(format!(
            "failed to connect to etcd: {err}",
        )))?;

    let ttl_secs = i64::try_from(options.lease_ttl.as_secs().max(1)).map_err(|_| {
        DistributedIdError::InvalidConfig("lease TTL is too large to fit into i64".into())
    })?;

    let lease = client
        .lease_grant(ttl_secs, None)
        .await
        .map_err(|err| DistributedIdError::Coordinator(format!(
            "failed to acquire etcd lease: {err}",
        )))?;
    let lease_id = lease.id();

    let (worker_id, key) = allocate_worker_id(&mut client, app_name, &options, lease_id).await?;

    let guard = WorkerLeaseGuard::new(client.clone(), lease_id, key.clone(), options.keep_alive_interval()).await?;

    let worker = Worker {
        distributed_id: worker_id as u64,
        name: app_name.to_owned(),
        created_at: Utc::now()
    };

    Ok((worker, guard))
}

async fn allocate_worker_id(
    client: &mut Client,
    app_name: &str,
    options: &EtcdWorkerOptions,
    lease_id: i64
) -> Result<(u16, String), DistributedIdError> {
    let prefix = options.workers_prefix(app_name);

    for worker_id in 0..=MAX_WORKER_ID {
        let key = format!("{prefix}/{worker_id}");
        let value = json!({
            "app": app_name,
            "worker_id": worker_id,
            "allocated_at": Utc::now().to_rfc3339()
        })
        .to_string();

        let compare = Compare::create_revision(key.clone(), CompareOp::Equal, 0);
        let put = TxnOp::put(
            key.clone(),
            value,
            Some(PutOptions::new().with_lease(lease_id))
        );
        let txn = Txn::new().when(vec![compare]).and_then(vec![put]);

        let response = client
            .txn(txn)
            .await
            .map_err(|err| DistributedIdError::Coordinator(format!(
                "failed while reserving worker id {worker_id}: {err}",
            )))?;

        if response.succeeded() {
            return Ok((worker_id, key));
        }
    }

    Err(DistributedIdError::WorkerIdsExhausted(
        options.namespace_with_app(app_name),
    ))
}
