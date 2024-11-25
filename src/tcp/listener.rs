use std::{env, net::SocketAddr};

use core_services::services::{base::Resolve, errors::Errors};
use log::error;
use tonic::transport::server::TcpIncoming;

pub struct TcpConnection {
    pub listener: TcpIncoming,
    pub port: u16,
    pub public_host: String // The host of this service that is available for others to connect
}

pub async fn find_listener() -> Resolve<TcpConnection> {
    let base_port: u16 = 3000; // Starting port
    let max_port: u16 = 50100; // Maximum port to try
    let mut port = base_port;

    let listener = loop {
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().expect("Invalid address");

        match TcpIncoming::new(addr, true, None) {
            Ok(listener) => {
                println!("Server successfully bound to {}", addr);
                break listener;
            }
            Err(_) => {
                port += 1;

                if port > max_port {
                    error!(target: "tcp-listener", "No available ports in range {}-{}", base_port, max_port);
                    return Err(Errors::TcpFailure("No port available".to_owned()));
                }
            }
        }
    };

    Ok(TcpConnection {
        listener,
        port,
        public_host: env::var("DEVLOG_SERVICE_HOST")
            .map(|it| it.parse().expect("DEVLOG_SERVICE_HOST must be string"))
            .unwrap_or("host.docker.internal".to_owned())
    })
}

