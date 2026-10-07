//! Pista de vídeo AV1 de un WebM (`matroska-demuxer`).

use super::{exigir_limite, fotogramas_av1, max_pixeles, Apertura, FuenteVideo, RechazoVideo};
use crate::configuracion::ConfiguracionWorker;
use crate::ipc::mensajes::FotogramaVideo;
use crate::worker::av1::DecodificadorAv1;
use matroska_demuxer::{Frame, MatroskaFile, TrackType};
use std::io::Cursor;
use std::sync::Arc;

/// Identificador Matroska del códec AV1.
const CODEC_MATROSKA_AV1: &str = "V_AV1";
/// Nanosegundos por milisegundo (las marcas Matroska se escalan a ns).
const NS_POR_MS: u64 = 1_000_000;

/// Pista de vídeo AV1 de un WebM.
struct FuenteMatroska {
    archivo: MatroskaFile<Cursor<Arc<[u8]>>>,
    pista: u64,
    escala_ns: u64,
    decodificador: DecodificadorAv1,
    dimensiones: (u32, u32),
    cfg: ConfiguracionWorker,
    trama: Frame,
    terminada: bool,
}

/// Abre la pista de vídeo de un WebM; `Ok(None)` si no tiene.
pub(super) fn abrir(datos: Arc<[u8]>, cfg: &ConfiguracionWorker) -> Apertura {
    let archivo = MatroskaFile::open(Cursor::new(datos))
        .map_err(|e| RechazoVideo::Invalido(format!("WebM ilegible: {e}")))?;
    let Some(pista) = archivo
        .tracks()
        .iter()
        .find(|p| p.track_type() == TrackType::Video)
    else {
        return Ok(None);
    };
    if pista.content_encodings().is_some() {
        return Err(RechazoVideo::Invalido(
            "pista de vídeo comprimida o cifrada no admitida".to_string(),
        ));
    }
    let video = pista
        .video()
        .ok_or_else(|| RechazoVideo::Invalido("pista sin datos de vídeo".to_string()))?;
    let dimensiones = exigir_limite(video.pixel_width().get(), video.pixel_height().get(), cfg)?;
    if pista.codec_id() != CODEC_MATROSKA_AV1 {
        return Err(RechazoVideo::Invalido(format!(
            "códec de vídeo «{}» no admitido",
            pista.codec_id()
        )));
    }
    let numero = pista.track_number().get();
    let escala_ns = archivo.info().timestamp_scale().get();
    Ok(Some(Box::new(FuenteMatroska {
        archivo,
        pista: numero,
        escala_ns,
        decodificador: DecodificadorAv1::nuevo(max_pixeles(cfg)).map_err(RechazoVideo::Invalido)?,
        dimensiones,
        cfg: cfg.clone(),
        trama: Frame::default(),
        terminada: false,
    })))
}

impl FuenteVideo for FuenteMatroska {
    fn dimensiones(&self) -> (u32, u32) {
        self.dimensiones
    }

    fn siguiente(&mut self) -> Result<Option<Vec<FotogramaVideo>>, String> {
        if self.terminada {
            return Ok(None);
        }
        loop {
            let hay = self
                .archivo
                .next_frame(&mut self.trama)
                .map_err(|e| format!("WebM dañado: {e}"))?;
            if !hay {
                self.terminada = true;
                return fotogramas_av1(self.decodificador.terminar()?, &self.cfg).map(Some);
            }
            if self.trama.track != self.pista {
                continue;
            }
            let marca = self.trama.timestamp.saturating_mul(self.escala_ns) / NS_POR_MS;
            let datos = std::mem::take(&mut self.trama.data);
            let listos = self
                .decodificador
                .decodificar(datos, Some(i64::try_from(marca).unwrap_or(i64::MAX)))?;
            return fotogramas_av1(listos, &self.cfg).map(Some);
        }
    }
}
