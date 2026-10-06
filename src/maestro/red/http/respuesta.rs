//! Interpretación estricta de la cabecera de una respuesta HTTP/1.1.

use crate::error::{ErrorApp, Resultado};

/// Prefijo de versión aceptado en la línea de estado.
const PREFIJO_VERSION_HTTP1: &str = "HTTP/1.";

/// Respuesta HTTP/1.1 descargada y validada en el Proceso Maestro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespuestaHttp {
    codigo_estado: u16,
    razon_estado: String,
    cabeceras: Vec<(String, String)>,
    cuerpo: Vec<u8>,
}

impl RespuestaHttp {
    /// Construye una respuesta ya recibida (útil para dobles de prueba de la
    /// capa de navegación). Los nombres de cabecera se normalizan a minúsculas.
    pub fn nueva(codigo_estado: u16, cabeceras: &[(&str, &str)], cuerpo: Vec<u8>) -> Self {
        Self {
            codigo_estado,
            razon_estado: String::new(),
            cabeceras: cabeceras
                .iter()
                .map(|(k, v)| (k.to_ascii_lowercase(), (*v).to_string()))
                .collect(),
            cuerpo,
        }
    }

    /// Código numérico de estado.
    pub fn codigo_estado(&self) -> u16 {
        self.codigo_estado
    }

    /// Frase de estado (puede estar vacía).
    pub fn razon_estado(&self) -> &str {
        &self.razon_estado
    }

    /// Primer valor de la cabecera `nombre`, sin distinguir mayúsculas.
    pub fn cabecera(&self, nombre: &str) -> Option<&str> {
        self.cabeceras
            .iter()
            .find(|(clave, _)| clave.eq_ignore_ascii_case(nombre))
            .map(|(_, valor)| valor.as_str())
    }

    /// Cuerpo recibido.
    pub fn cuerpo(&self) -> &[u8] {
        &self.cuerpo
    }

    /// Consume la respuesta y devuelve su cuerpo.
    pub fn en_cuerpo(self) -> Vec<u8> {
        self.cuerpo
    }

    /// Indica si el código es una redirección que lleva `Location`.
    pub fn es_redireccion(&self) -> bool {
        matches!(self.codigo_estado, 301 | 302 | 303 | 307 | 308)
    }
}

/// Cabecera interpretada pendiente de recibir el cuerpo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CabeceraRespuesta {
    pub(crate) codigo_estado: u16,
    pub(crate) razon_estado: String,
    pub(crate) cabeceras: Vec<(String, String)>,
}

impl CabeceraRespuesta {
    /// Todos los valores de una cabecera (puede repetirse).
    pub(crate) fn valores<'a>(&'a self, nombre: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.cabeceras
            .iter()
            .filter(move |(clave, _)| clave.eq_ignore_ascii_case(nombre))
            .map(|(_, valor)| valor.as_str())
    }

    /// Une la cabecera con el cuerpo recibido.
    pub(crate) fn con_cuerpo(self, cuerpo: Vec<u8>) -> RespuestaHttp {
        RespuestaHttp {
            codigo_estado: self.codigo_estado,
            razon_estado: self.razon_estado,
            cabeceras: self.cabeceras,
            cuerpo,
        }
    }
}

/// Interpreta el bloque de cabeceras (sin el `\r\n\r\n` final).
///
/// # Errors
/// [`ErrorApp::ProtocoloHttp`] ante UTF-8 inválido, línea de estado mal formada,
/// cabeceras plegadas (obsoletas y usadas para *smuggling*) o sin `:`.
pub(crate) fn interpretar_cabecera(bytes: &[u8]) -> Resultado<CabeceraRespuesta> {
    let texto = std::str::from_utf8(bytes)
        .map_err(|e| ErrorApp::ProtocoloHttp(format!("cabeceras no UTF-8: {e}")))?;
    let mut lineas = texto.split("\r\n");
    let (codigo_estado, razon_estado) =
        interpretar_linea_estado(lineas.next().unwrap_or_default())?;
    let cabeceras = lineas
        .map(interpretar_linea_cabecera)
        .collect::<Resultado<Vec<_>>>()?;
    Ok(CabeceraRespuesta {
        codigo_estado,
        razon_estado,
        cabeceras,
    })
}

/// Interpreta `HTTP/1.x NNN frase`.
fn interpretar_linea_estado(linea: &str) -> Resultado<(u16, String)> {
    let mut partes = linea.splitn(3, ' ');
    let version = partes.next().unwrap_or_default();
    if !version.starts_with(PREFIJO_VERSION_HTTP1) {
        return Err(ErrorApp::ProtocoloHttp(format!(
            "versión no soportada: «{}»",
            version.escape_debug()
        )));
    }
    let codigo_texto = partes.next().unwrap_or_default();
    let codigo = codigo_texto
        .parse::<u16>()
        .ok()
        .filter(|c| (100..=999).contains(c))
        .ok_or_else(|| {
            ErrorApp::ProtocoloHttp(format!(
                "código de estado inválido: «{}»",
                codigo_texto.escape_debug()
            ))
        })?;
    Ok((codigo, partes.next().unwrap_or_default().trim().to_string()))
}

/// Interpreta `Nombre: valor`.
fn interpretar_linea_cabecera(linea: &str) -> Resultado<(String, String)> {
    if linea.starts_with([' ', '\t']) {
        return Err(ErrorApp::ProtocoloHttp(
            "cabecera plegada (obs-fold) rechazada".to_string(),
        ));
    }
    let (nombre, valor) = linea
        .split_once(':')
        .ok_or_else(|| ErrorApp::ProtocoloHttp("línea de cabecera sin «:»".to_string()))?;
    if nombre.is_empty() || nombre.bytes().any(|b| !b.is_ascii_graphic()) {
        return Err(ErrorApp::ProtocoloHttp(
            "nombre de cabecera inválido".to_string(),
        ));
    }
    Ok((nombre.to_ascii_lowercase(), valor.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpreta_estado_y_cabeceras() {
        let c =
            interpretar_cabecera(b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nX-A: 1").unwrap();
        assert_eq!(c.codigo_estado, 200);
        assert_eq!(c.razon_estado, "OK");
        assert_eq!(c.valores("content-type").collect::<Vec<_>>(), ["text/html"]);
    }

    #[test]
    fn rechaza_plegado_y_version_desconocida() {
        assert!(interpretar_cabecera(b"HTTP/1.1 200 OK\r\nA: 1\r\n continuacion").is_err());
        assert!(interpretar_cabecera(b"SPDY/3 200 OK").is_err());
        assert!(interpretar_cabecera(b"HTTP/1.1 abc OK").is_err());
    }
}
