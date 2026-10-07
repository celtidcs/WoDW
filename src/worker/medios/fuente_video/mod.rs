//! Pista de vídeo de un contenedor ya identificado y revisado: lee muestras,
//! las decodifica y reconstruye cada fotograma como píxeles RGBA nuevos
//! (CA-V2). Del archivo original solo salen esos píxeles y su instante.
//!
//! - MP4 (`mp4`, con `re_mp4`): H.264 con OpenH264 (`h264`; C de Cisco,
//!   solo aquí, dentro del Worker confinado y sin red) o AV1 con `re_rav1d`.
//! - WebM (`matroska`, con `matroska-demuxer`): AV1 con `re_rav1d`.
//!
//! Las dimensiones declaradas por el contenedor se comprueban contra el
//! límite de resolución **antes** de crear ningún decodificador.

mod h264;
mod matroska;
mod mp4;

use super::contenedor::Contenedor;
use crate::configuracion::ConfiguracionWorker;
use crate::ipc::mensajes::FotogramaVideo;
use crate::worker::av1::FotogramaRgba;
use std::sync::Arc;

/// Pista de vídeo abierta.
pub trait FuenteVideo: Send {
    /// Dimensiones declaradas por el contenedor (ya dentro del límite).
    fn dimensiones(&self) -> (u32, u32);
    /// Fotogramas listos tras procesar la siguiente muestra; `Ok(None)` al terminar.
    ///
    /// # Errors
    /// Descripción si el flujo está dañado.
    fn siguiente(&mut self) -> Result<Option<Vec<FotogramaVideo>>, String>;
}

/// Error de apertura: inválido u hostil (por encima del límite de resolución).
#[derive(Debug)]
pub enum RechazoVideo {
    /// No se puede abrir o no está admitido.
    Invalido(String),
    /// Declara una resolución por encima del límite.
    ExcedeLimites(String),
}

/// Resultado de abrir una pista: `Ok(None)` si el contenedor no tiene vídeo.
type Apertura = Result<Option<Box<dyn FuenteVideo>>, RechazoVideo>;

/// Abre la pista de vídeo; `Ok(None)` si el contenedor no tiene vídeo.
///
/// # Errors
/// [`RechazoVideo`] si no se puede abrir, el códec no está admitido o la
/// resolución supera el límite.
pub fn abrir(contenedor: Contenedor, datos: Arc<[u8]>, cfg: &ConfiguracionWorker) -> Apertura {
    match contenedor {
        Contenedor::Mp4 => mp4::abrir(datos, cfg),
        Contenedor::Matroska => matroska::abrir(datos, cfg),
        _ => Ok(None),
    }
}

/// Comprueba las dimensiones declaradas antes de decodificar.
fn exigir_limite(
    ancho: u64,
    alto: u64,
    cfg: &ConfiguracionWorker,
) -> Result<(u32, u32), RechazoVideo> {
    let (Ok(a), Ok(h)) = (u32::try_from(ancho), u32::try_from(alto)) else {
        return Err(RechazoVideo::ExcedeLimites(format!(
            "vídeo de {ancho}×{alto}"
        )));
    };
    if a == 0 || h == 0 {
        return Err(RechazoVideo::Invalido("vídeo sin dimensiones".to_string()));
    }
    if !cfg.admite_resolucion(a, h) {
        return Err(RechazoVideo::ExcedeLimites(format!(
            "vídeo de {a}×{h} supera el límite de {}×{}",
            cfg.lado_largo_maximo_px, cfg.lado_corto_maximo_px
        )));
    }
    Ok((a, h))
}

/// Límite de píxeles por fotograma para el decodificador AV1.
fn max_pixeles(cfg: &ConfiguracionWorker) -> u32 {
    cfg.lado_largo_maximo_px
        .saturating_mul(cfg.lado_corto_maximo_px)
}

/// Convierte fotogramas AV1 a fotogramas de salida, exigiendo el límite.
fn fotogramas_av1(
    fotogramas: Vec<FotogramaRgba>,
    cfg: &ConfiguracionWorker,
) -> Result<Vec<FotogramaVideo>, String> {
    fotogramas
        .into_iter()
        .map(|f| {
            if !cfg.admite_resolucion(f.ancho, f.alto) {
                return Err(format!(
                    "fotograma de {}×{} fuera del límite",
                    f.ancho, f.alto
                ));
            }
            Ok(FotogramaVideo {
                marca_ms: u64::try_from(f.marca_tiempo.unwrap_or(0)).unwrap_or(0),
                ancho: f.ancho,
                alto: f.alto,
                rgba: f.rgba,
            })
        })
        .collect()
}
