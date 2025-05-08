mod secure;
use axum::{
    extract::WebSocketUpgrade,
    response::IntoResponse,
    routing::get,
    Router,
};
use secure::{load_ssl_config, CertError};
use std::{fmt, net::ToSocketAddrs, path::Path, sync::Arc};
use tokio::{net::TcpListener, net::TcpStream};
use tokio_rustls::TlsAcceptor;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_rustls::server::TlsStream;
use tower::make::Shared;
use hyper::server::conn::Http;
use std::convert::Infallible;
use futures_util::StreamExt;

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
    let config = Arc::new(load_ssl_config(key_path, cert_path).await?);
    let acceptor = TlsAcceptor::from(config.clone());

    let listener = TcpListener::bind(addrs).await?;
    println!("🔒 Listening with TLS...");

    // Axum router with WebSocket example
    let app = Router::new().route("/ws/*path", get(ws_handler));
    let service = Shared::new(app.into_make_service());

    loop {
        let (stream, addr) = listener.accept().await?;
        let acceptor = acceptor.clone();
        let service = service.clone();

        tokio::spawn(async move {
            match acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    if let Err(err) = Http::new()
                        .serve_connection(tls_stream, service)
                        .await
                    {
                        eprintln!("HTTP connection error: {err}");
                    }
                }
                Err(err) => {
                    eprintln!("TLS handshake failed for {addr}: {err}");
                }
            }
        });
    }
}

// WebSocket handler (basic echo)
async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|mut socket| async move {
        while let Some(Ok(msg)) = socket.recv().await {
            if socket.send(msg).await.is_err() {
                break;
            }
        }
    })
}

