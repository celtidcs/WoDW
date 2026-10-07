//! Direcciones de servicios onion v3: formato y suma de control.
//!
//! Una dirección v3 es `base32(CLAVE | SUMA | VERSIÓN) + ".onion"`, con una
//! clave pública de 32 bytes, una suma de 2 bytes y la versión 3
//! (rend-spec-v3, sección «Encoding onion addresses»). La suma es el principio
//! de `SHA3-256(".onion checksum" | CLAVE | VERSIÓN)`. Una dirección mal
//! copiada o inventada tiene buen aspecto pero no supera la suma: ningún
//! servicio puede tenerla.

use data_encoding::BASE32_NOPAD;
use sha3::{Digest, Sha3_256};

/// Sufijo de los servicios onion.
pub(crate) const SUFIJO_ONION: &str = ".onion";
/// Longitud de la etiqueta: 35 bytes en base32 son 56 caracteres.
const LONGITUD_ETIQUETA_ONION_V3: usize = 56;
/// Bytes de la clave pública ed25519.
const BYTES_CLAVE: usize = 32;
/// Bytes de la suma de control.
const BYTES_SUMA: usize = 2;
/// Versión del formato de dirección.
const VERSION_ONION_V3: u8 = 3;
/// Prefijo constante del cálculo de la suma.
const PREFIJO_SUMA: &[u8] = b".onion checksum";

/// `true` si `host` es una dirección onion v3 auténticamente bien formada: 56
/// caracteres base32 en minúsculas (`a-z`, `2-7`) seguidos de `.onion`, de
/// versión 3 y con la suma de control correcta. Admite subdominios delante de
/// la etiqueta v3.
pub fn es_direccion_onion_v3(host: &str) -> bool {
    let Some(sin_sufijo) = host.strip_suffix(SUFIJO_ONION) else {
        return false;
    };
    let etiqueta = sin_sufijo.rsplit('.').next().unwrap_or_default();
    formato_valido(etiqueta) && suma_correcta(etiqueta)
}

/// Longitud y alfabeto base32 en minúsculas.
fn formato_valido(etiqueta: &str) -> bool {
    etiqueta.len() == LONGITUD_ETIQUETA_ONION_V3
        && etiqueta
            .bytes()
            .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
}

/// Versión 3 y suma de control de rend-spec-v3.
fn suma_correcta(etiqueta: &str) -> bool {
    let Ok(bytes) = BASE32_NOPAD.decode(etiqueta.to_ascii_uppercase().as_bytes()) else {
        return false;
    };
    let (clave, resto) = bytes.split_at(BYTES_CLAVE.min(bytes.len()));
    let (suma, version) = resto.split_at(BYTES_SUMA.min(resto.len()));
    if version != [VERSION_ONION_V3] {
        return false;
    }
    let calculada = Sha3_256::new()
        .chain_update(PREFIJO_SUMA)
        .chain_update(clave)
        .chain_update(version)
        .finalize();
    calculada.get(..BYTES_SUMA) == Some(suma)
}
