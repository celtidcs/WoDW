//! Capa TLS sobre flujos Tor para destinos `https`.

use crate::error::{ErrorApp, Resultado};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_native_tls::TlsStream;

/// Negocia TLS sobre `flujo` verificando el certificado para `host`.
///
/// # Errors
/// [`ErrorApp::TiempoAgotado`] si la negociación supera `plazo` y
/// [`ErrorApp::RedArti`] si el certificado o la negociación fallan.
pub async fn conectar_tls<S>(flujo: S, host: &str, plazo: Duration) -> Resultado<TlsStream<S>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let conector = native_tls::TlsConnector::new()
        .map(tokio_native_tls::TlsConnector::from)
        .map_err(|e| ErrorApp::RedArti(format!("no se pudo iniciar TLS: {e}")))?;
    tokio::time::timeout(plazo, conector.connect(host, flujo))
        .await
        .map_err(|_| ErrorApp::TiempoAgotado {
            operacion: "negociación TLS",
            milisegundos: u64::try_from(plazo.as_millis()).unwrap_or(u64::MAX),
        })?
        .map_err(|e| ErrorApp::RedArti(format!("negociación TLS con {host} fallida: {e}")))
}
