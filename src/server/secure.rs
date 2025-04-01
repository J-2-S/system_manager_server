use std::{fmt, fs::File, io::BufReader, path::Path};
use rustls::{ServerConfig,Error as LsError};
use rustls_pemfile::{certs, pkcs8_private_keys,Error as PemError};
#[derive(Debug)]
pub enum CertError {
    PemError(PemError),
    LsError(LsError),
    IoError(std::io::Error)
}
impl fmt::Display for CertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self{
            Self::PemError(error)=> write!(f,"{:?}",error),
            Self::LsError(error)=> write!(f,"{}",error),
            Self::IoError(error)=> write!(f,"{}",error)
        
        }
    }
}
impl From<PemError> for CertError {
    fn from(value: PemError) -> Self {
        Self::PemError(value)
    }
}
impl From<LsError> for CertError {
    fn from(value: LsError) -> Self {
        Self::LsError(value)
    }
}
impl From<std::io::Error> for CertError {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}

pub async fn load_ssl_config(key_path:&Path,cert_path:&Path)-> Result<ServerConfig,CertError> {
    // Load certificates and key
    let cert_file = &mut BufReader::new(File::open(cert_path).unwrap());
    let key_file = &mut BufReader::new(File::open(key_path).unwrap());
    
    let mut cert_chain = vec![];
    for x in certs(cert_file){
        cert_chain.push(x?);
    
    }
    
    let mut keys = vec![];
    for x in pkcs8_private_keys(key_file){
        keys.push(x?);
    }
    let key = keys.first().unwrap();

    // Configure TLS
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, rustls::pki_types::PrivateKeyDer::Pkcs8(key.clone_key()))?;
    Ok(config)
}

