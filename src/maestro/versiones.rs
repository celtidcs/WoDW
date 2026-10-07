//! Aviso de versión nueva (CA-VN1).
//!
//! Al conectar con Tor, la sesión pregunta a la API de GitHub cuál es la
//! última versión publicada, **por Tor** y con un aislamiento propio. Solo se
//! avisa: no se descarga nada. La respuesta no es confiable: se exige un número
//! de versión `X.Y.Z` limpio y el enlace del aviso se construye a partir de la
//! configuración, nunca a partir de lo que diga la respuesta.

use crate::configuracion::ConfiguracionActualizaciones;
use serde::Deserialize;

/// Prefijo opcional de las etiquetas de versión.
const PREFIJO_ETIQUETA: char = 'v';
/// Componentes de un número de versión (`mayor.menor.parche`).
const COMPONENTES_VERSION: usize = 3;

/// Versión nueva disponible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionNueva {
    /// Número de versión (`X.Y.Z`).
    pub version: String,
    /// Enlace a su página de publicación (construido desde la configuración).
    pub enlace: String,
}

/// Campos de la respuesta de GitHub que se usan (el resto se ignora).
#[derive(Deserialize)]
struct UltimaPublicacion {
    tag_name: String,
}

/// Interpreta la respuesta de la API y decide si hay que avisar.
///
/// # Errors
/// Descripción si la respuesta no es JSON, no trae etiqueta o la etiqueta no
/// es un número de versión `X.Y.Z` (con `v` opcional).
pub fn interpretar_ultima_version(
    respuesta: &[u8],
    instalada: &str,
    cfg: &ConfiguracionActualizaciones,
) -> Result<Option<VersionNueva>, String> {
    let publicacion: UltimaPublicacion = serde_json::from_slice(respuesta)
        .map_err(|e| format!("respuesta de versiones inválida: {e}"))?;
    let etiqueta = publicacion.tag_name;
    let remota = numero_de_version(&etiqueta)?;
    let local = numero_de_version(instalada)?;
    if remota <= local {
        return Ok(None);
    }
    let version = remota.map(|n| n.to_string()).join(".");
    Ok(Some(VersionNueva {
        enlace: format!("{}{PREFIJO_ETIQUETA}{version}", cfg.url_publicaciones),
        version,
    }))
}

/// Convierte `vX.Y.Z` o `X.Y.Z` en sus tres números.
fn numero_de_version(texto: &str) -> Result<[u32; COMPONENTES_VERSION], String> {
    let limpio = texto.strip_prefix(PREFIJO_ETIQUETA).unwrap_or(texto);
    let partes: Vec<&str> = limpio.split('.').collect();
    let invalida = || {
        format!(
            "etiqueta de versión no admitida: «{}»",
            texto.escape_debug()
        )
    };
    if partes.len() != COMPONENTES_VERSION {
        return Err(invalida());
    }
    let mut numeros = [0u32; COMPONENTES_VERSION];
    for (numero, parte) in numeros.iter_mut().zip(partes) {
        if parte.is_empty() || !parte.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalida());
        }
        *numero = parte.parse().map_err(|_| invalida())?;
    }
    Ok(numeros)
}
