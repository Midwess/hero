use std::{path::Path, pin::Pin, process::Stdio, task::{Context, Poll}};
use log::info;
use tokio::process::Command;
use tokio_stream::Stream;
use crate::{
    services::base::Resolve,
    utils::pool::allocator::PoolResourceProvider
};
use super::shell::StdStream;

#[async_trait::async_trait]
pub trait DockerShell: Send + Sync {
    async fn shell<P1, P2>(
        &self,
        image_name: String,
        bind_mounts: Option<Vec<(P1, P2)>>,
        execute_shell: String
    ) -> Resolve<DockerShellRun<P1, P2>> where P1: AsRef<Path> + Unpin + 'async_trait + Send, P2: AsRef<Path> + Unpin + 'async_trait + Send;
}

pub struct DockerClientProvider {}

#[async_trait::async_trait]
impl PoolResourceProvider<DockerClient> for DockerClientProvider {
    async fn new(&self) -> DockerClient {
        DockerClient {}
    }
}

pub struct DockerShellRun<P1, P2> where P1: AsRef<Path> + Unpin, P2: AsRef<Path> + Unpin {
    pub image_name: String,
    pub execute_shell: String,
    pub bind_mounts: Option<Vec<(P1, P2)>>,
    std_stream: Option<StdStream>
}

pub struct DockerClient {}

impl<P1, P2> Stream for DockerShellRun<P1, P2> where P1: AsRef<Path> + Unpin, P2: AsRef<Path> + Unpin {
    type Item = String;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.std_stream.is_none() {
            let mut command = Command::new("docker");
            command.args(&["run", "--network", "none", "--rm", "--cpus", "1.0", "--memory", "200mb", "--stop-timeout", "500"]);
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());

            if let Some(bind_mounts) = &self.bind_mounts {
                for bind_mount in bind_mounts.iter() {
                    command.args(&["-v", format!("{}:/app/{}",
                        AsRef::<Path>::as_ref(&bind_mount.0).to_str().unwrap(),
                        AsRef::<Path>::as_ref(&bind_mount.1).to_str().unwrap(),
                    ).as_str()]);
                }
            };

            if self.execute_shell.is_empty() {
                command.args(&[&self.image_name]);
            }
            else {
                command.args(&[&self.image_name, &self.execute_shell]);
            }

            info!(target: "docker client", "Executing command {:?}", command);
            let handle = command.spawn().unwrap();
            self.as_mut().std_stream = Some(StdStream::from(handle));
        }

        let mut this = self.as_mut();
        let stream = this.std_stream.as_mut().unwrap();
        let ss = Pin::new(stream);
        ss.poll_next(cx)
    }
}

#[async_trait::async_trait]
impl DockerShell for DockerClient {
    async fn shell<P1, P2>(
        &self,
        image_name: String,
        bind_mounts: Option<Vec<(P1, P2)>>,
        execute_shell: String
    ) -> Resolve<DockerShellRun<P1, P2>> where P1: AsRef<Path> + Unpin + 'async_trait + Send, P2: AsRef<Path> + Unpin + 'async_trait + Send {
        Ok(DockerShellRun::<P1, P2> {
            image_name,
            bind_mounts,
            execute_shell,
            std_stream: None
        })
    }
}

