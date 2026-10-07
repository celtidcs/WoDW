//! Detección de enlaces engañosos.
//!
//! Un enlace cuyo texto visible parece una dirección distinta de su destino
//! real es la base del *phishing*; un host en punycode (`xn--`) puede imitar
//! a otro con letras de otro alfabeto (homógrafos).

use url::Url;

/// Prefijo ACE de las etiquetas de dominio internacionalizadas (RFC 3492).
const PREFIJO_PUNYCODE: &str = "xn--";
/// Sufijo de los servicios onion.
const SUFIJO_ONION: &str = ".onion";
/// Prefijos con los que un texto visible se presenta como dirección.
const PREFIJOS_DIRECCION: &[&str] = &["http://", "https://", "www."];

/// Aviso sobre un enlace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AvisoEnlace {
    /// El texto visible muestra una dirección que no es el destino real.
    DestinoDistinto {
        /// Host al que lleva de verdad.
        host_real: String,
    },
    /// El destino usa un nombre internacionalizado que puede imitar a otro.
    Punycode {
        /// Host real en su forma ASCII.
        host_real: String,
    },
}

impl AvisoEnlace {
    /// Host real, mutable para poder sobrescribirlo en la purga.
    pub fn host_real_mut(&mut self) -> &mut String {
        match self {
            Self::DestinoDistinto { host_real } | Self::Punycode { host_real } => host_real,
        }
    }
}

/// Evalúa un enlace con texto visible `texto` y destino absoluto `url`.
///
/// Devuelve `None` si no hay nada sospechoso o si `url` no es válida (en ese
/// caso el enlace ya se habría descartado antes).
pub fn evaluar_enlace(texto: &str, url: &str) -> Option<AvisoEnlace> {
    let host_real = Url::parse(url).ok()?.host_str()?.to_ascii_lowercase();
    if let Some(host_texto) = host_mostrado(texto) {
        if sin_www(&host_texto) != sin_www(&host_real) {
            return Some(AvisoEnlace::DestinoDistinto { host_real });
        }
    }
    host_real
        .split('.')
        .any(|etiqueta| etiqueta.starts_with(PREFIJO_PUNYCODE))
        .then_some(AvisoEnlace::Punycode { host_real })
}

/// Host que el texto visible aparenta, si se presenta como una dirección.
fn host_mostrado(texto: &str) -> Option<String> {
    let texto = texto.trim().to_lowercase();
    let parece_direccion = PREFIJOS_DIRECCION.iter().any(|p| texto.starts_with(p))
        || texto
            .split(['/', ' '])
            .next()
            .is_some_and(|h| h.ends_with(SUFIJO_ONION));
    if !parece_direccion || texto.contains(char::is_whitespace) {
        return None;
    }
    let con_esquema = if texto.contains("://") {
        texto
    } else {
        format!("http://{texto}")
    };
    Url::parse(&con_esquema)
        .ok()?
        .host_str()
        .map(str::to_string)
}

/// Quita un `www.` inicial para comparar hosts.
fn sin_www(host: &str) -> &str {
    host.strip_prefix("www.").unwrap_or(host)
}
