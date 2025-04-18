mod request_handler;
mod secure;
use request_handler::handle_connection;
use tokio::{
    net::{TcpListener, TcpStream},
};
use std::{fmt, net::ToSocketAddrs, path::Path, sync::Arc};
use tokio_rustls::TlsAcceptor;

use secure::{load_ssl_config, CertError};

use tokio_rustls::server::TlsStream;


#[derive(Debug)]
pub enum ServerError {
    CertError(CertError),
    IoError(std::io::Error),
}
impl From<CertError> for ServerError {
    fn from(value: CertError) -> Self {
        Self::CertError(value)
    }
}
impl From<std::io::Error> for ServerError {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}
impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CertError(error) => write!(f, "{}", error),
            Self::IoError(error) => write!(f, "{}", error),
        }
    }
}

pub async fn start<A>(key_path: &Path, cert_path: &Path, addrs: A) -> Result<(), ServerError>
where
    A: ToSocketAddrs + tokio::net::ToSocketAddrs,
{
    let server = TcpListener::bind(addrs).await?;
    let config = Arc::new(load_ssl_config(key_path, cert_path).await?);
    //let acceptor = TlsAcceptor::from(config);

    loop {
        match server.accept().await {
            Ok((socket, addr)) => {
                println!("Received connection from {}", addr);
                //let acceptor = acceptor.clone();

                tokio::spawn(async move {
                    handle_connection(socket).await;
                    //match acceptor.accept(socket).await {
                    //    Ok(tls_stream) => {
                    //        handle_connection(tls_stream).await;
                    //    }
                    //    Err(e) => {
                    //        eprintln!("TLS handshake failed: {}", e);
                    //    }
                    //}
                });
            }
            Err(e) => {
                eprintln!("Connection accept failed: {}", e);
            }
        }
    }
}

