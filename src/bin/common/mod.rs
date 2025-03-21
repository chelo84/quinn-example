//! Commonly used code in most examples.

use quinn::{ClientConfig, Endpoint, ServerConfig};
use std::time::Duration;
use std::{error::Error, net::SocketAddr, sync::Arc};
use tokio::signal;
use tokio::sync::mpsc;

/// Constructs a QUIC endpoint configured for use a client only.
///
/// ## Args
///
/// - server_certs: list of trusted certificates.
#[allow(unused)]
pub fn make_client_endpoint(
    bind_addr: SocketAddr,
    server_certs: &[&[u8]],
) -> anyhow::Result<Endpoint, Box<dyn Error>> {
    let client_cfg = configure_client(server_certs)?;
    let mut endpoint = Endpoint::client(bind_addr)?;
    endpoint.set_default_client_config(client_cfg);
    Ok(endpoint)
}

/// Constructs a QUIC endpoint configured to listen for incoming connections on a certain address
/// and port.
///
/// ## Returns
///
/// - a stream of incoming QUIC connections
/// - server certificate serialized into DER format
#[allow(unused)]
pub async fn make_server_endpoint(
    bind_addr: SocketAddr,
) -> Result<(Endpoint, Vec<u8>), Box<dyn Error>> {
    let (server_config, server_cert) = configure_server().await?;
    let endpoint = Endpoint::server(server_config, bind_addr)?;
    Ok((endpoint, server_cert))
}

/// Builds default quinn client config and trusts given certificates.
///
/// ## Args
///
/// - server_certs: a list of trusted certificates in DER format.
fn configure_client(server_certs: &[&[u8]]) -> Result<ClientConfig, Box<dyn Error>> {
    let mut certs = rustls::RootCertStore::empty();
    for cert in server_certs {
        certs.add(&rustls::Certificate(cert.to_vec()))?;
    }

    let client_config = ClientConfig::with_root_certificates(certs);

    // let mut transport_config = TransportConfig::default();
    // transport_config.max_idle_timeout(Some(Duration::from_secs(1_000).try_into()?));
    // client_config.transport_config(Arc::new(transport_config));

    Ok(client_config)
}

/// Returns default server configuration along with its certificate.
async fn configure_server() -> Result<(ServerConfig, Vec<u8>), Box<dyn Error>> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert_der = cert.serialize_der().unwrap();
    let priv_key = cert.serialize_private_key_der();
    let priv_key = rustls::PrivateKey(priv_key);
    let cert_chain = vec![rustls::Certificate(cert_der.clone())];

    let mut server_config = ServerConfig::with_single_cert(cert_chain, priv_key)?;
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0_u8.into());
    transport_config.keep_alive_interval(Some(Duration::from_secs(1)));
    transport_config.max_idle_timeout(Some(Duration::from_secs(5).try_into()?));

    Ok((server_config, cert_der))
}

#[allow(unused)]
pub const ALPN_QUIC_HTTP: &[&[u8]] = &[b"hq-29"];

#[allow(unused)]
pub async fn create_stop_signal() -> (mpsc::Sender<()>, mpsc::Receiver<()>) {
    let (stop_signal_sender, stop_signal_recv) = mpsc::channel(1);
    tokio::spawn({
        let stop_signal_sender = stop_signal_sender.clone();

        async move {
            match signal::ctrl_c().await {
                Ok(()) => {
                    let _ = stop_signal_sender.send(()).await;
                }
                Err(err) => {
                    eprintln!("Unable to listen for shutdown signal: {}", err);
                }
            }
        }
    });

    (stop_signal_sender, stop_signal_recv)
}
