//! Cliente HTTP/1.1 endurecido y normalizado para navegación anónima sobre Tor.
//!
//! Opera sobre cualquier flujo `AsyncRead + AsyncWrite` (flujo Tor en claro o
//! envuelto en TLS). Aplica normalización de cabeceras, retardo aleatorio
//! (*jitter*), plazos en toda lectura y escritura, y límites de tamaño
//! comprobados antes de reservar memoria.

mod cuerpo;
mod lector;
mod peticion;
mod respuesta;

pub use peticion::{MetodoHttp, PeticionHttp};
pub use respuesta::RespuestaHttp;

use crate::configuracion::ConfiguracionRedHttp;
use crate::error::{ErrorApp, Resultado};
use crate::ipc::mensajes::FamiliaMedio;
use cuerpo::{leer_cuerpo, LimitesCuerpo};
use lector::LectorAcotado;
use rand::Rng;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

/// Separador entre cabeceras y cuerpo.
const FIN_CABECERAS: &[u8] = b"\r\n\r\n";

/// Espera un retardo aleatorio dentro de `[jitter_min_ms, jitter_max_ms]`.
pub async fn aplicar_jitter(red: &ConfiguracionRedHttp) {
    if red.jitter_max_ms == 0 {
        return;
    }
    let milisegundos = rand::thread_rng().gen_range(red.jitter_min_ms..=red.jitter_max_ms);
    tokio::time::sleep(Duration::from_millis(milisegundos)).await;
}

/// Ejecuta una petición HTTP sobre `flujo` con plazos y límites de `red`.
///
/// # Errors
/// [`ErrorApp::TiempoAgotado`] si una lectura o la escritura superan su plazo,
/// [`ErrorApp::LimiteExcedido`] si cabeceras o cuerpo superan sus límites y
/// [`ErrorApp::ProtocoloHttp`] si la respuesta está mal formada.
pub async fn ejecutar_peticion_en_flujo<S>(
    flujo: &mut S,
    peticion: &PeticionHttp,
    red: &ConfiguracionRedHttp,
) -> Resultado<RespuestaHttp>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    aplicar_jitter(red).await;
    let plazo = Duration::from_millis(red.tiempo_espera_lectura_ms);
    enviar(
        flujo,
        &peticion.serializar(red),
        plazo,
        red.tiempo_espera_lectura_ms,
    )
    .await?;

    let mut lector = LectorAcotado::nuevo(flujo, plazo);
    let bloque = lector
        .leer_hasta(FIN_CABECERAS, red.limite_cabeceras_bytes, "cabeceras HTTP")
        .await?;
    let cabecera = respuesta::interpretar_cabecera(&bloque)?;
    let es_medio = cabecera
        .valores("content-type")
        .next()
        .and_then(|t| t.split(';').next())
        .and_then(|t| FamiliaMedio::desde_mime(&t.trim().to_ascii_lowercase()))
        .is_some();
    let limites = LimitesCuerpo {
        cuerpo: if es_medio {
            red.limite_cuerpo_medios_bytes
        } else {
            red.limite_cuerpo_bytes
        },
        linea_chunk: red.limite_linea_chunk_bytes,
        cabeceras: red.limite_cabeceras_bytes,
    };
    let cuerpo = leer_cuerpo(&mut lector, peticion.metodo(), &cabecera, limites).await?;
    Ok(cabecera.con_cuerpo(cuerpo))
}

/// Escribe y vacía la petición dentro del plazo.
async fn enviar<S: AsyncWrite + Unpin>(
    flujo: &mut S,
    bytes: &[u8],
    plazo: Duration,
    milisegundos: u64,
) -> Resultado<()> {
    let escritura = async {
        flujo.write_all(bytes).await?;
        flujo.flush().await
    };
    tokio::time::timeout(plazo, escritura)
        .await
        .map_err(|_| ErrorApp::TiempoAgotado {
            operacion: "escritura HTTP",
            milisegundos,
        })?
        .map_err(ErrorApp::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{duplex, AsyncReadExt};

    /// Configuración sin jitter para pruebas deterministas.
    fn red_prueba() -> ConfiguracionRedHttp {
        ConfiguracionRedHttp {
            jitter_min_ms: 0,
            jitter_max_ms: 0,
            ..Default::default()
        }
    }

    async fn responder(respuesta: &'static [u8]) -> Resultado<RespuestaHttp> {
        let (mut cliente, mut servidor) = duplex(64 * 1024);
        tokio::spawn(async move {
            let mut peticion = [0u8; 2048];
            let _ = servidor.read(&mut peticion).await;
            let _ = servidor.write_all(respuesta).await;
        });
        let peticion = PeticionHttp::get("destino.onion", 80, "/")?;
        ejecutar_peticion_en_flujo(&mut cliente, &peticion, &red_prueba()).await
    }

    #[tokio::test]
    async fn respuesta_con_content_length() {
        let r = responder(b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 13\r\n\r\n<html></html>")
            .await
            .unwrap();
        assert_eq!(r.codigo_estado(), 200);
        assert_eq!(r.cabecera("Content-Type"), Some("text/html"));
        assert_eq!(r.cuerpo(), b"<html></html>");
    }

    #[tokio::test]
    async fn respuesta_chunked_con_extension_y_trailer() {
        let r = responder(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5;x=1\r\nHello\r\n6\r\n World\r\n0\r\nX-T: 1\r\n\r\n")
            .await
            .unwrap();
        assert_eq!(r.cuerpo(), b"Hello World");
    }

    #[tokio::test]
    async fn cuerpo_truncado_es_error() {
        let r = responder(b"HTTP/1.1 200 OK\r\nContent-Length: 50\r\n\r\ncorto").await;
        assert!(matches!(r, Err(ErrorApp::ProtocoloHttp(_))));
    }

    #[tokio::test]
    async fn content_length_contradictorio_es_error() {
        let r =
            responder(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\nab").await;
        assert!(matches!(r, Err(ErrorApp::ProtocoloHttp(_))));
    }

    #[tokio::test]
    async fn sin_encuadre_lee_hasta_el_cierre() {
        let r = responder(b"HTTP/1.0 200 OK\r\n\r\nfin").await.unwrap();
        assert_eq!(r.cuerpo(), b"fin");
    }
}
