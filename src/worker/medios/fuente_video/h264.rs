//! H.264 con OpenH264: preparación del decodificador a partir de la
//! configuración `avcC` del MP4, paso de las muestras a Annex B y
//! reconstrucción de cada fotograma a RGBA.

use crate::configuracion::ConfiguracionWorker;
use crate::ipc::mensajes::FotogramaVideo;
use crate::worker::yuv::{self, FotogramaYuv, Matriz, Plano, Submuestreo};
use openh264::formats::YUVSource;

/// Prefijo de inicio de unidad NAL en formato Annex B.
const INICIO_NAL: [u8; 4] = [0, 0, 0, 1];
/// Altura a partir de la cual se asume la matriz de color de alta definición.
const ALTO_ALTA_DEFINICION: usize = 720;
/// Posición en `avcC` del byte con el tamaño del prefijo de longitud de NAL
/// (ISO/IEC 14496-15, `lengthSizeMinusOne`).
const AVCC_TAMANO_PREFIJO: usize = 4;
/// Bits de `lengthSizeMinusOne`.
const AVCC_MASCARA_TAMANO_PREFIJO: u8 = 0b11;
/// Posición en `avcC` del número de conjuntos SPS, al que siguen los PPS.
const AVCC_INICIO_CONJUNTOS: usize = 5;
/// Bits del número de SPS (los tres altos están reservados); el número de PPS
/// ocupa el byte entero.
const AVCC_MASCARAS_CUENTA: [u8; 2] = [0x1F, u8::MAX];
/// Bytes del tamaño que precede a cada conjunto de parámetros.
const AVCC_BYTES_LARGO_CONJUNTO: usize = 2;

/// OpenH264 listo para decodificar y lo que necesita cada muestra.
pub(super) struct PreparacionH264 {
    /// Decodificador.
    pub decodificador: Box<openh264::decoder::Decoder>,
    /// Bytes del prefijo de longitud de cada NAL en las muestras.
    pub longitud_nal: usize,
    /// Conjuntos de parámetros (SPS/PPS) en Annex B, para la primera muestra.
    pub cabecera: Vec<u8>,
}

/// Prepara OpenH264 a partir de la configuración `avcC` del MP4.
///
/// # Errors
/// Descripción si la configuración está truncada o el decodificador no arranca.
pub(super) fn preparar(avcc: &[u8]) -> Result<PreparacionH264, String> {
    let invalido = |m: &str| format!("configuración H.264: {m}");
    let longitud_nal = usize::from(
        avcc.get(AVCC_TAMANO_PREFIJO)
            .ok_or_else(|| invalido("truncada"))?
            & AVCC_MASCARA_TAMANO_PREFIJO,
    ) + 1;
    let cabecera = conjuntos_de_parametros(avcc).map_err(invalido)?;
    // Sin vaciado forzado tras cada muestra: el vaciado por defecto de la
    // biblioteca rompe el búfer de reordenación de H.264 (comprobado: un High
    // con retardo de reordenación fallaba con «out of memory» en el 7.º
    // fotograma). Los fotogramas retenidos se recogen al final de la pista.
    let configuracion = openh264::decoder::DecoderConfig::new()
        .flush_after_decode(openh264::decoder::Flush::NoFlush);
    let decodificador = openh264::decoder::Decoder::with_api_config(
        openh264::OpenH264API::from_source(),
        configuracion,
    )
    .map_err(|e| format!("decodificador H.264: {e}"))?;
    Ok(PreparacionH264 {
        decodificador: Box::new(decodificador),
        longitud_nal,
        cabecera,
    })
}

/// SPS y PPS de `avcC`, cada uno con su prefijo de inicio Annex B.
fn conjuntos_de_parametros(avcc: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut cabecera = Vec::new();
    let mut posicion = AVCC_INICIO_CONJUNTOS;
    for mascara in AVCC_MASCARAS_CUENTA {
        let cuenta = avcc.get(posicion).ok_or("truncada")? & mascara;
        posicion += 1;
        for _ in 0..cuenta {
            let largo = avcc
                .get(posicion..posicion + AVCC_BYTES_LARGO_CONJUNTO)
                .map(|b| usize::from(u16::from_be_bytes([b[0], b[1]])))
                .ok_or("truncada")?;
            let inicio = posicion + AVCC_BYTES_LARGO_CONJUNTO;
            let nal = avcc
                .get(inicio..inicio + largo)
                .ok_or("conjunto de parámetros truncado")?;
            cabecera.extend_from_slice(&INICIO_NAL);
            cabecera.extend_from_slice(nal);
            posicion = inicio + largo;
        }
    }
    Ok(cabecera)
}

/// Convierte una muestra con NAL prefijadas por su longitud a Annex B.
///
/// # Errors
/// Descripción si una longitud está truncada o una NAL se sale de la muestra.
pub(super) fn a_annex_b(muestra: &[u8], longitud_nal: usize) -> Result<Vec<u8>, String> {
    let mut salida = Vec::with_capacity(muestra.len());
    let mut posicion = 0usize;
    while posicion < muestra.len() {
        let largo = muestra
            .get(posicion..posicion + longitud_nal)
            .ok_or("longitud de NAL truncada")?
            .iter()
            .fold(0usize, |acc, &b| (acc << u8::BITS) | usize::from(b));
        let inicio = posicion + longitud_nal;
        let nal = inicio
            .checked_add(largo)
            .and_then(|fin| muestra.get(inicio..fin))
            .ok_or("NAL que se sale de su muestra")?;
        salida.extend_from_slice(&INICIO_NAL);
        salida.extend_from_slice(nal);
        posicion = inicio + largo;
    }
    Ok(salida)
}

/// Reconstruye a RGBA un fotograma decodificado por OpenH264.
///
/// # Errors
/// Descripción si el fotograma supera el límite o sus planos no cuadran.
pub(super) fn a_rgba(
    imagen: &openh264::decoder::DecodedYUV<'_>,
    marca_ms: u64,
    cfg: &ConfiguracionWorker,
) -> Result<FotogramaVideo, String> {
    let (ancho, alto) = imagen.dimensions();
    let (paso_y, paso_u, paso_v) = imagen.strides();
    let (a, h) = (
        u32::try_from(ancho).map_err(|_| "ancho")?,
        u32::try_from(alto).map_err(|_| "alto")?,
    );
    if !cfg.admite_resolucion(a, h) {
        return Err(format!("fotograma de {a}×{h} fuera del límite"));
    }
    let plano = |datos, paso| Plano { datos, paso };
    let rgba = yuv::a_rgba(&FotogramaYuv {
        ancho,
        alto,
        y: plano(imagen.y(), paso_y),
        u: plano(imagen.u(), paso_u),
        v: plano(imagen.v(), paso_v),
        submuestreo: Submuestreo::Medio,
        matriz: if alto >= ALTO_ALTA_DEFINICION {
            Matriz::Bt709
        } else {
            Matriz::Bt601
        },
        rango_completo: false,
    })?;
    Ok(FotogramaVideo {
        marca_ms,
        ancho: a,
        alto: h,
        rgba,
    })
}
