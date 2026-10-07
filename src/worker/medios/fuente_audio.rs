//! Pista de audio de un contenedor ya identificado y revisado: lee paquetes y
//! los decodifica a muestras `f32` intercaladas.
//!
//! Desmultiplexado y decodificación con `symphonia` (Rust, sin `unsafe`); Opus
//! con `opus-decoder` (Rust, sin `unsafe`). Se instancia el lector del
//! contenedor concreto, nunca un detector genérico que pruebe todos.

use super::contenedor::Contenedor;
use opus_decoder::OpusDecoder;
use std::io::Cursor;
use std::sync::Arc;
use symphonia::core::codecs::audio::well_known::CODEC_ID_OPUS;
use symphonia::core::codecs::audio::{AudioCodecParameters, AudioDecoder, AudioDecoderOptions};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::packet::Packet;
use symphonia::default::formats::{
    AdtsReader, FlacReader, IsoMp4Reader, MkvReader, MpaReader, OggReader, WavReader,
};

/// Frecuencia a la que se decodifica Opus (la nativa del códec).
const FRECUENCIA_OPUS: u32 = 48_000;
/// Muestras máximas por canal de un paquete Opus (120 ms a 48 kHz).
const MAX_MUESTRAS_OPUS: usize = 5_760;
/// Paquetes dañados seguidos tolerados antes de dar el flujo por inválido.
const MAX_ERRORES_SEGUIDOS: u32 = 16;

/// Decodificador de la pista.
enum Decodificador {
    Symphonia(Box<dyn AudioDecoder>),
    Opus(Box<OpusDecoder>),
}

/// Pista de audio abierta.
pub struct FuenteAudio {
    lector: Box<dyn FormatReader>,
    pista: u32,
    decodificador: Decodificador,
    /// Frecuencia de muestreo de origen.
    pub frecuencia: u32,
    /// Canales de origen.
    pub canales: usize,
    /// Duración declarada, si la hay.
    pub duracion_ms: Option<u64>,
    errores_seguidos: u32,
    muestras: Vec<f32>,
}

/// Abre la pista de audio del contenedor; `Ok(None)` si no tiene audio.
///
/// # Errors
/// Descripción si el contenedor no se puede leer o el códec no está admitido.
pub fn abrir(contenedor: Contenedor, datos: Arc<[u8]>) -> Result<Option<FuenteAudio>, String> {
    let flujo = MediaSourceStream::new(
        Box::new(Cursor::new(datos)),
        MediaSourceStreamOptions::default(),
    );
    let opciones = FormatOptions::default();
    let error = |e: symphonia::core::errors::Error| format!("contenedor de audio ilegible: {e}");
    let lector: Box<dyn FormatReader> = match contenedor {
        Contenedor::Mp3 => Box::new(MpaReader::try_new(flujo, opciones).map_err(error)?),
        Contenedor::Adts => Box::new(AdtsReader::try_new(flujo, opciones).map_err(error)?),
        Contenedor::Ogg => Box::new(OggReader::try_new(flujo, opciones).map_err(error)?),
        Contenedor::Flac => Box::new(FlacReader::try_new(flujo, opciones).map_err(error)?),
        Contenedor::Wav => Box::new(WavReader::try_new(flujo, opciones).map_err(error)?),
        Contenedor::Mp4 => Box::new(IsoMp4Reader::try_new(flujo, opciones).map_err(error)?),
        Contenedor::Matroska => Box::new(MkvReader::try_new(flujo, opciones).map_err(error)?),
    };
    let Some(pista) = lector.default_track(TrackType::Audio) else {
        return Ok(None);
    };
    let Some(CodecParameters::Audio(parametros)) = pista.codec_params.clone() else {
        return Ok(None);
    };
    let id = pista.id;
    let duracion_ms = duracion(pista.num_frames, parametros.sample_rate);
    let (decodificador, frecuencia, canales) = decodificador_para(&parametros)?;
    Ok(Some(FuenteAudio {
        lector,
        pista: id,
        decodificador,
        frecuencia,
        canales,
        duracion_ms,
        errores_seguidos: 0,
        muestras: Vec::new(),
    }))
}

/// Duración en milisegundos a partir de muestras y frecuencia.
fn duracion(muestras: Option<u64>, frecuencia: Option<u32>) -> Option<u64> {
    let frecuencia = u64::from(frecuencia.filter(|&f| f > 0)?);
    muestras?
        .checked_mul(crate::unidades::MILISEGUNDOS_POR_SEGUNDO)
        .map(|m| m / frecuencia)
}

/// Crea el decodificador del códec y devuelve también frecuencia y canales.
fn decodificador_para(p: &AudioCodecParameters) -> Result<(Decodificador, u32, usize), String> {
    let canales = p.channels.as_ref().map_or(0, |c| c.count());
    if p.codec == CODEC_ID_OPUS {
        let decodificador = OpusDecoder::new(FRECUENCIA_OPUS, canales)
            .map_err(|e| format!("Opus de {canales} canales no admitido: {e:?}"))?;
        return Ok((
            Decodificador::Opus(Box::new(decodificador)),
            FRECUENCIA_OPUS,
            canales,
        ));
    }
    let frecuencia = p
        .sample_rate
        .ok_or("pista de audio sin frecuencia de muestreo")?;
    let decodificador = symphonia::default::get_codecs()
        .make_audio_decoder(p, &AudioDecoderOptions::default())
        .map_err(|e| format!("códec de audio no admitido: {e}"))?;
    Ok((Decodificador::Symphonia(decodificador), frecuencia, canales))
}

impl FuenteAudio {
    /// Muestras intercaladas del siguiente paquete; `Ok(None)` al terminar.
    ///
    /// # Errors
    /// Descripción si el flujo está dañado más allá de lo tolerable.
    pub fn siguiente(&mut self) -> Result<Option<&[f32]>, String> {
        loop {
            let paquete = match self.lector.next_packet() {
                Ok(Some(paquete)) => paquete,
                Ok(None) => return Ok(None),
                Err(e) => return Err(format!("flujo de audio dañado: {e}")),
            };
            if paquete.track_id != self.pista {
                continue;
            }
            match self.decodificar(&paquete) {
                Ok(()) => {
                    self.errores_seguidos = 0;
                    return Ok(Some(&self.muestras));
                }
                Err(e) => {
                    self.errores_seguidos += 1;
                    if self.errores_seguidos > MAX_ERRORES_SEGUIDOS {
                        return Err(format!("demasiados paquetes de audio dañados: {e}"));
                    }
                }
            }
        }
    }

    /// Decodifica un paquete en `self.muestras`.
    fn decodificar(&mut self, paquete: &Packet) -> Result<(), String> {
        self.muestras.clear();
        match &mut self.decodificador {
            Decodificador::Symphonia(d) => {
                let bufer = d.decode(paquete).map_err(|e| e.to_string())?;
                bufer.copy_to_vec_interleaved(&mut self.muestras);
            }
            Decodificador::Opus(d) => {
                self.muestras.resize(MAX_MUESTRAS_OPUS * self.canales, 0.0);
                let por_canal = d
                    .decode_float(&paquete.data, &mut self.muestras, false)
                    .map_err(|e| format!("{e:?}"))?;
                self.muestras.truncate(por_canal * self.canales);
            }
        }
        Ok(())
    }
}
