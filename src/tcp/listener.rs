//! Bind helpers that expose both the socket and its advertised service address.
//!
//! Discovery helpers scan from port 3000 (or a supplied starting port) through
//! 50100 and bind an IPv6 socket with IPv4 compatibility. `SERVICE_HOST`
//! controls the host returned to gateway registration; it defaults to
//! `host.docker.internal`.

use std::env;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};

use log::error;
use socket2::{Domain, Protocol, Socket, Type};
#[cfg(feature = "grpc-server")]
use std::net::TcpListener as StdTcpListener;
use thiserror::Error;
use tokio::net::{TcpListener as TokioTcpListener, UdpSocket};
#[cfg(feature = "grpc-server")]
use tonic::transport::server::TcpIncoming;

/// Result type for listener discovery and binding.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
/// Listener creation failure.
pub enum Error {
    /// Socket creation, configuration, binding, or conversion failed.
    #[error("network listener failure: {0}")]
    Listener(String),
}

#[cfg(feature = "grpc-server")]
/// Bound tonic incoming stream and the address metadata advertised to peers.
pub struct GrpcConnection {
    /// Incoming connections accepted by tonic.
    pub listener: TcpIncoming,
    /// Selected local port.
    pub port: u16,
    /// Host other services should use to reach this listener.
    pub public_host: String,
}

#[cfg(feature = "grpc-server")]
/// Finds the first available dual-stack tonic listener in Hero's port range.
pub async fn find_grpc_listener(from: Option<u16>) -> Result<GrpcConnection> {
    let base_port: u16 = from.unwrap_or(3000); // Starting port
    let max_port: u16 = 50100; // Maximum port to try
    let mut port = base_port;

    let listener = loop {
        let addr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port);

        let socket = match Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP)) {
            Ok(socket) => socket,
            Err(error) => {
                error!(target: "tcp-listener", "Failed to create TCP socket: {}", error);
                return Err(Error::Listener(format!("Failed to create TCP socket: {error}")));
            }
        };

        if let Err(error) = socket.set_only_v6(false) {
            error!(target: "tcp-listener", "Failed to enable dual-stack TCP socket: {}", error);
            return Err(Error::Listener(format!("Failed to enable dual-stack TCP socket: {error}")));
        }

        if let Err(error) = socket.set_nonblocking(true) {
            error!(target: "tcp-listener", "Failed to set TCP socket nonblocking: {}", error);
            return Err(Error::Listener(format!("Failed to set TCP socket nonblocking: {error}")));
        }

        match socket.bind(&addr.into()) {
            Ok(()) => {
                if let Err(error) = socket.listen(512) {
                    error!(target: "tcp-listener", "Failed to listen on TCP socket: {}", error);
                    return Err(Error::Listener(format!("Failed to listen on TCP socket: {error}")));
                }
                // Convert socket2::Socket -> StdTcpListener -> TokioTcpListener -> TcpIncoming
                let std_listener: StdTcpListener = socket.into();
                match TokioTcpListener::from_std(std_listener) {
                    Ok(tokio_listener) => {
                        println!("gRPC server successfully bound to {addr}");
                        break TcpIncoming::from(tokio_listener);
                    }
                    Err(error) => {
                        error!(target: "tcp-listener", "Failed to create TokioTcpListener: {}", error);
                        return Err(Error::Listener(format!("Failed to create TokioTcpListener: {error}")));
                    }
                }
            }
            Err(_) => {
                port += 1;

                if port > max_port {
                    error!(target: "tcp-listener", "No available ports in range {base_port}-{max_port}");
                    return Err(Error::Listener("No port available".to_owned()));
                }
            }
        }
    };

    Ok(GrpcConnection {
        listener,
        port,
        public_host: env::var("SERVICE_HOST").unwrap_or_else(|_| "host.docker.internal".to_owned()),
    })
}

/// Bound Tokio TCP listener and the address metadata advertised to peers.
pub struct TcpConnection {
    /// Nonblocking Tokio listener.
    pub listener: TokioTcpListener,
    /// Selected local port.
    pub port: u16,
    /// Host other services should use to reach this listener.
    pub public_host: String,
}

/// Finds the first available dual-stack TCP listener in Hero's port range.
pub async fn find_tcp_listener(from: Option<u16>) -> Result<TcpConnection> {
    let base_port: u16 = from.unwrap_or(3000); // Starting port
    let max_port: u16 = 50100; // Maximum port to try
    let mut port = base_port;

    let listener = loop {
        let addr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port);

        let socket = match Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP)) {
            Ok(socket) => socket,
            Err(error) => {
                error!(target: "tcp-listener", "Failed to create TCP socket: {}", error);
                return Err(Error::Listener(format!("Failed to create TCP socket: {error}")));
            }
        };

        if let Err(error) = socket.set_only_v6(false) {
            error!(target: "tcp-listener", "Failed to enable dual-stack TCP socket: {}", error);
            return Err(Error::Listener(format!("Failed to enable dual-stack TCP socket: {error}")));
        }

        if let Err(error) = socket.set_nonblocking(true) {
            error!(target: "tcp-listener", "Failed to set TCP socket nonblocking: {}", error);
            return Err(Error::Listener(format!("Failed to set TCP socket nonblocking: {error}")));
        }

        match socket.bind(&addr.into()) {
            Ok(()) => {
                if let Err(error) = socket.listen(512) {
                    error!(target: "tcp-listener", "Failed to listen on TCP socket: {}", error);
                    return Err(Error::Listener(format!("Failed to listen on TCP socket: {error}")));
                }
                match TokioTcpListener::from_std(socket.into()) {
                    Ok(listener) => {
                        println!("TCP server successfully bound to {addr}");
                        break listener;
                    }
                    Err(error) => {
                        error!(target: "tcp-listener", "Failed to create TokioTcpListener: {}", error);
                        return Err(Error::Listener(format!("Failed to create TokioTcpListener: {error}")));
                    }
                }
            }
            Err(_) => {
                port += 1;

                if port > max_port {
                    error!(
                        target: "tcp-listener",
                        "No available ports in range {base_port}-{max_port}"
                    );
                    return Err(Error::Listener("No port available".to_owned()));
                }
            }
        }
    };

    Ok(TcpConnection {
        listener,
        port,
        public_host: env::var("SERVICE_HOST").unwrap_or_else(|_| "host.docker.internal".to_owned()),
    })
}

/// Binds a dual-stack TCP listener to exactly one port.
///
/// Unlike [`find_tcp_listener`], this never scans forward when the port is busy.
pub async fn bind_tcp_listener(port: u16) -> Result<TcpConnection> {
    let addr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port);

    let socket = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP)).map_err(|e| {
        error!(target: "tcp-listener", "Failed to create TCP socket: {}", e);
        Error::Listener(format!("Failed to create TCP socket: {e}"))
    })?;

    if let Err(error) = socket.set_only_v6(false) {
        error!(target: "tcp-listener", "Failed to enable dual-stack TCP socket: {}", error);
        return Err(Error::Listener(format!("Failed to enable dual-stack TCP socket: {error}")));
    }

    socket.bind(&addr.into()).map_err(|e| {
        error!(target: "tcp-listener", "Failed to bind to port {}: {}", port, e);
        Error::Listener(format!("Port {} already in use", port))
    })?;

    if let Err(error) = socket.listen(512) {
        error!(target: "tcp-listener", "Failed to listen on TCP socket: {}", error);
        return Err(Error::Listener(format!("Failed to listen on TCP socket: {error}")));
    }

    let listener = TokioTcpListener::from_std(socket.into()).map_err(|e| {
        error!(target: "tcp-listener", "Failed to wrap TCP socket: {}", e);
        Error::Listener(format!("Failed to wrap TCP socket: {e}"))
    })?;

    println!("TCP server successfully bound to {addr}");

    Ok(TcpConnection {
        listener,
        port,
        public_host: env::var("SERVICE_HOST").unwrap_or_else(|_| "host.docker.internal".to_owned()),
    })
}

/// Bound Tokio UDP socket and the address metadata advertised to peers.
pub struct UdpConnection {
    /// Nonblocking Tokio UDP socket.
    pub socket: UdpSocket,
    /// Selected local port.
    pub port: u16,
    /// Host other services should use to reach this socket.
    pub public_host: String,
}

/// Finds the first available dual-stack UDP socket in Hero's port range.
pub async fn find_udp_socket(from: Option<u16>) -> Result<UdpConnection> {
    let base_port: u16 = from.unwrap_or(3000); // Starting port
    let max_port: u16 = 50100; // Maximum port to try
    let mut port = base_port;

    let socket = loop {
        let addr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port);

        let socket = match Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP)) {
            Ok(socket) => socket,
            Err(error) => {
                error!(target: "udp-listener", "Failed to create UDP socket: {}", error);
                return Err(Error::Listener(format!("Failed to create UDP socket: {error}")));
            }
        };

        if let Err(error) = socket.set_only_v6(false) {
            error!(target: "udp-listener", "Failed to enable dual-stack UDP socket: {}", error);
            return Err(Error::Listener(format!("Failed to enable dual-stack UDP socket: {error}")));
        }

        if let Err(error) = socket.set_nonblocking(true) {
            error!(target: "udp-listener", "Failed to set UDP socket nonblocking: {}", error);
            return Err(Error::Listener(format!("Failed to set UDP socket nonblocking: {error}")));
        }

        match socket.bind(&addr.into()) {
            Ok(()) => {
                println!("UDP successfully bound to {addr}");
                match UdpSocket::from_std(socket.into()) {
                    Ok(socket) => break socket,
                    Err(error) => {
                        error!(target: "udp-listener", "Failed to wrap UDP socket: {}", error);
                        return Err(Error::Listener(format!("Failed to wrap UDP socket: {error}")));
                    }
                }
            }
            Err(_) => {
                port += 1;

                if port > max_port {
                    error!(target: "udp-listener", "No available ports in range {base_port}-{max_port}");
                    return Err(Error::Listener("No port available".to_owned()));
                }
            }
        }
    };

    Ok(UdpConnection {
        socket,
        port,
        public_host: env::var("SERVICE_HOST").unwrap_or_else(|_| "host.docker.internal".to_owned()),
    })
}
