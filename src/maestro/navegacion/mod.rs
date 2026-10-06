//! Servicio de navegación del Maestro: descarga, redirecciones, clasificación
//! del contenido, procesamiento confinado y respuesta automática a incidentes.
//!
//! Depende de dos abstracciones (DIP): [`FuenteHttp`] (en producción, el
//! cliente Tor aislado por pestaña) y [`ProcesadorContenido`] (en producción,
//! sub-Workers efímeros confinados). Así se prueba sin red ni subprocesos.

pub mod incidentes;

use crate::error::{ErrorApp, Resultado};
use crate::ipc::mensajes::{Enlace, FormatoImagen, OrdenWorker, RespuestaWorker};
use crate::maestro::proceso_worker::ProcesadorSubworker;
use crate::maestro::red::{ClienteTor, RespuestaHttp};
use incidentes::{clasificar, RespuestaAutomatica};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use url::Url;

/// Futuro en caja para métodos asíncronos de rasgos con objetos.
pub type FuturoCaja<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Origen de respuestas HTTP con aislamiento por pestaña.
pub trait FuenteHttp: Send + Sync {
    /// Descarga `url` por el aislamiento de la pestaña `id_pestana`.
    fn obtener<'a>(
        &'a self,
        id_pestana: u64,
        url: &'a Url,
    ) -> FuturoCaja<'a, Resultado<RespuestaHttp>>;
}

/// Procesador de contenido no confiable.
pub trait ProcesadorContenido: Send + Sync {
    /// Procesa `orden` y devuelve la respuesta del Worker.
    fn procesar<'a>(&'a self, orden: &'a OrdenWorker)
        -> FuturoCaja<'a, Resultado<RespuestaWorker>>;
}

impl<T: FuenteHttp> FuenteHttp for std::sync::Arc<T> {
    fn obtener<'a>(
        &'a self,
        id_pestana: u64,
        url: &'a Url,
    ) -> FuturoCaja<'a, Resultado<RespuestaHttp>> {
        (**self).obtener(id_pestana, url)
    }
}

impl FuenteHttp for ClienteTor {
    fn obtener<'a>(
        &'a self,
        id_pestana: u64,
        url: &'a Url,
    ) -> FuturoCaja<'a, Resultado<RespuestaHttp>> {
        Box::pin(ClienteTor::obtener(self, id_pestana, url))
    }
}

impl ProcesadorContenido for ProcesadorSubworker {
    fn procesar<'a>(
        &'a self,
        orden: &'a OrdenWorker,
    ) -> FuturoCaja<'a, Resultado<RespuestaWorker>> {
        Box::pin(ProcesadorSubworker::procesar(self, orden))
    }
}

/// Contenido presentable de una página.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContenidoPagina {
    /// Documento de texto sanitizado.
    Documento {
        /// Título.
        titulo: String,
        /// Texto visible.
        texto: String,
        /// Enlaces http/https.
        enlaces: Vec<Enlace>,
    },
    /// Imagen decodificada a RGBA.
    Imagen {
        /// Ancho.
        ancho: u32,
        /// Alto.
        alto: u32,
        /// Píxeles RGBA.
        rgba: Vec<u8>,
    },
    /// Audio filtrado (sin reproducción: solo se informa).
    Audio {
        /// Frecuencia de muestreo.
        frecuencia_muestreo: u32,
        /// Canales.
        canales: u16,
        /// Muestras por canal.
        muestras_por_canal: usize,
    },
    /// Tipo no soportado: no se procesa ni se descarga a disco.
    NoSoportado {
        /// Tipo MIME recibido.
        tipo_mime: String,
    },
}

/// Resultado de una navegación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoNavegacion {
    /// URL final tras redirecciones.
    pub url: String,
    /// Código HTTP de la respuesta final.
    pub codigo_estado: u16,
    /// Contenido presentable.
    pub contenido: ContenidoPagina,
}

/// Fallo de navegación ya tratado por la respuesta automática.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FalloNavegacion {
    /// Mensaje para el usuario.
    pub mensaje: String,
    /// Si la pestaña debe purgarse.
    pub purgar_pestana: bool,
}

/// Servicio de navegación.
pub struct ServicioNavegacion<F, P> {
    fuente: F,
    procesador: P,
    respuesta: RespuestaAutomatica,
    max_redirecciones: u8,
    siguiente_tarea: AtomicU64,
}

impl<F: FuenteHttp, P: ProcesadorContenido> ServicioNavegacion<F, P> {
    /// Crea el servicio.
    pub fn nuevo(
        fuente: F,
        procesador: P,
        respuesta: RespuestaAutomatica,
        max_redirecciones: u8,
    ) -> Self {
        Self {
            fuente,
            procesador,
            respuesta,
            max_redirecciones,
            siguiente_tarea: AtomicU64::new(1),
        }
    }

    /// Fuente HTTP subyacente.
    pub fn fuente(&self) -> &F {
        &self.fuente
    }

    /// Procesador de contenido subyacente.
    pub fn procesador(&self) -> &P {
        &self.procesador
    }

    /// Respuesta automática (bloqueos de la sesión).
    pub fn respuesta_automatica(&self) -> &RespuestaAutomatica {
        &self.respuesta
    }

    /// Navega a `direccion` desde la pestaña `id_pestana`.
    ///
    /// # Errors
    /// [`FalloNavegacion`] con el mensaje para el usuario; si el fallo es un
    /// incidente, ya se ha bloqueado el host y notificado al IDS.
    pub async fn navegar(
        &self,
        id_pestana: u64,
        direccion: &str,
    ) -> Result<ResultadoNavegacion, FalloNavegacion> {
        let url = Url::parse(direccion).map_err(|e| {
            fallo(&ErrorApp::EntradaInvalida {
                campo: "url",
                motivo: e.to_string(),
            })
        })?;
        let (url, respuesta) = self.descargar(id_pestana, url).await?;
        let codigo_estado = respuesta.codigo_estado();
        let contenido = self.procesar(&url, respuesta).await?;
        Ok(ResultadoNavegacion {
            url: url.to_string(),
            codigo_estado,
            contenido,
        })
    }

    /// Descarga siguiendo redirecciones hasta el máximo configurado.
    async fn descargar(
        &self,
        id_pestana: u64,
        mut url: Url,
    ) -> Result<(Url, RespuestaHttp), FalloNavegacion> {
        for _ in 0..=self.max_redirecciones {
            let host = url.host_str().unwrap_or_default().to_string();
            if self.respuesta.esta_bloqueado(&host) {
                return Err(fallo(&ErrorApp::HostBloqueado(host)));
            }
            let respuesta = self
                .fuente
                .obtener(id_pestana, &url)
                .await
                .map_err(|e| self.tratar(&host, e))?;
            match respuesta
                .cabecera("location")
                .filter(|_| respuesta.es_redireccion())
            {
                Some(destino) => {
                    url = url.join(destino).map_err(|e| {
                        fallo(&ErrorApp::ProtocoloHttp(format!(
                            "redirección inválida: {e}"
                        )))
                    })?;
                }
                None => return Ok((url, respuesta)),
            }
        }
        Err(fallo(&ErrorApp::ProtocoloHttp(format!(
            "más de {} redirecciones",
            self.max_redirecciones
        ))))
    }

    /// Clasifica el contenido y lo procesa en el Worker si procede.
    async fn procesar(
        &self,
        url: &Url,
        respuesta: RespuestaHttp,
    ) -> Result<ContenidoPagina, FalloNavegacion> {
        let host = url.host_str().unwrap_or_default();
        let id_tarea = self.siguiente_tarea.fetch_add(1, Ordering::Relaxed);
        let tipo = tipo_mime(&respuesta);
        let Some(orden) = orden_para(&tipo, id_tarea, url, respuesta.en_cuerpo()) else {
            return Ok(ContenidoPagina::NoSoportado { tipo_mime: tipo });
        };
        let resultado = self
            .procesador
            .procesar(&orden)
            .await
            .and_then(contenido_desde_respuesta);
        resultado.map_err(|e| self.tratar(host, e))
    }

    /// Aplica la respuesta automática si el error es un incidente.
    fn tratar(&self, host: &str, error: ErrorApp) -> FalloNavegacion {
        match clasificar(host, &error) {
            Some(incidente) => {
                self.respuesta.responder(host, &incidente);
                FalloNavegacion {
                    mensaje: format!("{error}. Respuesta automática aplicada: el sitio queda bloqueado en esta sesión."),
                    purgar_pestana: incidente.purgar_pestana,
                }
            }
            None => fallo(&error),
        }
    }
}

/// Fallo ordinario sin incidente.
fn fallo(error: &ErrorApp) -> FalloNavegacion {
    FalloNavegacion {
        mensaje: error.to_string(),
        purgar_pestana: false,
    }
}

/// Tipo MIME sin parámetros; sin cabecera se asume HTML (solo se extrae texto).
fn tipo_mime(respuesta: &RespuestaHttp) -> String {
    respuesta
        .cabecera("content-type")
        .unwrap_or("text/html")
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

/// Bytes por píxel RGBA.
const BYTES_POR_PIXEL_RGBA: usize = 4;

/// Tipos tratados como documento de texto.
const TIPOS_DOCUMENTO: &[&str] = &["text/html", "application/xhtml+xml", "text/plain"];
/// Tipos tratados como audio WAV.
const TIPOS_WAV: &[&str] = &["audio/wav", "audio/x-wav", "audio/wave", "audio/vnd.wave"];

/// Orden de Worker adecuada para el tipo, o `None` si no se soporta.
fn orden_para(tipo: &str, id_tarea: u64, url: &Url, cuerpo: Vec<u8>) -> Option<OrdenWorker> {
    if TIPOS_DOCUMENTO.contains(&tipo) {
        return Some(OrdenWorker::ProcesarHtml {
            id_tarea,
            url_origen: url.to_string(),
            contenido_html: cuerpo,
        });
    }
    if let Some(formato) = FormatoImagen::desde_mime(tipo) {
        return Some(OrdenWorker::ProcesarImagen {
            id_tarea,
            formato,
            datos_crudos: cuerpo,
        });
    }
    TIPOS_WAV
        .contains(&tipo)
        .then_some(OrdenWorker::ProcesarAudio {
            id_tarea,
            datos_crudos: cuerpo,
        })
}

/// Traduce la respuesta del Worker a contenido o error tipado.
fn contenido_desde_respuesta(respuesta: RespuestaWorker) -> Resultado<ContenidoPagina> {
    match respuesta {
        RespuestaWorker::HtmlProcesado {
            titulo,
            texto_limpio,
            enlaces,
            ..
        } => Ok(ContenidoPagina::Documento {
            titulo,
            texto: texto_limpio,
            enlaces,
        }),
        RespuestaWorker::ImagenProcesada {
            ancho,
            alto,
            datos_rgba,
            ..
        } => {
            // La respuesta del Worker tampoco es confiable: un Worker comprometido
            // podría declarar dimensiones incoherentes con el búfer.
            let esperado = (ancho as usize)
                .checked_mul(alto as usize)
                .and_then(|pixeles| pixeles.checked_mul(BYTES_POR_PIXEL_RGBA));
            if esperado != Some(datos_rgba.len()) {
                return Err(ErrorApp::ContenidoHostil(
                    "imagen con dimensiones incoherentes con sus píxeles".to_string(),
                ));
            }
            Ok(ContenidoPagina::Imagen {
                ancho,
                alto,
                rgba: datos_rgba,
            })
        }
        RespuestaWorker::AudioProcesado {
            frecuencia_muestreo,
            canales,
            muestras_pcm,
            ..
        } => Ok(ContenidoPagina::Audio {
            frecuencia_muestreo,
            canales,
            muestras_por_canal: muestras_pcm.len() / usize::from(canales.max(1)),
        }),
        RespuestaWorker::AlertaSeguridad {
            vector, mensaje, ..
        } => Err(ErrorApp::ContenidoHostil(format!("{vector}: {mensaje}"))),
        RespuestaWorker::ErrorTarea { mensaje, .. } => Err(ErrorApp::Proceso(mensaje)),
        RespuestaWorker::Pong { .. } => Err(ErrorApp::Proceso(
            "respuesta inesperada del Worker".to_string(),
        )),
    }
}
