//! Construcción validada y serialización normalizada de peticiones HTTP/1.1.
//!
//! Todo dato que acaba en la línea de petición o en una cabecera se valida al
//! construir la petición: un CR, LF o carácter de control permitiría inyectar
//! cabeceras o partir la petición en dos (*request smuggling*).

use crate::configuracion::ConfiguracionRedHttp;
use crate::error::{ErrorApp, Resultado};

/// Puerto HTTP por defecto: no se repite en la cabecera `Host`.
const PUERTO_HTTP: u16 = 80;
/// Puerto HTTPS por defecto: no se repite en la cabecera `Host`.
const PUERTO_HTTPS: u16 = 443;

/// Cabeceras fijas idénticas a una navegación de documento en Tor Browser.
const CABECERAS_FIJAS: &str = "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/png,image/jpeg,*/*;q=0.8\r\n\
Accept-Encoding: identity\r\n\
Connection: close\r\n\
Upgrade-Insecure-Requests: 1\r\n\
Sec-Fetch-Dest: document\r\n\
Sec-Fetch-Mode: navigate\r\n\
Sec-Fetch-Site: none\r\n\
Sec-Fetch-User: ?1\r\n";

/// Métodos HTTP soportados.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetodoHttp {
    /// Petición GET estándar para recursos y páginas.
    Get,
    /// Petición HEAD (sin cuerpo en la respuesta).
    Head,
    /// Petición POST para formularios de búsqueda.
    Post,
}

impl MetodoHttp {
    /// Representación textual del método.
    pub fn como_texto(self) -> &'static str {
        match self {
            MetodoHttp::Get => "GET",
            MetodoHttp::Head => "HEAD",
            MetodoHttp::Post => "POST",
        }
    }
}

/// Cuerpo de una petición POST con su tipo de contenido.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CuerpoPeticion {
    tipo_contenido: String,
    datos: Vec<u8>,
}

/// Petición HTTP validada. Sus campos solo se fijan a través de los constructores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeticionHttp {
    metodo: MetodoHttp,
    host: String,
    puerto: u16,
    ruta: String,
    cuerpo: Option<CuerpoPeticion>,
}

impl PeticionHttp {
    /// Construye una petición GET.
    ///
    /// # Errors
    /// [`ErrorApp::EntradaInvalida`] si el host o la ruta contienen caracteres
    /// que permitirían inyectar cabeceras.
    pub fn get(host: &str, puerto: u16, ruta: &str) -> Resultado<Self> {
        Self::nueva(MetodoHttp::Get, host, puerto, ruta, None)
    }

    /// Construye una petición POST con cuerpo.
    ///
    /// # Errors
    /// Los de [`Self::get`] y también si el tipo de contenido no es seguro.
    pub fn post(
        host: &str,
        puerto: u16,
        ruta: &str,
        datos: Vec<u8>,
        tipo_contenido: &str,
    ) -> Resultado<Self> {
        validar_valor_cabecera("content_type", tipo_contenido)?;
        let cuerpo = CuerpoPeticion {
            tipo_contenido: tipo_contenido.to_string(),
            datos,
        };
        Self::nueva(MetodoHttp::Post, host, puerto, ruta, Some(cuerpo))
    }

    /// Construye una petición HEAD.
    ///
    /// # Errors
    /// Los de [`Self::get`].
    pub fn head(host: &str, puerto: u16, ruta: &str) -> Resultado<Self> {
        Self::nueva(MetodoHttp::Head, host, puerto, ruta, None)
    }

    fn nueva(
        metodo: MetodoHttp,
        host: &str,
        puerto: u16,
        ruta: &str,
        cuerpo: Option<CuerpoPeticion>,
    ) -> Resultado<Self> {
        validar_host(host)?;
        let ruta = normalizar_ruta(ruta)?;
        Ok(Self {
            metodo,
            host: host.to_string(),
            puerto,
            ruta,
            cuerpo,
        })
    }

    /// Método de la petición.
    pub fn metodo(&self) -> MetodoHttp {
        self.metodo
    }

    /// Host de destino.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Puerto de destino.
    pub fn puerto(&self) -> u16 {
        self.puerto
    }

    /// Ruta solicitada (siempre empieza por `/`).
    pub fn ruta(&self) -> &str {
        &self.ruta
    }

    /// Serializa la petición en bytes HTTP/1.1 con cabeceras normalizadas.
    pub fn serializar(&self, red: &ConfiguracionRedHttp) -> Vec<u8> {
        let host = if self.puerto == PUERTO_HTTP || self.puerto == PUERTO_HTTPS {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.puerto)
        };
        let mut texto = format!(
            "{} {} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: {}\r\nAccept-Language: {}\r\n{CABECERAS_FIJAS}",
            self.metodo.como_texto(),
            self.ruta,
            red.user_agent,
            red.accept_language,
        );
        if let Some(cuerpo) = &self.cuerpo {
            texto.push_str(&format!(
                "Content-Type: {}\r\nContent-Length: {}\r\n",
                cuerpo.tipo_contenido,
                cuerpo.datos.len()
            ));
        }
        texto.push_str("\r\n");
        let mut bytes = texto.into_bytes();
        if let Some(cuerpo) = &self.cuerpo {
            bytes.extend_from_slice(&cuerpo.datos);
        }
        bytes
    }
}

/// Un host solo puede contener letras, dígitos, guiones y puntos (nombres DNS y onion).
fn validar_host(host: &str) -> Resultado<()> {
    let valido = !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.');
    if !valido {
        return Err(ErrorApp::EntradaInvalida {
            campo: "host",
            motivo: format!(
                "«{}» contiene caracteres no permitidos",
                host.escape_debug()
            ),
        });
    }
    Ok(())
}

/// La ruta debe ser ASCII visible sin espacios: los caracteres restantes
/// deben llegar ya codificados con `%XX`.
fn normalizar_ruta(ruta: &str) -> Resultado<String> {
    if !ruta.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(ErrorApp::EntradaInvalida {
            campo: "ruta",
            motivo: "contiene espacios, controles o caracteres no ASCII sin codificar".to_string(),
        });
    }
    if ruta.starts_with('/') {
        Ok(ruta.to_string())
    } else {
        Ok(format!("/{ruta}"))
    }
}

/// Un valor de cabecera no puede contener caracteres de control (CR, LF…).
fn validar_valor_cabecera(campo: &'static str, valor: &str) -> Resultado<()> {
    if valor.chars().any(char::is_control) {
        return Err(ErrorApp::EntradaInvalida {
            campo,
            motivo: "contiene caracteres de control".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_serializa_cabeceras_normalizadas() {
        let red = ConfiguracionRedHttp::default();
        let texto = String::from_utf8(
            PeticionHttp::get("ahmia.onion", 80, "/search?q=test")
                .unwrap()
                .serializar(&red),
        )
        .unwrap();
        assert!(texto.starts_with("GET /search?q=test HTTP/1.1\r\nHost: ahmia.onion\r\n"));
        assert!(texto.contains(&format!("User-Agent: {}\r\n", red.user_agent)));
        assert!(texto.contains("Accept-Encoding: identity\r\n"));
        assert!(!texto.contains("Referer:"));
        assert!(texto.ends_with("\r\n\r\n"));
    }

    #[test]
    fn post_incluye_cuerpo_y_longitud() {
        let red = ConfiguracionRedHttp::default();
        let bytes = PeticionHttp::post(
            "torch.onion",
            8080,
            "/search",
            b"q=a".to_vec(),
            "application/x-www-form-urlencoded",
        )
        .unwrap()
        .serializar(&red);
        let texto = String::from_utf8(bytes).unwrap();
        assert!(texto.contains("Host: torch.onion:8080\r\n"));
        assert!(texto.contains("Content-Length: 3\r\n"));
        assert!(texto.ends_with("\r\n\r\nq=a"));
    }

    #[test]
    fn ruta_relativa_se_normaliza() {
        assert_eq!(PeticionHttp::get("a.onion", 80, "x").unwrap().ruta(), "/x");
    }
}
