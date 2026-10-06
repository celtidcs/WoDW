//! Lectura acotada del cuerpo de una respuesta HTTP/1.1 (RFC 9112 §6).

use super::lector::LectorAcotado;
use super::peticion::MetodoHttp;
use super::respuesta::CabeceraRespuesta;
use crate::error::{ErrorApp, Resultado};
use tokio::io::AsyncRead;

/// Delimitador de línea HTTP.
const CRLF: &[u8] = b"\r\n";
/// Base del tamaño de chunk.
const BASE_HEXADECIMAL: u32 = 16;

/// Límites aplicables a la lectura del cuerpo.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LimitesCuerpo {
    pub(crate) cuerpo: usize,
    pub(crate) linea_chunk: usize,
    pub(crate) cabeceras: usize,
}

/// Lee el cuerpo según el método, el código de estado y las cabeceras de encuadre.
///
/// # Errors
/// [`ErrorApp::LimiteExcedido`] si el cuerpo supera el límite (antes de
/// descargarlo cuando `Content-Length` lo anuncia), [`ErrorApp::ProtocoloHttp`]
/// ante encuadre ambiguo o mal formado, y los errores de lectura.
pub(crate) async fn leer_cuerpo<S: AsyncRead + Unpin>(
    lector: &mut LectorAcotado<'_, S>,
    metodo: MetodoHttp,
    cabecera: &CabeceraRespuesta,
    limites: LimitesCuerpo,
) -> Resultado<Vec<u8>> {
    if metodo == MetodoHttp::Head || sin_cuerpo(cabecera.codigo_estado) {
        return Ok(Vec::new());
    }
    if es_chunked(cabecera)? {
        return leer_chunked(lector, limites).await;
    }
    match longitud_declarada(cabecera)? {
        Some(longitud) if longitud > limites.cuerpo => Err(ErrorApp::LimiteExcedido {
            recurso: "cuerpo HTTP (Content-Length)",
            limite: limites.cuerpo,
        }),
        Some(longitud) => lector.leer_exacto(longitud).await,
        None => lector.leer_hasta_fin(limites.cuerpo, "cuerpo HTTP").await,
    }
}

/// Códigos que nunca llevan cuerpo (RFC 9112 §6.3).
fn sin_cuerpo(codigo: u16) -> bool {
    (100..200).contains(&codigo) || codigo == 204 || codigo == 304
}

/// `Transfer-Encoding` solo se admite como `chunked` final; cualquier otra
/// codificación no se puede delimitar con seguridad.
fn es_chunked(cabecera: &CabeceraRespuesta) -> Resultado<bool> {
    let codificaciones: Vec<String> = cabecera
        .valores("transfer-encoding")
        .flat_map(|v| v.split(','))
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .collect();
    match codificaciones.as_slice() {
        [] => Ok(false),
        [unica] if unica == "chunked" => Ok(true),
        _ => Err(ErrorApp::ProtocoloHttp(format!(
            "Transfer-Encoding no soportado: {codificaciones:?}"
        ))),
    }
}

/// `Content-Length` repetido con valores distintos es un vector de *smuggling*.
fn longitud_declarada(cabecera: &CabeceraRespuesta) -> Resultado<Option<usize>> {
    let mut longitud = None;
    for valor in cabecera.valores("content-length") {
        let actual = valor.parse::<usize>().map_err(|_| {
            ErrorApp::ProtocoloHttp(format!(
                "Content-Length inválido: «{}»",
                valor.escape_debug()
            ))
        })?;
        if longitud.is_some_and(|previa| previa != actual) {
            return Err(ErrorApp::ProtocoloHttp(
                "Content-Length duplicado con valores distintos".to_string(),
            ));
        }
        longitud = Some(actual);
    }
    Ok(longitud)
}

/// Decodifica *chunked transfer coding* con aritmética comprobada.
async fn leer_chunked<S: AsyncRead + Unpin>(
    lector: &mut LectorAcotado<'_, S>,
    limites: LimitesCuerpo,
) -> Resultado<Vec<u8>> {
    let mut cuerpo = Vec::new();
    loop {
        let linea = lector
            .leer_hasta(CRLF, limites.linea_chunk, "línea de tamaño de chunk")
            .await?;
        let tamano = interpretar_tamano_chunk(&linea)?;
        if tamano == 0 {
            descartar_trailers(lector, limites.cabeceras).await?;
            return Ok(cuerpo);
        }
        let total = cuerpo
            .len()
            .checked_add(tamano)
            .filter(|total| *total <= limites.cuerpo);
        if total.is_none() {
            return Err(ErrorApp::LimiteExcedido {
                recurso: "cuerpo HTTP (chunked)",
                limite: limites.cuerpo,
            });
        }
        cuerpo.extend_from_slice(&lector.leer_exacto(tamano).await?);
        if lector.leer_exacto(CRLF.len()).await? != CRLF {
            return Err(ErrorApp::ProtocoloHttp(
                "chunk sin CRLF de cierre".to_string(),
            ));
        }
    }
}

/// Interpreta `TAMAÑO[;extensiones]` en hexadecimal.
fn interpretar_tamano_chunk(linea: &[u8]) -> Resultado<usize> {
    let texto = std::str::from_utf8(linea)
        .map_err(|_| ErrorApp::ProtocoloHttp("línea de chunk no UTF-8".to_string()))?;
    let hexadecimal = texto.split(';').next().unwrap_or_default().trim();
    if hexadecimal.is_empty() || !hexadecimal.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ErrorApp::ProtocoloHttp(format!(
            "tamaño de chunk inválido: «{}»",
            hexadecimal.escape_debug()
        )));
    }
    usize::from_str_radix(hexadecimal, BASE_HEXADECIMAL)
        .map_err(|_| ErrorApp::ProtocoloHttp("tamaño de chunk fuera de rango".to_string()))
}

/// Consume las líneas de *trailer* hasta la línea vacía final.
async fn descartar_trailers<S: AsyncRead + Unpin>(
    lector: &mut LectorAcotado<'_, S>,
    limite: usize,
) -> Resultado<()> {
    let mut consumidos = 0usize;
    loop {
        let linea = lector.leer_hasta(CRLF, limite, "trailers HTTP").await?;
        if linea.is_empty() {
            return Ok(());
        }
        consumidos = consumidos.saturating_add(linea.len());
        if consumidos > limite {
            return Err(ErrorApp::LimiteExcedido {
                recurso: "trailers HTTP",
                limite,
            });
        }
    }
}
