//! Medios reproducibles (audio y vídeo) dentro del Worker.
//!
//! Un medio se abre una vez ([`SesionMedio::abrir`]): se identifica por su
//! firma, se exige que pertenezca a la familia declarada, se revisa su
//! estructura y solo entonces se entrega a los decodificadores. Después se
//! piden bloques ([`SesionMedio::siguiente_bloque`]) de duración acotada, de
//! modo que la memoria no crece con la duración del medio.

pub mod cadena_audio;
pub mod contenedor;
pub mod fuente_audio;
pub mod fuente_video;

use crate::configuracion::ConfiguracionWorker;
use crate::ipc::mensajes::{FamiliaMedio, FotogramaVideo, CANALES_SALIDA, FRECUENCIA_SALIDA_HZ};
use cadena_audio::CadenaAudio;
use contenedor::Contenedor;
use fuente_audio::FuenteAudio;
use fuente_video::{FuenteVideo, RechazoVideo};
use std::collections::VecDeque;
use std::sync::Arc;

/// Margen reservado en cada respuesta para el audio y la serialización.
const MARGEN_RESPUESTA_BYTES: usize = 1024 * 1024;
/// Milisegundos por segundo.
const MS_POR_S: u64 = 1000;

/// Motivo por el que un medio no se abre o no continúa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RechazoMedio {
    /// Datos inválidos, de otra familia, mal estructurados o no admitidos.
    Invalido(String),
    /// Comportamiento hostil (por ejemplo, un decodificador que entra en pánico).
    Hostil(String),
}

/// Lo que se sabe del medio al abrirlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoMedio {
    /// Si tiene audio.
    pub audio: bool,
    /// Dimensiones del vídeo, si tiene.
    pub video: Option<(u32, u32)>,
    /// Duración declarada, si la hay.
    pub duracion_ms: Option<u64>,
}

/// Bloque producido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BloqueProducido {
    /// PCM estéreo de 16 bits a 48 kHz.
    pub audio_pcm: Vec<i16>,
    /// Fotogramas del tramo.
    pub fotogramas: Vec<FotogramaVideo>,
    /// Último bloque.
    pub fin: bool,
}

/// Audio en curso: fuente y cadena.
struct PistaAudio {
    fuente: FuenteAudio,
    cadena: CadenaAudio,
    terminada: bool,
}

/// Vídeo en curso: fuente y fotogramas ya decodificados aún no entregados.
struct PistaVideo {
    fuente: Box<dyn FuenteVideo>,
    pendientes: VecDeque<FotogramaVideo>,
    terminada: bool,
}

/// Medio abierto.
///
/// Cada bloque cubre el tramo `[posicion_ms, posicion_ms + duracion_bloque)`:
/// los fotogramas cuya marca cae en él y el audio hasta su final. Sin pista de
/// audio (o si el audio acaba antes que el vídeo) se rellena con silencio, de
/// modo que el reloj del Maestro, que es el audio, siempre avanza.
pub struct SesionMedio {
    audio: Option<PistaAudio>,
    video: Option<PistaVideo>,
    duracion_bloque_ms: u64,
    posicion_ms: u64,
    /// Muestras estéreo de audio ya entregadas (incluido el silencio).
    audio_entregado: u64,
    max_bytes_fotogramas: usize,
}

/// Identifica el contenedor, exige que sea de la `familia` declarada y revisa
/// su estructura antes de que ningún decodificador lo vea.
fn contenedor_admitido(familia: FamiliaMedio, datos: &[u8]) -> Result<Contenedor, RechazoMedio> {
    let contenedor = contenedor::identificar(datos)
        .filter(|c| c.admitido_en(familia))
        .ok_or_else(|| {
            RechazoMedio::Invalido(format!("el contenido no es un {familia:?} admitido"))
        })?;
    contenedor::revisar_estructura(contenedor, datos).map_err(RechazoMedio::Invalido)?;
    Ok(contenedor)
}

/// Un medio tiene que tener algo reproducible y, si es un vídeo, imagen.
fn exigir_pistas(familia: FamiliaMedio, audio: bool, video: bool) -> Result<(), RechazoMedio> {
    if !audio && !video {
        return Err(RechazoMedio::Invalido(format!(
            "el medio no tiene ninguna pista de {} reproducible",
            if familia == FamiliaMedio::Video {
                "vídeo"
            } else {
                "audio"
            }
        )));
    }
    if familia == FamiliaMedio::Video && !video {
        return Err(RechazoMedio::Invalido(
            "el vídeo no tiene pista de imagen".to_string(),
        ));
    }
    Ok(())
}

impl SesionMedio {
    /// Abre `datos` como medio de la `familia` declarada.
    ///
    /// # Errors
    /// [`RechazoMedio`] si el contenido no es de esa familia, su estructura no
    /// cuadra o no tiene nada reproducible.
    pub fn abrir(
        familia: FamiliaMedio,
        datos: Vec<u8>,
        cfg: &ConfiguracionWorker,
    ) -> Result<(Self, InfoMedio), RechazoMedio> {
        let contenedor = contenedor_admitido(familia, &datos)?;
        let datos: Arc<[u8]> = Arc::from(datos);
        let video = match familia {
            FamiliaMedio::Video => abrir_video(contenedor, datos.clone(), cfg)?,
            FamiliaMedio::Audio => None,
        };
        let audio = abrir_audio(contenedor, datos, cfg)?;
        exigir_pistas(familia, audio.is_some(), video.is_some())?;
        let info = InfoMedio {
            audio: audio.is_some(),
            video: video.as_ref().map(|v| v.fuente.dimensiones()),
            duracion_ms: audio.as_ref().and_then(|a| a.fuente.duracion_ms),
        };
        Ok((
            Self {
                audio,
                video,
                duracion_bloque_ms: u64::from(cfg.duracion_bloque_ms),
                posicion_ms: 0,
                audio_entregado: 0,
                max_bytes_fotogramas: cfg
                    .limite_mensaje_ipc_bytes
                    .saturating_sub(MARGEN_RESPUESTA_BYTES),
            },
            info,
        ))
    }

    /// Produce el siguiente bloque.
    ///
    /// # Errors
    /// [`RechazoMedio`] si el flujo está dañado o un decodificador falla.
    pub fn siguiente_bloque(&mut self) -> Result<BloqueProducido, RechazoMedio> {
        let mut objetivo_ms = self.posicion_ms + self.duracion_bloque_ms;
        let fotogramas = self.fotogramas_hasta(&mut objetivo_ms)?;
        let mut audio_pcm = self.audio_hasta(objetivo_ms)?;
        let video_vivo = self
            .video
            .as_ref()
            .is_some_and(|v| !v.terminada || !v.pendientes.is_empty());
        let audio_vivo = self.audio.as_ref().is_some_and(|a| !a.terminada);
        if !audio_vivo && (video_vivo || !fotogramas.is_empty()) {
            self.rellenar_silencio(&mut audio_pcm, objetivo_ms);
        }
        self.posicion_ms = objetivo_ms;
        Ok(BloqueProducido {
            audio_pcm,
            fotogramas,
            fin: !video_vivo && !audio_vivo,
        })
    }

    /// Fotogramas con marca anterior a `objetivo_ms`, sin pasar del tope de
    /// bytes de una respuesta; si el tope corta el tramo, `objetivo_ms` se
    /// acorta hasta el primer fotograma que no cupo.
    fn fotogramas_hasta(
        &mut self,
        objetivo_ms: &mut u64,
    ) -> Result<Vec<FotogramaVideo>, RechazoMedio> {
        let Some(pista) = self.video.as_mut() else {
            return Ok(Vec::new());
        };
        while !pista.terminada
            && pista
                .pendientes
                .back()
                .is_none_or(|f| f.marca_ms < *objetivo_ms)
        {
            match protegido(|| pista.fuente.siguiente())? {
                Some(nuevos) => pista.pendientes.extend(nuevos),
                None => pista.terminada = true,
            }
        }
        let mut salida = Vec::new();
        let mut bytes = 0usize;
        while let Some(f) = pista
            .pendientes
            .front()
            .filter(|f| f.marca_ms < *objetivo_ms)
        {
            if !salida.is_empty() && bytes + f.rgba.len() > self.max_bytes_fotogramas {
                *objetivo_ms = f.marca_ms.max(self.posicion_ms + 1);
                break;
            }
            bytes += f.rgba.len();
            salida.extend(pista.pendientes.pop_front());
        }
        Ok(salida)
    }

    /// Audio hasta `objetivo_ms` desde el inicio del medio.
    fn audio_hasta(&mut self, objetivo_ms: u64) -> Result<Vec<i16>, RechazoMedio> {
        let objetivo = muestras_hasta(objetivo_ms);
        let mut audio_pcm = Vec::new();
        if let Some(pista) = self.audio.as_mut() {
            while !pista.terminada && self.audio_entregado < objetivo {
                match protegido(|| {
                    pista
                        .fuente
                        .siguiente()
                        .map(|m| m.map(|m| pista.cadena.procesar(m)))
                })? {
                    Some(pcm) => {
                        self.audio_entregado += pcm.len() as u64 / u64::from(CANALES_SALIDA);
                        audio_pcm.extend(pcm);
                    }
                    None => pista.terminada = true,
                }
            }
        }
        Ok(audio_pcm)
    }

    /// Silencio hasta `objetivo_ms` para que el reloj del Maestro siga al vídeo.
    fn rellenar_silencio(&mut self, audio_pcm: &mut Vec<i16>, objetivo_ms: u64) {
        let faltan = muestras_hasta(objetivo_ms).saturating_sub(self.audio_entregado);
        let faltan = usize::try_from(faltan).unwrap_or(0);
        audio_pcm.resize(audio_pcm.len() + faltan * usize::from(CANALES_SALIDA), 0);
        self.audio_entregado += faltan as u64;
    }
}

/// Muestras estéreo a 48 kHz que caben en `ms` milisegundos.
fn muestras_hasta(ms: u64) -> u64 {
    ms * u64::from(FRECUENCIA_SALIDA_HZ) / MS_POR_S
}

/// Abre la pista de vídeo, si la hay.
fn abrir_video(
    contenedor: Contenedor,
    datos: Arc<[u8]>,
    cfg: &ConfiguracionWorker,
) -> Result<Option<PistaVideo>, RechazoMedio> {
    let abierta = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fuente_video::abrir(contenedor, datos, cfg)
    })) {
        Ok(r) => r,
        Err(_) => {
            return Err(RechazoMedio::Hostil(
                "el lector de vídeo falló de forma anómala".to_string(),
            ))
        }
    };
    match abierta {
        Ok(fuente) => Ok(fuente.map(|fuente| PistaVideo {
            fuente,
            pendientes: VecDeque::new(),
            terminada: false,
        })),
        Err(RechazoVideo::Invalido(m)) => Err(RechazoMedio::Invalido(m)),
        Err(RechazoVideo::ExcedeLimites(m)) => Err(RechazoMedio::Hostil(m)),
    }
}

/// Abre la pista de audio, si la hay, y prepara su cadena.
fn abrir_audio(
    contenedor: Contenedor,
    datos: Arc<[u8]>,
    cfg: &ConfiguracionWorker,
) -> Result<Option<PistaAudio>, RechazoMedio> {
    let Some(fuente) = protegido(|| fuente_audio::abrir(contenedor, datos))? else {
        return Ok(None);
    };
    let cadena = CadenaAudio::nueva(fuente.frecuencia, fuente.canales, cfg)
        .map_err(RechazoMedio::Invalido)?;
    Ok(Some(PistaAudio {
        fuente,
        cadena,
        terminada: false,
    }))
}

/// Ejecuta código de un decodificador de terceros: un error es un medio
/// inválido y un pánico, comportamiento hostil (el Worker sigue vivo para
/// informar y después se destruye).
fn protegido<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, RechazoMedio> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(resultado) => resultado.map_err(RechazoMedio::Invalido),
        Err(_) => Err(RechazoMedio::Hostil(
            "un decodificador falló de forma anómala con este medio".to_string(),
        )),
    }
}
