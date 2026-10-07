//! Identificación de contenedores por su firma (CA-C1) y revisión estructural
//! propia antes de decodificar (CA-C5).
//!
//! La revisión no interpreta el contenido: solo comprueba que las partes del
//! archivo declaran tamaños coherentes, no se pisan, no se salen del archivo
//! y, donde el formato lo permite, que su suma de comprobación cuadra. Muchos
//! ataques a decodificadores se apoyan en estructuras imposibles; aquí se
//! rechazan sin que ningún decodificador las vea.

use crate::ipc::mensajes::FamiliaMedio;
use std::ops::Range;

/// Contenedor reconocido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contenedor {
    /// MPEG audio (MP3), con o sin etiqueta ID3v2 delante.
    Mp3,
    /// AAC en tramas ADTS.
    Adts,
    /// Ogg (Vorbis u Opus).
    Ogg,
    /// FLAC nativo.
    Flac,
    /// RIFF/WAVE.
    Wav,
    /// ISO BMFF (MP4/M4A).
    Mp4,
    /// Matroska/WebM.
    Matroska,
}

impl Contenedor {
    /// `true` si el contenedor pertenece a la familia declarada.
    pub fn admitido_en(self, familia: FamiliaMedio) -> bool {
        match familia {
            FamiliaMedio::Audio => !matches!(self, Self::Matroska),
            FamiliaMedio::Video => matches!(self, Self::Mp4 | Self::Matroska),
        }
    }
}

/// Profundidad máxima de anidamiento que se recorre en MP4.
const PROFUNDIDAD_MAXIMA: u32 = 16;
/// Cajas MP4 que solo contienen otras cajas.
const CAJAS_CONTENEDORAS: &[&[u8; 4]] = &[
    b"moov", b"trak", b"mdia", b"minf", b"stbl", b"edts", b"dinf", b"mvex", b"moof", b"traf",
];
/// Identificador EBML de la cabecera de un archivo Matroska.
const EBML_CABECERA: u32 = 0x1A45_DFA3;
/// Identificador EBML del segmento Matroska.
const EBML_SEGMENTO: u32 = 0x1853_8067;
/// Identificador EBML de un «cluster» (puede tener tamaño desconocido).
const EBML_CLUSTER: u32 = 0x1F43_B675;
/// Tipo de bloque FLAC inválido según la especificación.
const FLAC_TIPO_INVALIDO: u8 = 127;
/// Tipo y longitud del bloque STREAMINFO de FLAC (obligatorio y primero).
const FLAC_STREAMINFO: (u8, usize) = (0, 34);
/// Longitud fija de la cabecera de una página Ogg (sin la tabla de segmentos).
const OGG_CABECERA: usize = 27;
/// Longitud de la cabecera ID3v2 (y del pie, si lo declara).
const ID3_CABECERA: usize = 10;
/// Longitud mínima de una cabecera ADTS.
const ADTS_CABECERA: usize = 7;
/// Byte de sincronía con el que empieza toda trama MPEG, ADTS o FLAC.
const BYTE_SINCRONIA: u8 = 0xFF;
/// Tres bits altos del segundo byte, que completan los 11 bits de sincronía MPEG.
const MPEG_MASCARA_SINCRONIA: u8 = 0xE0;
/// Bits de la capa MPEG en el segundo byte: `00` identifica una trama ADTS.
const MPEG_MASCARA_CAPA: u8 = 0x06;
/// Bit de «sin protección», que una trama ADTS lleva a uno.
const ADTS_SIN_PROTECCION: u8 = 0x10;
/// Segundo byte de una trama ADTS sin los bits de protección y de versión.
const ADTS_MASCARA_SINCRONIA: u8 = 0xF6;
/// Valor esperado tras aplicar [`ADTS_MASCARA_SINCRONIA`].
const ADTS_SINCRONIA: u8 = 0xF0;
/// Posición de la firma `WAVE` dentro de la cabecera RIFF.
const RIFF_FORMATO: Range<usize> = 8..12;
/// Posición del tipo de caja (`ftyp`) en la primera caja de un MP4.
const MP4_TIPO_PRIMERA_CAJA: Range<usize> = 4..8;
/// Cabecera de caja MP4: tamaño de 32 bits y tipo.
const MP4_CABECERA_CAJA: usize = 8;
/// Cabecera de caja MP4 con tamaño de 64 bits tras el tipo.
const MP4_CABECERA_CAJA_GRANDE: usize = 16;
/// Tamaño de 32 bits que significa «hasta el final del archivo».
const MP4_TAMANO_HASTA_EL_FINAL: u64 = 0;
/// Tamaño de 32 bits que significa «el tamaño real va en 64 bits».
const MP4_TAMANO_DE_64_BITS: u64 = 1;
/// Bytes del tamaño corto (32 bits) de una caja MP4.
const MP4_BYTES_TAMANO: usize = 4;
/// Bytes del tamaño largo (64 bits) de una caja MP4.
const MP4_BYTES_TAMANO_GRANDE: usize = 8;
/// Firma de una página Ogg.
const OGG_FIRMA: &[u8; 4] = b"OggS";
/// Posición del número de segmentos en la cabecera de página Ogg.
const OGG_NUMERO_SEGMENTOS: usize = 26;
/// Campo de la suma de comprobación en la cabecera de página Ogg.
const OGG_CAMPO_CRC: Range<usize> = 22..26;
/// Bytes de la firma `fLaC` con la que empieza un FLAC.
const FLAC_FIRMA: usize = 4;
/// Cabecera de un bloque de metadatos FLAC: tipo (1 byte) y longitud (3 bytes).
const FLAC_CABECERA_BLOQUE: usize = 4;
/// Bytes de la longitud de un bloque de metadatos FLAC.
const FLAC_BYTES_LONGITUD: usize = 3;
/// Bit que marca el último bloque de metadatos FLAC.
const FLAC_MARCA_ULTIMO: u8 = 0x80;
/// Bits del tipo de bloque de metadatos FLAC.
const FLAC_MASCARA_TIPO: u8 = 0x7F;
/// Segundo byte de una trama FLAC sin el bit de tamaño de bloque variable.
const FLAC_MASCARA_SINCRONIA: u8 = 0xFE;
/// Valor esperado tras aplicar [`FLAC_MASCARA_SINCRONIA`].
const FLAC_SINCRONIA: u8 = 0xF8;
/// Cabecera de un fragmento RIFF: identificador y tamaño de 32 bits.
const RIFF_CABECERA_FRAGMENTO: usize = 8;
/// Primer fragmento RIFF, tras `RIFF`, el tamaño y `WAVE`.
const RIFF_INICIO_FRAGMENTOS: usize = 12;
/// Posición del tamaño en la cabecera RIFF y en la de cada fragmento.
const RIFF_TAMANO: Range<usize> = 4..8;
/// Posición de las banderas en la cabecera ID3v2.
const ID3_BANDERAS: usize = 5;
/// Bandera ID3v2 que anuncia un pie de etiqueta.
const ID3_BANDERA_PIE: u8 = 0x10;
/// Posición del tamaño «sincronizado» (7 bits útiles por byte) en la cabecera ID3v2.
const ID3_TAMANO: Range<usize> = 6..10;
/// Bit que nunca puede estar a uno en un byte de tamaño sincronizado.
const ID3_BIT_PROHIBIDO: u8 = 0x80;
/// Bits útiles de cada byte del tamaño sincronizado ID3v2.
const ID3_BITS_POR_BYTE: u32 = 7;
/// Longitud máxima en bytes de un entero variable EBML.
const EBML_LONGITUD_MAXIMA: usize = 8;
/// Longitud máxima en bytes de un identificador EBML.
const EBML_LARGO_ID_MAXIMO: usize = 4;
/// Bits de valor de cada byte de un entero variable EBML (el resto es la marca).
const EBML_BITS_POR_BYTE: usize = 7;

/// Identifica el contenedor por sus primeros bytes.
pub fn identificar(datos: &[u8]) -> Option<Contenedor> {
    let inicio = |firma: &[u8]| datos.starts_with(firma);
    if inicio(b"fLaC") {
        Some(Contenedor::Flac)
    } else if inicio(OGG_FIRMA) {
        Some(Contenedor::Ogg)
    } else if inicio(b"RIFF") && datos.get(RIFF_FORMATO) == Some(b"WAVE") {
        Some(Contenedor::Wav)
    } else if datos.get(MP4_TIPO_PRIMERA_CAJA) == Some(b"ftyp") {
        Some(Contenedor::Mp4)
    } else if inicio(&EBML_CABECERA.to_be_bytes()) {
        Some(Contenedor::Matroska)
    } else if inicio(b"ID3") {
        Some(Contenedor::Mp3)
    } else {
        match datos {
            // Sincronía de trama MPEG: 11 bits a uno; la capa 00 es ADTS (AAC).
            [BYTE_SINCRONIA, b, ..]
                if sincronia_mpeg(*b)
                    && b & MPEG_MASCARA_CAPA == 0
                    && b & ADTS_SIN_PROTECCION != 0 =>
            {
                Some(Contenedor::Adts)
            }
            [BYTE_SINCRONIA, b, ..] if sincronia_mpeg(*b) && b & MPEG_MASCARA_CAPA != 0 => {
                Some(Contenedor::Mp3)
            }
            _ => None,
        }
    }
}

/// `true` si el segundo byte de una trama completa la sincronía MPEG.
fn sincronia_mpeg(segundo: u8) -> bool {
    segundo & MPEG_MASCARA_SINCRONIA == MPEG_MASCARA_SINCRONIA
}

/// Revisa la estructura del contenedor.
///
/// # Errors
/// Descripción de la incoherencia encontrada (siempre empieza por «estructura»).
pub fn revisar_estructura(contenedor: Contenedor, datos: &[u8]) -> Result<(), String> {
    let resultado = match contenedor {
        Contenedor::Mp4 => revisar_cajas_mp4(datos, 0),
        Contenedor::Ogg => revisar_ogg(datos),
        Contenedor::Flac => revisar_flac(datos),
        Contenedor::Wav => revisar_wav(datos),
        Contenedor::Mp3 => revisar_id3(datos),
        Contenedor::Adts => revisar_adts(datos),
        Contenedor::Matroska => revisar_matroska(datos),
    };
    resultado.map_err(|motivo| format!("estructura {contenedor:?} incoherente: {motivo}"))
}

/// Lee un entero big-endian de `n` bytes en `posicion`.
fn leer_be(datos: &[u8], posicion: usize, n: usize) -> Result<u64, String> {
    let bytes = posicion
        .checked_add(n)
        .and_then(|fin| datos.get(posicion..fin))
        .ok_or("lectura fuera del archivo")?;
    Ok(bytes
        .iter()
        .fold(0u64, |acc, &b| (acc << u8::BITS) | u64::from(b)))
}

/// Recorre una secuencia de cajas MP4 que debe llenar `datos` exactamente.
fn revisar_cajas_mp4(datos: &[u8], profundidad: u32) -> Result<(), String> {
    if profundidad > PROFUNDIDAD_MAXIMA {
        return Err("anidamiento excesivo de cajas".to_string());
    }
    let mut posicion = 0usize;
    while posicion < datos.len() {
        let tamano = leer_be(datos, posicion, MP4_BYTES_TAMANO)?;
        let tipo = datos
            .get(posicion + MP4_BYTES_TAMANO..posicion + MP4_CABECERA_CAJA)
            .ok_or("cabecera de caja truncada")?;
        let (cabecera, tamano) = match tamano {
            MP4_TAMANO_HASTA_EL_FINAL => (MP4_CABECERA_CAJA, (datos.len() - posicion) as u64),
            MP4_TAMANO_DE_64_BITS => (
                MP4_CABECERA_CAJA_GRANDE,
                leer_be(datos, posicion + MP4_CABECERA_CAJA, MP4_BYTES_TAMANO_GRANDE)?,
            ),
            t => (MP4_CABECERA_CAJA, t),
        };
        let tamano = usize::try_from(tamano).map_err(|_| "tamaño de caja enorme")?;
        if tamano < cabecera {
            return Err(format!(
                "caja {:?} menor que su cabecera",
                String::from_utf8_lossy(tipo)
            ));
        }
        let fin = posicion
            .checked_add(tamano)
            .filter(|&fin| fin <= datos.len())
            .ok_or_else(|| {
                format!(
                    "caja {:?} se sale del archivo",
                    String::from_utf8_lossy(tipo)
                )
            })?;
        if CAJAS_CONTENEDORAS.iter().any(|c| c.as_slice() == tipo) {
            revisar_cajas_mp4(&datos[posicion + cabecera..fin], profundidad + 1)?;
        }
        posicion = fin;
    }
    Ok(())
}

/// Recorre las páginas Ogg: deben teselar el archivo y su CRC debe cuadrar.
fn revisar_ogg(datos: &[u8]) -> Result<(), String> {
    let mut posicion = 0usize;
    while posicion < datos.len() {
        let pagina = &datos[posicion..];
        // Tras la firma va la versión del formato, que solo puede ser 0.
        if !pagina.starts_with(OGG_FIRMA) || pagina.get(OGG_FIRMA.len()) != Some(&0) {
            return Err(format!("página sin firma o versión en {posicion}"));
        }
        let segmentos = usize::from(
            *pagina
                .get(OGG_NUMERO_SEGMENTOS)
                .ok_or("cabecera de página truncada")?,
        );
        let tabla = pagina
            .get(OGG_CABECERA..OGG_CABECERA + segmentos)
            .ok_or("tabla de segmentos truncada")?;
        let longitud =
            OGG_CABECERA + segmentos + tabla.iter().map(|&l| usize::from(l)).sum::<usize>();
        let pagina = pagina
            .get(..longitud)
            .ok_or("página que se sale del archivo")?;
        let declarado = u32::from_le_bytes(
            pagina[OGG_CAMPO_CRC]
                .try_into()
                .map_err(|_| "suma de comprobación truncada")?,
        );
        if crc_ogg(pagina) != declarado {
            return Err(format!("suma de comprobación de página en {posicion}"));
        }
        posicion += longitud;
    }
    Ok(())
}

/// CRC-32 de Ogg (polinomio 0x04C11DB7, sin reflejar, valor inicial 0) con el
/// campo de la suma ([`OGG_CAMPO_CRC`]) a cero.
fn crc_ogg(pagina: &[u8]) -> u32 {
    /// Polinomio generador de la especificación Ogg (RFC 3533).
    const POLINOMIO: u32 = 0x04C1_1DB7;
    /// Desplazamiento que lleva un byte a la parte alta del registro de 32 bits.
    const BYTE_ALTO: u32 = u32::BITS - u8::BITS;
    /// Bit más alto del registro, que decide si se aplica el polinomio.
    const BIT_ALTO: u32 = 1 << (u32::BITS - 1);
    pagina.iter().enumerate().fold(0u32, |crc, (i, &b)| {
        let b = if OGG_CAMPO_CRC.contains(&i) { 0 } else { b };
        (0..u8::BITS).fold(crc ^ (u32::from(b) << BYTE_ALTO), |c, _| {
            if c & BIT_ALTO != 0 {
                (c << 1) ^ POLINOMIO
            } else {
                c << 1
            }
        })
    })
}

/// Bloques de metadatos FLAC: STREAMINFO primero, todos dentro del archivo y,
/// tras el último, una trama de audio.
fn revisar_flac(datos: &[u8]) -> Result<(), String> {
    let mut posicion = FLAC_FIRMA;
    let mut primero = true;
    loop {
        let cabecera = *datos.get(posicion).ok_or("bloque de metadatos truncado")?;
        let (ultimo, tipo) = (
            cabecera & FLAC_MARCA_ULTIMO != 0,
            cabecera & FLAC_MASCARA_TIPO,
        );
        let longitud = usize::try_from(leer_be(datos, posicion + 1, FLAC_BYTES_LONGITUD)?)
            .map_err(|_| "longitud")?;
        if tipo == FLAC_TIPO_INVALIDO {
            return Err("tipo de bloque inválido".to_string());
        }
        if primero && (tipo, longitud) != FLAC_STREAMINFO {
            return Err("el primer bloque no es STREAMINFO".to_string());
        }
        primero = false;
        posicion = posicion
            .checked_add(FLAC_CABECERA_BLOQUE + longitud)
            .filter(|&fin| fin <= datos.len())
            .ok_or("bloque de metadatos que se sale del archivo")?;
        if ultimo {
            break;
        }
    }
    match datos.get(posicion..posicion + 2) {
        Some([BYTE_SINCRONIA, b]) if b & FLAC_MASCARA_SINCRONIA == FLAC_SINCRONIA => Ok(()),
        _ => Err("tras los metadatos no hay una trama de audio".to_string()),
    }
}

/// Fragmentos RIFF dentro del tamaño declarado, con `fmt ` y `data`.
fn revisar_wav(datos: &[u8]) -> Result<(), String> {
    let declarado = tamano_riff(datos, 0).ok_or("cabecera RIFF truncada")?;
    let fin = declarado
        .checked_add(RIFF_CABECERA_FRAGMENTO)
        .filter(|&f| f <= datos.len())
        .ok_or("RIFF declara más bytes de los que hay")?;
    let (mut posicion, mut fmt, mut data) = (RIFF_INICIO_FRAGMENTOS, false, false);
    while posicion + RIFF_CABECERA_FRAGMENTO <= fin {
        let id = &datos[posicion..posicion + RIFF_TAMANO.start];
        let tamano = tamano_riff(datos, posicion).ok_or("tamaño de fragmento")?;
        fmt |= id == b"fmt ";
        data |= id == b"data";
        // Los fragmentos de tamaño impar llevan un byte de relleno.
        posicion = posicion
            .checked_add(RIFF_CABECERA_FRAGMENTO + tamano + tamano % 2)
            .filter(|&p| p <= fin + 1)
            .ok_or("fragmento que se sale del archivo")?;
    }
    (fmt && data)
        .then_some(())
        .ok_or_else(|| "faltan los fragmentos fmt o data".to_string())
}

/// Tamaño de 32 bits (little-endian) de la cabecera RIFF o del fragmento que
/// empieza en `inicio`.
fn tamano_riff(datos: &[u8], inicio: usize) -> Option<usize> {
    let campo = datos.get(inicio + RIFF_TAMANO.start..inicio + RIFF_TAMANO.end)?;
    usize::try_from(u32::from_le_bytes(campo.try_into().ok()?)).ok()
}

/// Etiqueta ID3v2 (si la hay) dentro del archivo y seguida de una trama MPEG.
fn revisar_id3(datos: &[u8]) -> Result<(), String> {
    let mut posicion = 0usize;
    if datos.starts_with(b"ID3") {
        let cabecera = datos.get(..ID3_CABECERA).ok_or("cabecera ID3 truncada")?;
        if cabecera[ID3_TAMANO]
            .iter()
            .any(|b| b & ID3_BIT_PROHIBIDO != 0)
        {
            return Err("tamaño ID3 no sincronizado".to_string());
        }
        let tamano = cabecera[ID3_TAMANO].iter().fold(0usize, |acc, &b| {
            (acc << ID3_BITS_POR_BYTE) | usize::from(b)
        });
        let pie = if cabecera[ID3_BANDERAS] & ID3_BANDERA_PIE != 0 {
            ID3_CABECERA
        } else {
            0
        };
        posicion = ID3_CABECERA + tamano + pie;
    }
    match datos.get(posicion..posicion + 2) {
        Some([BYTE_SINCRONIA, b]) if sincronia_mpeg(*b) => Ok(()),
        _ => Err("tras la etiqueta ID3 no hay una trama MPEG".to_string()),
    }
}

/// Tramas ADTS encadenadas hasta el final del archivo.
fn revisar_adts(datos: &[u8]) -> Result<(), String> {
    let mut posicion = 0usize;
    while posicion < datos.len() {
        let cabecera = datos
            .get(posicion..posicion + ADTS_CABECERA)
            .ok_or("cabecera ADTS truncada")?;
        if cabecera[0] != BYTE_SINCRONIA || cabecera[1] & ADTS_MASCARA_SINCRONIA != ADTS_SINCRONIA {
            return Err(format!("trama ADTS sin sincronía en {posicion}"));
        }
        let longitud = longitud_trama_adts(cabecera);
        if longitud < ADTS_CABECERA {
            return Err("trama ADTS menor que su cabecera".to_string());
        }
        posicion = posicion
            .checked_add(longitud)
            .filter(|&p| p <= datos.len())
            .ok_or("trama ADTS que se sale del archivo")?;
    }
    Ok(())
}

/// Longitud de la trama ADTS (cabecera incluida): 13 bits repartidos entre los
/// bytes 3 (2 bits bajos), 4 (8 bits) y 5 (3 bits altos), según ISO/IEC 14496-3.
fn longitud_trama_adts(cabecera: &[u8]) -> usize {
    (usize::from(cabecera[3] & 0b11) << 11)
        | (usize::from(cabecera[4]) << 3)
        | (usize::from(cabecera[5]) >> 5)
}

/// Elemento EBML leído: identificador, desplazamiento del contenido y tamaño
/// (`None` si es «desconocido»).
struct ElementoEbml {
    id: u32,
    contenido: usize,
    tamano: Option<usize>,
}

/// Lee un entero de longitud variable EBML. Devuelve (valor, bytes, todo_unos).
fn leer_vint(
    datos: &[u8],
    posicion: usize,
    conservar_marca: bool,
) -> Result<(u64, usize, bool), String> {
    let primero = *datos.get(posicion).ok_or("entero EBML truncado")?;
    let longitud = primero.leading_zeros() as usize + 1;
    if longitud > EBML_LONGITUD_MAXIMA {
        return Err("entero EBML sin marca de longitud".to_string());
    }
    let crudo = leer_be(datos, posicion, longitud)?;
    let mascara = if longitud == EBML_LONGITUD_MAXIMA {
        u64::MAX >> u8::BITS
    } else {
        (1u64 << (EBML_BITS_POR_BYTE * longitud)) - 1
    };
    let valor = if conservar_marca {
        crudo
    } else {
        crudo & mascara
    };
    Ok((valor, longitud, crudo & mascara == mascara))
}

/// Lee la cabecera de un elemento EBML en `posicion`.
fn leer_elemento(datos: &[u8], posicion: usize) -> Result<ElementoEbml, String> {
    let (id, largo_id, _) = leer_vint(datos, posicion, true)?;
    if largo_id > EBML_LARGO_ID_MAXIMO {
        return Err("identificador EBML demasiado largo".to_string());
    }
    let (tamano, largo_tamano, desconocido) = leer_vint(datos, posicion + largo_id, false)?;
    Ok(ElementoEbml {
        id: u32::try_from(id).map_err(|_| "identificador EBML")?,
        contenido: posicion + largo_id + largo_tamano,
        tamano: if desconocido {
            None
        } else {
            Some(usize::try_from(tamano).map_err(|_| "tamaño EBML enorme")?)
        },
    })
}

/// Cabecera EBML y elementos del segmento dentro de sus límites. Un elemento
/// de tamaño desconocido solo se admite para el segmento y los «clusters».
fn revisar_matroska(datos: &[u8]) -> Result<(), String> {
    let cabecera = leer_elemento(datos, 0)?;
    let fin_cabecera =
        fin_elemento(&cabecera, datos.len())?.ok_or("cabecera EBML de tamaño desconocido")?;
    let segmento = leer_elemento(datos, fin_cabecera)?;
    if segmento.id != EBML_SEGMENTO {
        return Err("tras la cabecera EBML no hay un segmento".to_string());
    }
    let fin_segmento = fin_elemento(&segmento, datos.len())?.unwrap_or(datos.len());
    let mut posicion = segmento.contenido;
    while posicion < fin_segmento {
        let hijo = leer_elemento(datos, posicion)?;
        match fin_elemento(&hijo, fin_segmento)? {
            Some(fin) => posicion = fin,
            None if hijo.id == EBML_CLUSTER => return Ok(()),
            None => return Err("elemento de tamaño desconocido no admitido".to_string()),
        }
    }
    Ok(())
}

/// Fin de un elemento si cabe dentro de `limite`; `None` si su tamaño es desconocido.
fn fin_elemento(elemento: &ElementoEbml, limite: usize) -> Result<Option<usize>, String> {
    match elemento.tamano {
        None => Ok(None),
        Some(tamano) => elemento
            .contenido
            .checked_add(tamano)
            .filter(|&fin| fin <= limite)
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "elemento EBML {:#x} que se sale de su contenedor",
                    elemento.id
                )
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firmas_basicas() {
        assert_eq!(identificar(b"fLaC...."), Some(Contenedor::Flac));
        assert_eq!(identificar(b"OggS...."), Some(Contenedor::Ogg));
        assert_eq!(
            identificar(b"\x00\x00\x00\x18ftypM4A "),
            Some(Contenedor::Mp4)
        );
        assert_eq!(identificar(&[0xFF, 0xFB, 0x90]), Some(Contenedor::Mp3));
        assert_eq!(identificar(&[0xFF, 0xF1, 0x50]), Some(Contenedor::Adts));
        assert_eq!(identificar(b"<html>"), None);
    }

    #[test]
    fn crc_ogg_de_referencia() {
        // CRC de una página vacía conocida: cabecera mínima sin segmentos.
        let mut pagina = b"OggS\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00".to_vec();
        let crc = crc_ogg(&pagina);
        pagina[22..26].copy_from_slice(&crc.to_le_bytes());
        assert!(revisar_ogg(&pagina).is_ok());
        pagina[14] ^= 1;
        assert!(revisar_ogg(&pagina).is_err());
    }

    #[test]
    fn wav_truncado_se_rechaza() {
        let mut wav = b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00".to_vec();
        wav.extend_from_slice(&[0; 16]);
        wav.extend_from_slice(b"data\x00\x00\x00\x00");
        assert!(revisar_wav(&wav).is_ok());
        wav.truncate(30);
        assert!(revisar_wav(&wav).is_err());
    }
}
