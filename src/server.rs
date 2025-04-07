mod request_handler;
mod secure;
use request_handler::first_handshake;
use tokio::{io::{AsyncReadExt, Error as IoError}, net::{TcpListener, TcpStream}, task};
use std::{fmt, path::Path, sync::Arc};
use tokio_rustls::TlsAcceptor;

use secure::{load_ssl_config, CertError};

pub enum ServerError{
    CertError(CertError),
    IoError(IoError)

}
impl From<CertError> for ServerError {
    fn from(value: CertError) -> Self {
        Self::CertError(value)
    }
}
impl From<IoError> for ServerError {
    fn from(value: IoError) -> Self {
        Self::IoError(value)
    }
}
impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CertError(error)=>write!(f,"{}",error),
            Self::IoError(error)=>write!(f,"{}",error),
        }
    }
}
pub async fn start(key_path:&Path,cert_path:&Path,addrs:&str) -> Result<(), ServerError> {
    // Step 1: Bind the listener to a TCP address
    let server = TcpListener::bind(addrs).await?;

    // Step 2: Load SSL configuration
    let config = Arc::new(load_ssl_config(key_path,cert_path).await?);
    let acceptor = TlsAcceptor::from(config);

    // Step 3: Accept connections in a loop
    loop {
        match server.accept().await {
            Ok((socket, addr)) => {
                println!("Received connection from {}", addr);

                // Step 4: Wrap the incoming socket in an SSL/TLS stream
                let tls_stream = acceptor.accept(socket).await;
                
                match tls_stream {
                    Ok(ssl_socket) => {
                        // Step 5: Spawn a new task to handle each connection asynchronously
                        first_handshake(ssl_socket).await;
                    }
                    Err(e) => {
                        eprintln!("Failed to establish SSL connection: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
            }
        }
    }
}
