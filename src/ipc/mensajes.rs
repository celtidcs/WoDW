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
            _ => None,
        }
    }
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
    /// Solicita decodificar un WAV PCM y aplicarle el filtro paso bajo.
    ProcesarAudio {
        /// Identificador correlativo de la tarea.
        id_tarea: IdTareaIpc,
        /// Bytes crudos del contenedor de audio.
        datos_crudos: Vec<u8>,
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
    /// Resultado de procesamiento de audio con filtro paso bajo aplicado.
    AudioProcesado {
        /// Identificador correlativo de la tarea.
        id_tarea: IdTareaIpc,
        /// Frecuencia de muestreo en Hz leída de la cabecera WAV.
        frecuencia_muestreo: u32,
        /// Número de canales leído de la cabecera WAV.
        canales: u16,
        /// Muestras PCM de 16 bits intercaladas y filtradas.
        muestras_pcm: Vec<i16>,
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
