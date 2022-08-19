mod error;
pub use error::Error as E3Error;
mod tls_verifier;

use hyper::client::conn::{Connection as HyperConnection, SendRequest};
use hyper::header::HeaderValue;
use hyper::{Body, Response};
use serde::de::DeserializeOwned;
use serde_json::value::Value;
use tokio_rustls::rustls::{ClientConfig, OwnedTrustAnchor, ServerName};
use tokio_rustls::{client::TlsStream, TlsConnector};

fn get_tls_client_config() -> ClientConfig {
    let config_builder = tokio_rustls::rustls::ClientConfig::builder().with_safe_defaults();
    let mut root_store = tokio_rustls::rustls::RootCertStore::empty();
    root_store.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));
    let mut client_config = config_builder
        .with_root_certificates(root_store)
        .with_no_client_auth();
    let mut dangerous = client_config.dangerous();
    dangerous.set_certificate_verifier(std::sync::Arc::new(tls_verifier::E3CertVerifier));
    client_config
}

#[cfg(not(feature = "enclave"))]
type Connection = tokio::net::TcpStream;
#[cfg(feature = "enclave")]
use tokio_vsock::VsockStream;
#[cfg(feature = "enclave")]
type Connection = tokio_vsock::VsockStream;

pub struct E3Client {
    tls_connector: TlsConnector,
    e3_server_name: ServerName,
}

impl std::default::Default for E3Client {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "enclave"))]
use tokio::net::TcpStream;

use crate::CageContext;

#[cfg(not(feature = "enclave"))]
async fn get_socket() -> Result<Connection, tokio::io::Error> {
    TcpStream::connect(std::net::SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
        shared::ENCLAVE_CRYPTO_PORT,
    ))
    .await
}

#[cfg(feature = "enclave")]
async fn get_socket() -> Result<Connection, tokio::io::Error> {
    VsockStream::connect(shared::PARENT_CID, shared::ENCLAVE_CRYPTO_PORT.into()).await
}

impl E3Client {
    pub fn new() -> Self {
        let tls_config = get_tls_client_config();
        Self {
            tls_connector: TlsConnector::from(std::sync::Arc::new(tls_config)),
            e3_server_name: ServerName::try_from("e3.cages-e3.internal")
                .expect("Hardcoded hostname"),
        }
    }

    fn uri(&self, path: &str) -> String {
        format!("https://e3.cages-e3.internal{}", path)
    }

    async fn get_conn(
        &self,
    ) -> Result<
        (
            SendRequest<hyper::Body>,
            HyperConnection<TlsStream<Connection>, hyper::Body>,
        ),
        E3Error,
    > {
        let client_connection: Connection = get_socket().await?;
        let connection = self
            .tls_connector
            .connect(self.e3_server_name.clone(), client_connection)
            .await?;

        let connection_info = hyper::client::conn::Builder::new()
            .handshake::<TlsStream<Connection>, hyper::Body>(connection)
            .await?;

        Ok(connection_info)
    }

    async fn send<V>(
        &self,
        api_key: V,
        path: &str,
        payload: hyper::Body,
    ) -> Result<Response<Body>, E3Error>
    where
        HeaderValue: TryFrom<V>,
        hyper::http::Error: From<<HeaderValue as TryFrom<V>>::Error>,
    {
        let decrypt_request = hyper::Request::builder()
            .uri(self.uri(path))
            .header("api-key", api_key)
            .method("POST")
            .body(payload)
            .expect("Failed to create request");

        // TODO: connection pooling
        let (mut request_sender, connection) = self.get_conn().await?;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Error in e3 connection: {}", e);
            }
        });

        let response = request_sender.send_request(decrypt_request).await?;
        if !response.status().is_success() {
            return Err(E3Error::FailedRequest(response.status()));
        }

        Ok(response)
    }

    pub async fn decrypt<'a, T, V>(&self, api_key: V, payload: E3Payload<'a>) -> Result<T, E3Error>
    where
        T: DeserializeOwned,
        HeaderValue: TryFrom<V>,
        hyper::http::Error: From<<HeaderValue as TryFrom<V>>::Error>,
    {
        let response = self.send(api_key, "/decrypt", payload.try_into()?).await?;
        self.parse_response(response).await
    }

    pub async fn encrypt<'a, T, V>(&self, api_key: V, payload: E3Payload<'a>) -> Result<T, E3Error>
    where
        T: DeserializeOwned,
        HeaderValue: TryFrom<V>,
        hyper::http::Error: From<<HeaderValue as TryFrom<V>>::Error>,
    {
        let response = self.send(api_key, "/encrypt", payload.try_into()?).await?;
        self.parse_response(response).await
    }

    pub async fn authenticate<'a, V>(
        &self,
        api_key: V,
        payload: E3Payload<'a>,
    ) -> Result<bool, E3Error>
    where
        HeaderValue: TryFrom<V>,
        hyper::http::Error: From<<HeaderValue as TryFrom<V>>::Error>,
    {
        let response = self
            .send(api_key, "/authenticate", payload.try_into()?)
            .await?;

        Ok(response.status().is_success())
    }

    async fn parse_response<T: DeserializeOwned>(&self, res: Response<Body>) -> Result<T, E3Error> {
        let response_body = res.into_body();
        let response_body = hyper::body::to_bytes(response_body).await?;
        Ok(serde_json::from_slice(&response_body)?)
    }
}

pub struct E3Payload<'a> {
    data: Option<&'a Value>,
    context: &'a crate::CageContext,
}

impl<'a> std::convert::From<(&'a Value, &'a CageContext)> for E3Payload<'a> {
    fn from((val, context): (&'a Value, &'a CageContext)) -> Self {
        Self {
            data: Some(val),
            context,
        }
    }
}

impl<'a> std::convert::From<&'a CageContext> for E3Payload<'a> {
    fn from(context: &'a CageContext) -> Self {
        Self {
            data: None,
            context,
        }
    }
}

impl<'a> std::convert::TryInto<hyper::Body> for E3Payload<'a> {
    type Error = E3Error;
    fn try_into(self) -> Result<hyper::Body, E3Error> {
        let object = serde_json::json!({
            "data": self.data,
            "team_uuid": self.context.team_uuid(),
            "app_uuid": self.context.app_uuid(),
        });
        Ok(hyper::Body::from(serde_json::to_vec(&object)?))
    }
}
