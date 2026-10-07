//! Pista de vídeo de un MP4 (`re_mp4`): H.264 o AV1.

use super::{
    exigir_limite, fotogramas_av1, h264, max_pixeles, Apertura, FuenteVideo, RechazoVideo,
};
use crate::configuracion::ConfiguracionWorker;
use crate::ipc::mensajes::FotogramaVideo;
use crate::unidades::MILISEGUNDOS_POR_SEGUNDO;
use crate::worker::av1::DecodificadorAv1;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

/// Prefijo de la cadena de códec de H.264 en MP4.
const CODEC_H264: &str = "avc1";
/// Prefijo de la cadena de códec de AV1 en MP4.
const CODEC_AV1: &str = "av01";

/// Muestra de vídeo MP4 ya localizada: rango en el archivo y marca (ms).
struct MuestraMp4 {
    inicio: usize,
    fin: usize,
    marca_ms: u64,
}

/// Decodificador de una pista MP4.
enum DecodificadorMp4 {
    H264 {
        decodificador: Box<openh264::decoder::Decoder>,
        longitud_nal: usize,
        cabecera: Option<Vec<u8>>,
        /// Marcas de presentación pendientes: OpenH264 entrega en orden de
        /// presentación, así que cada fotograma toma la menor.
        marcas: BinaryHeap<Reverse<u64>>,
    },
    Av1(Box<DecodificadorAv1>),
}

/// Pista de vídeo de un MP4.
struct FuenteMp4 {
    datos: Arc<[u8]>,
    muestras: std::vec::IntoIter<MuestraMp4>,
    decodificador: DecodificadorMp4,
    dimensiones: (u32, u32),
    cfg: ConfiguracionWorker,
    terminada: bool,
}

/// Abre la pista de vídeo de un MP4; `Ok(None)` si no tiene.
pub(super) fn abrir(datos: Arc<[u8]>, cfg: &ConfiguracionWorker) -> Apertura {
    let mp4 = re_mp4::Mp4::read_bytes(&datos)
        .map_err(|e| RechazoVideo::Invalido(format!("MP4 ilegible: {e}")))?;
    let Some(pista) = mp4
        .tracks()
        .values()
        .find(|p| p.kind == Some(re_mp4::TrackKind::Video))
    else {
        return Ok(None);
    };
    let dimensiones = exigir_limite(u64::from(pista.width), u64::from(pista.height), cfg)?;
    let codec = pista.codec_string(&mp4).unwrap_or_default();
    let configuracion = pista.raw_codec_config(&mp4).unwrap_or_default();
    let decodificador = decodificador_para(&codec, &configuracion, cfg)?;
    let muestras = localizar_muestras(&pista.samples, pista.timescale.max(1));
    Ok(Some(Box::new(FuenteMp4 {
        datos,
        muestras: muestras.into_iter(),
        decodificador,
        dimensiones,
        cfg: cfg.clone(),
        terminada: false,
    })))
}

/// Elige el decodificador por la cadena de códec; cualquier otro se rechaza.
fn decodificador_para(
    codec: &str,
    configuracion: &[u8],
    cfg: &ConfiguracionWorker,
) -> Result<DecodificadorMp4, RechazoVideo> {
    if codec.starts_with(CODEC_H264) {
        let h264 = h264::preparar(configuracion).map_err(RechazoVideo::Invalido)?;
        Ok(DecodificadorMp4::H264 {
            decodificador: h264.decodificador,
            longitud_nal: h264.longitud_nal,
            cabecera: Some(h264.cabecera),
            marcas: BinaryHeap::new(),
        })
    } else if codec.starts_with(CODEC_AV1) {
        Ok(DecodificadorMp4::Av1(Box::new(
            DecodificadorAv1::nuevo(max_pixeles(cfg)).map_err(RechazoVideo::Invalido)?,
        )))
    } else {
        Err(RechazoVideo::Invalido(format!(
            "códec de vídeo «{codec}» no admitido"
        )))
    }
}

/// Rango de cada muestra en el archivo y su marca de presentación en ms.
fn localizar_muestras(muestras: &[re_mp4::Sample], escala: u64) -> Vec<MuestraMp4> {
    muestras
        .iter()
        .map(|m| MuestraMp4 {
            inicio: usize::try_from(m.offset).unwrap_or(usize::MAX),
            fin: usize::try_from(m.offset.saturating_add(m.size)).unwrap_or(usize::MAX),
            marca_ms: u64::try_from(m.composition_timestamp.max(0)).unwrap_or(0)
                * MILISEGUNDOS_POR_SEGUNDO
                / escala,
        })
        .collect()
}

impl FuenteVideo for FuenteMp4 {
    fn dimensiones(&self) -> (u32, u32) {
        self.dimensiones
    }

    fn siguiente(&mut self) -> Result<Option<Vec<FotogramaVideo>>, String> {
        if self.terminada {
            return Ok(None);
        }
        let Some(muestra) = self.muestras.next() else {
            self.terminada = true;
            return self.vaciar().map(Some);
        };
        let datos = self
            .datos
            .get(muestra.inicio..muestra.fin)
            .ok_or("muestra de vídeo fuera del archivo")?;
        match &mut self.decodificador {
            DecodificadorMp4::H264 {
                decodificador,
                longitud_nal,
                cabecera,
                marcas,
            } => {
                let mut paquete = cabecera.take().unwrap_or_default();
                paquete.extend(h264::a_annex_b(datos, *longitud_nal)?);
                marcas.push(Reverse(muestra.marca_ms));
                let imagen = decodificador
                    .decode(&paquete)
                    .map_err(|e| format!("flujo H.264 inválido: {e}"))?;
                match imagen {
                    Some(imagen) => {
                        let marca = marcas.pop().map_or(0, |Reverse(m)| m);
                        Ok(Some(vec![h264::a_rgba(&imagen, marca, &self.cfg)?]))
                    }
                    None => Ok(Some(Vec::new())),
                }
            }
            DecodificadorMp4::Av1(d) => {
                let marca = i64::try_from(muestra.marca_ms).unwrap_or(i64::MAX);
                fotogramas_av1(d.decodificar(datos.to_vec(), Some(marca))?, &self.cfg).map(Some)
            }
        }
    }
}

impl FuenteMp4 {
    /// Fotogramas retenidos por el decodificador al terminar la pista.
    fn vaciar(&mut self) -> Result<Vec<FotogramaVideo>, String> {
        match &mut self.decodificador {
            DecodificadorMp4::H264 {
                decodificador,
                marcas,
                ..
            } => {
                let restantes = decodificador
                    .flush_remaining()
                    .map_err(|e| format!("flujo H.264 inválido: {e}"))?;
                restantes
                    .iter()
                    .map(|imagen| {
                        let marca = marcas.pop().map_or(0, |Reverse(m)| m);
                        h264::a_rgba(imagen, marca, &self.cfg)
                    })
                    .collect()
            }
            DecodificadorMp4::Av1(d) => fotogramas_av1(d.terminar()?, &self.cfg),
        }
    }
}
