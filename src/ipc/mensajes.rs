//! Definición de mensajes tipados intercambiados entre el Maestro y el Worker.
//!
//! Todas las estructuras son serializables mediante `postcard`. El tamaño máximo
//! de una trama lo fija `worker.limite_mensaje_ipc_bytes` en la configuración.

use serde::{Deserialize, Serialize};

/// Formato declarado de una imagen. El Worker exige que los bytes coincidan
/// con él: un archivo políglota que no lo sea se rechaza.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormatoImagen {
    /// Formato PNG
    Png,
    /// Formato JPEG
    Jpeg,
    /// Formato GIF
    Gif,
    /// Formato WebP
    Webp,
    /// Formato BMP
    Bmp,
    /// Formato TIFF
    Tiff,
    /// Icono de Windows (ICO)
    Ico,
    /// Formato QOI
    Qoi,
    /// Formato AVIF (fotograma AV1 en contenedor HEIF)
    Avif,
}

impl FormatoImagen {
    /// Deduce el formato a partir de un tipo MIME (`image/png`, …).
    pub fn desde_mime(mime: &str) -> Option<Self> {
        match mime {
            "image/png" => Some(Self::Png),
            "image/jpeg" | "image/jpg" => Some(Self::Jpeg),
            "image/gif" => Some(Self::Gif),
            "image/webp" => Some(Self::Webp),
            "image/bmp" => Some(Self::Bmp),
            "image/tiff" => Some(Self::Tiff),
            "image/x-icon" | "image/vnd.microsoft.icon" => Some(Self::Ico),
            "image/qoi" => Some(Self::Qoi),
            "image/avif" => Some(Self::Avif),
            _ => None,
        }
    }
}

/// Frecuencia de muestreo fija del audio que sale del Worker (Hz). Todo audio
/// se remuestrea a ella: lo que haya por encima de 24 kHz desaparece.
pub const FRECUENCIA_SALIDA_HZ: u32 = 48_000;
/// Canales fijos del audio que sale del Worker (estéreo intercalado).
pub const CANALES_SALIDA: u16 = 2;
/// Bytes de cada píxel de las imágenes y fotogramas que entrega el Worker (RGBA).
pub const BYTES_POR_PIXEL_RGBA: usize = 4;

/// `true` si `longitud` bytes son exactamente los píxeles RGBA de una imagen
/// de `ancho`×`alto` (sin desbordar al calcularlo).
pub fn rgba_cuadra(ancho: u32, alto: u32, longitud: usize) -> bool {
    (ancho as usize)
        .checked_mul(alto as usize)
        .and_then(|pixeles| pixeles.checked_mul(BYTES_POR_PIXEL_RGBA))
        == Some(longitud)
}

/// Familia de un medio reproducible, deducida del tipo MIME declarado. El
/// Worker exige que el contenido real pertenezca a ella.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FamiliaMedio {
    /// Audio: MP3, OGG (Vorbis u Opus), FLAC, WAV o AAC/M4A.
    Audio,
    /// Vídeo: MP4 (H.264 + AAC) o WebM (AV1 + Opus/Vorbis).
    Video,
}

impl FamiliaMedio {
    /// Deduce la familia a partir de un tipo MIME sin parámetros.
    pub fn desde_mime(mime: &str) -> Option<Self> {
        match mime {
            "audio/mpeg" | "audio/mp3" | "audio/ogg" | "audio/opus" | "audio/flac"
            | "audio/x-flac" | "audio/wav" | "audio/x-wav" | "audio/wave" | "audio/vnd.wave"
            | "audio/mp4" | "audio/x-m4a" | "audio/aac" | "audio/aacp" => Some(Self::Audio),
            "video/mp4" | "video/webm" => Some(Self::Video),
            _ => None,
        }
    }
}

/// Fotograma de vídeo reconstruido: solo píxeles y su instante.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FotogramaVideo {
    /// Instante de presentación en milisegundos desde el inicio.
    pub marca_ms: u64,
    /// Ancho en píxeles.
    pub ancho: u32,
    /// Alto en píxeles.
    pub alto: u32,
    /// Píxeles RGBA (4 bytes por píxel).
    pub rgba: Vec<u8>,
}

/// Identificador único de tarea IPC para correlación de respuestas asíncronas.
pub type IdTareaIpc = u64;

/// Hipervínculo extraído de un documento y ya resuelto a URL absoluta http/https.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Enlace {
    /// Texto visible del enlace (sanitizado).
    pub texto: String,
    /// URL absoluta de destino.
    pub url: String,
}

/// Tipo de un medio incrustado en una página.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TipoMedioEnlazado {
    /// `<img>`.
    Imagen,
    /// `<audio>` o su `<source>`.
    Audio,
    /// `<video>` o su `<source>`.
    Video,
}

/// Medio incrustado en una página: se lista, no se descarga hasta pulsarlo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MedioEnlazado {
    /// Tipo de elemento que lo incrusta.
    pub tipo: TipoMedioEnlazado,
    /// URL absoluta http/https.
    pub url: String,
    /// Texto alternativo (sanitizado; puede estar vacío).
    pub texto: String,
}

/// Órdenes emitidas por el Proceso Maestro hacia el Proceso Worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrdenWorker {
    /// Solicita procesar y sanitizar un documento HTML en modo Safest.
    ProcesarHtml {
        /// Identificador correlativo de la tarea.
        id_tarea: IdTareaIpc,
        /// URL de origen del documento para resolución relativa de hipervínculos.
        url_origen: String,
        /// Bytes del contenido HTML descargado en memoria por el Maestro.
        contenido_html: Vec<u8>,
    },
    /// Solicita decodificar una imagen a mapa de bits RGBA puro en memoria.
    ProcesarImagen {
        /// Identificador correlativo de la tarea.
        id_tarea: IdTareaIpc,
        /// Formato declarado del medio.
        formato: FormatoImagen,
        /// Bytes crudos de la imagen recibida de la red.
        datos_crudos: Vec<u8>,
    },
    /// Abre un audio o un vídeo para reproducirlo por bloques. El Worker que lo
    /// recibe conserva el medio hasta que se le ordene terminar.
    AbrirMedio {
        /// Identificador correlativo de la tarea.
        id_tarea: IdTareaIpc,
        /// Familia declarada (por el tipo MIME).
        familia: FamiliaMedio,
        /// Bytes crudos del archivo.
        datos_crudos: Vec<u8>,
    },
    /// Pide el siguiente bloque del medio abierto.
    SiguienteBloque {
        /// Identificador correlativo de la tarea.
        id_tarea: IdTareaIpc,
    },
    /// Comprobación periódica de latido del Worker.
    Ping {
        /// Marca de tiempo de emisión.
        marca_tiempo: u64,
    },
    /// Orden de cierre limpio y amigable del Worker.
    Terminar,
}

/// Respuestas generadas por el Proceso Worker hacia el Proceso Maestro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RespuestaWorker {
    /// Resultado de procesamiento de documento HTML sanitizado.
    HtmlProcesado {
        /// Identificador correlativo de la tarea.
        id_tarea: IdTareaIpc,
        /// Título extraído de la página.
        titulo: String,
        /// Texto plano sanitizado (sin secuencias de control ni Bidi).
        texto_limpio: String,
        /// Enlaces http/https absolutos.
        enlaces: Vec<Enlace>,
        /// Imágenes, audios y vídeos incrustados (sin descargar).
        medios: Vec<MedioEnlazado>,
        /// Si el texto superaba el tope de caracteres y se recortó.
        recortado: bool,
    },
    /// Resultado de decodificación de imagen a píxeles RGBA puros.
    ImagenProcesada {
        /// Identificador correlativo de la tarea.
        id_tarea: IdTareaIpc,
        /// Ancho en píxeles.
        ancho: u32,
        /// Alto en píxeles.
        alto: u32,
        /// Búfer de píxeles RGBA (4 bytes por píxel) sin metadatos.
        datos_rgba: Vec<u8>,
    },
    /// Medio abierto y listo para pedir bloques.
    MedioAbierto {
        /// Identificador correlativo de la tarea.
        id_tarea: IdTareaIpc,
        /// Si tiene pista de audio.
        audio: bool,
        /// Dimensiones del vídeo, si tiene.
        video: Option<(u32, u32)>,
        /// Duración declarada por el contenedor, si la declara (solo informativa).
        duracion_ms: Option<u64>,
    },
    /// Bloque del medio: audio a [`FRECUENCIA_SALIDA_HZ`] y [`CANALES_SALIDA`]
    /// ya filtrado y limitado, y los fotogramas reconstruidos del mismo tramo.
    BloqueMedio {
        /// Identificador correlativo de la tarea.
        id_tarea: IdTareaIpc,
        /// Muestras PCM de 16 bits intercaladas.
        audio_pcm: Vec<i16>,
        /// Fotogramas del tramo, en orden de presentación.
        fotogramas: Vec<FotogramaVideo>,
        /// `true` si es el último bloque.
        fin: bool,
    },
    /// Respuesta al ping de latido del Maestro.
    Pong {
        /// Marca de tiempo de respuesta coincidente.
        marca_tiempo: u64,
    },
    /// Notificación de error en una tarea particular sin derribar el proceso.
    ErrorTarea {
        /// Identificador correlativo de la tarea fallida.
        id_tarea: IdTareaIpc,
        /// Descripción del fallo de procesamiento.
        mensaje: String,
    },
    /// Contenido hostil detectado en el sandbox (por ejemplo, una bomba de descompresión).
    AlertaSeguridad {
        /// Identificador correlativo de la tarea afectada.
        id_tarea: IdTareaIpc,
        /// Vector de amenaza detectado.
        vector: String,
        /// Detalle del evento para telemetría IDS.
        mensaje: String,
    },
}
