//! Proceso Worker (no privilegiado, confinado).
//!
//! Procesa el contenido recibido de la red en un subproceso confinado que se
//! lanza para cada recurso (sub-Worker efímero) y se destruye al entregar la
//! respuesta. Un exploit en los decodificadores queda atrapado en un proceso
//! sin privilegios y de vida corta.

pub mod av1;
pub mod avif;
mod entidades_html;
pub mod html;
pub mod imagen;
pub mod medios;
pub mod sandbox;
pub mod yuv;

use crate::configuracion::{ConfiguracionWodw, ConfiguracionWorker};
use crate::error::{ErrorApp, Resultado};
use crate::ipc::mensajes::{FormatoImagen, IdTareaIpc, OrdenWorker, RespuestaWorker};
use crate::ipc::CanalIpc;
use imagen::RechazoImagen;
use medios::{RechazoMedio, SesionMedio};
use tokio::io::{AsyncRead, AsyncWrite};

/// Argumento que activa el modo Worker en el ejecutable.
pub const ARGUMENTO_MODO_WORKER: &str = "--modo-worker";
/// Argumento que precede a los parámetros del Worker serializados en TOML.
pub const ARGUMENTO_PARAMETROS_WORKER: &str = "--parametros-worker";

/// Punto de entrada del modo Worker.
///
/// Lee los parámetros de la línea de órdenes, se confina **antes** de crear el
/// runtime asíncrono (así todos los hilos heredan el confinamiento y ningún dato
/// no confiable llega antes) y atiende órdenes por stdin/stdout.
///
/// # Errors
/// [`ErrorApp::Configuracion`] si faltan los parámetros, [`ErrorApp::Sandbox`]
/// si el confinamiento no puede aplicarse y los errores del canal IPC.
pub fn ejecutar_proceso_worker(argumentos: &[String]) -> Resultado<()> {
    let cfg = parametros_desde_argumentos(argumentos)?;
    sandbox::aplicar_sandbox_proceso_actual(cfg.memoria_maxima_worker_bytes)?;
    // Los decodificadores de vídeo usan marcos de pila grandes: el hilo
    // principal de Windows (1 MiB) se desbordaba con H.264 y AV1
    // (STATUS_STACK_OVERFLOW, comprobado). El trabajo va a un hilo con
    // PILA_HILO_WORKER, creado tras confinarse (los hilos están permitidos).
    std::thread::Builder::new()
        .name("wodw-worker".to_string())
        .stack_size(PILA_HILO_WORKER)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(bucle_eventos_worker(
                    tokio::io::stdin(),
                    tokio::io::stdout(),
                    &cfg,
                ))
        })?
        .join()
        .unwrap_or_else(|_| {
            Err(ErrorApp::Proceso(
                "el hilo del Worker terminó de forma anómala".to_string(),
            ))
        })
}

/// Pila del hilo de trabajo del Worker: 16 MiB, holgado para los
/// decodificadores de vídeo incluso en compilación de depuración (con 1 MiB
/// se desbordaba). Es memoria virtual reservada, no ocupada.
const PILA_HILO_WORKER: usize = 16 * 1024 * 1024;

/// Extrae y valida los parámetros del Worker.
fn parametros_desde_argumentos(argumentos: &[String]) -> Resultado<ConfiguracionWorker> {
    let posicion = argumentos
        .iter()
        .position(|a| a == ARGUMENTO_PARAMETROS_WORKER)
        .and_then(|p| argumentos.get(p + 1))
        .ok_or_else(|| ErrorApp::Configuracion {
            campo: ARGUMENTO_PARAMETROS_WORKER.to_string(),
            motivo: "falta el argumento".to_string(),
        })?;
    ConfiguracionWodw::worker_desde_argumento(posicion)
}

/// Bucle de recepción de órdenes y emisión de respuestas.
///
/// # Errors
/// Los errores del canal IPC.
pub async fn bucle_eventos_worker<R, W>(
    lector: R,
    escritor: W,
    cfg: &ConfiguracionWorker,
) -> Resultado<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut canal = CanalIpc::con_limites(
        lector,
        escritor,
        cfg.limite_orden_ipc_bytes,
        cfg.limite_mensaje_ipc_bytes,
    );
    let mut estado = EstadoWorker::nuevo(cfg.clone());
    while let Some(orden) = canal.recibir::<OrdenWorker>().await? {
        match estado.atender(orden) {
            Some(respuesta) => canal.enviar(&respuesta).await?,
            None => break,
        }
    }
    Ok(())
}

/// Procesa una orden en un Worker sin estado previo. Devuelve `None` ante
/// [`OrdenWorker::Terminar`].
pub fn procesar_orden(orden: OrdenWorker, cfg: &ConfiguracionWorker) -> Option<RespuestaWorker> {
    EstadoWorker::nuevo(cfg.clone()).atender(orden)
}

/// Estado de un Worker: su configuración y, si lo hay, el medio abierto.
pub struct EstadoWorker {
    cfg: ConfiguracionWorker,
    medio: Option<(IdTareaIpc, SesionMedio)>,
}

impl EstadoWorker {
    /// Worker sin medio abierto.
    pub fn nuevo(cfg: ConfiguracionWorker) -> Self {
        Self { cfg, medio: None }
    }

    /// Atiende una orden. Devuelve `None` ante [`OrdenWorker::Terminar`].
    pub fn atender(&mut self, orden: OrdenWorker) -> Option<RespuestaWorker> {
        let respuesta = match orden {
            OrdenWorker::AbrirMedio {
                id_tarea,
                familia,
                datos_crudos,
            } => self.abrir_medio(id_tarea, familia, datos_crudos),
            OrdenWorker::SiguienteBloque { id_tarea } => self.siguiente_bloque(id_tarea),
            otra => return atender_sin_estado(otra, &self.cfg),
        };
        Some(respuesta)
    }

    /// Abre un medio y lo conserva para pedirle bloques.
    fn abrir_medio(
        &mut self,
        id_tarea: IdTareaIpc,
        familia: crate::ipc::mensajes::FamiliaMedio,
        datos: Vec<u8>,
    ) -> RespuestaWorker {
        self.medio = None;
        match SesionMedio::abrir(familia, datos, &self.cfg) {
            Ok((sesion, info)) => {
                self.medio = Some((id_tarea, sesion));
                RespuestaWorker::MedioAbierto {
                    id_tarea,
                    audio: info.audio,
                    video: info.video,
                    duracion_ms: info.duracion_ms,
                }
            }
            Err(rechazo) => respuesta_rechazo(id_tarea, rechazo),
        }
    }

    /// Siguiente bloque del medio abierto con ese identificador.
    fn siguiente_bloque(&mut self, id_tarea: IdTareaIpc) -> RespuestaWorker {
        let Some((_, sesion)) = self.medio.as_mut().filter(|(id, _)| *id == id_tarea) else {
            return RespuestaWorker::ErrorTarea {
                id_tarea,
                mensaje: "no hay un medio abierto con ese identificador".to_string(),
            };
        };
        match sesion.siguiente_bloque() {
            Ok(bloque) => RespuestaWorker::BloqueMedio {
                id_tarea,
                audio_pcm: bloque.audio_pcm,
                fotogramas: bloque.fotogramas,
                fin: bloque.fin,
            },
            Err(rechazo) => {
                self.medio = None;
                respuesta_rechazo(id_tarea, rechazo)
            }
        }
    }
}

/// Traduce un rechazo de medio a respuesta.
fn respuesta_rechazo(id_tarea: IdTareaIpc, rechazo: RechazoMedio) -> RespuestaWorker {
    match rechazo {
        RechazoMedio::Invalido(mensaje) => RespuestaWorker::ErrorTarea { id_tarea, mensaje },
        RechazoMedio::Hostil(mensaje) => RespuestaWorker::AlertaSeguridad {
            id_tarea,
            vector: "decodificador de medios".to_string(),
            mensaje,
        },
    }
}

/// Órdenes que no dependen de un medio abierto.
fn atender_sin_estado(orden: OrdenWorker, cfg: &ConfiguracionWorker) -> Option<RespuestaWorker> {
    let respuesta = match orden {
        OrdenWorker::Terminar => return None,
        OrdenWorker::Ping { marca_tiempo } => RespuestaWorker::Pong { marca_tiempo },
        OrdenWorker::ProcesarHtml {
            id_tarea,
            url_origen,
            contenido_html,
        } => {
            let doc = html::sanitizar_html(
                &contenido_html,
                &url_origen,
                html::LimitesHtml {
                    max_caracteres: cfg.max_caracteres_texto,
                    max_medios: cfg.max_medios_por_pagina,
                },
            );
            RespuestaWorker::HtmlProcesado {
                id_tarea,
                titulo: doc.titulo,
                texto_limpio: doc.texto,
                enlaces: doc.enlaces,
                medios: doc.medios,
                recortado: doc.recortado,
            }
        }
        OrdenWorker::ProcesarImagen {
            id_tarea,
            formato,
            datos_crudos,
        } => procesar_imagen(id_tarea, formato, &datos_crudos, cfg),
        OrdenWorker::AbrirMedio { id_tarea, .. } | OrdenWorker::SiguienteBloque { id_tarea } => {
            RespuestaWorker::ErrorTarea {
                id_tarea,
                mensaje: "orden de medio fuera de un Worker con estado".to_string(),
            }
        }
    };
    Some(respuesta)
}

/// Decodifica una imagen; una bomba de descompresión se notifica como alerta.
fn procesar_imagen(
    id_tarea: IdTareaIpc,
    formato: FormatoImagen,
    datos: &[u8],
    cfg: &ConfiguracionWorker,
) -> RespuestaWorker {
    match imagen::decodificar_imagen(formato, datos, cfg) {
        Ok(img) => RespuestaWorker::ImagenProcesada {
            id_tarea,
            ancho: img.ancho,
            alto: img.alto,
            datos_rgba: img.rgba,
        },
        Err(RechazoImagen::ExcedeLimites(mensaje)) => RespuestaWorker::AlertaSeguridad {
            id_tarea,
            vector: "bomba de descompresión".to_string(),
            mensaje,
        },
        Err(RechazoImagen::Invalida(mensaje)) => RespuestaWorker::ErrorTarea { id_tarea, mensaje },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn bucle_responde_ping_y_termina() {
        let cfg = ConfiguracionWorker::default();
        let (maestro, worker) = duplex(4096);
        let (lm, em) = tokio::io::split(maestro);
        let (lw, ew) = tokio::io::split(worker);
        let mut canal = CanalIpc::nuevo(lm, em, cfg.limite_mensaje_ipc_bytes);
        let cfg_worker = cfg.clone();
        let tarea = tokio::spawn(async move { bucle_eventos_worker(lw, ew, &cfg_worker).await });
        canal
            .enviar(&OrdenWorker::Ping { marca_tiempo: 9 })
            .await
            .unwrap();
        let r: Option<RespuestaWorker> = canal.recibir().await.unwrap();
        assert_eq!(r, Some(RespuestaWorker::Pong { marca_tiempo: 9 }));
        canal.enviar(&OrdenWorker::Terminar).await.unwrap();
        tarea.await.unwrap().unwrap();
    }

    #[test]
    fn imagen_que_excede_limites_es_alerta() {
        let cfg = ConfiguracionWorker {
            lado_largo_maximo_px: 1,
            lado_corto_maximo_px: 1,
            ..Default::default()
        };
        let png = {
            let img = image::RgbaImage::new(4, 4);
            let mut c = std::io::Cursor::new(Vec::new());
            img.write_to(&mut c, image::ImageFormat::Png).unwrap();
            c.into_inner()
        };
        assert!(matches!(
            procesar_orden(
                OrdenWorker::ProcesarImagen {
                    id_tarea: 1,
                    formato: FormatoImagen::Png,
                    datos_crudos: png
                },
                &cfg
            ),
            Some(RespuestaWorker::AlertaSeguridad { .. })
        ));
    }

    #[test]
    fn falta_de_parametros_es_error() {
        assert!(
            parametros_desde_argumentos(&["wodw".into(), ARGUMENTO_MODO_WORKER.into()]).is_err()
        );
    }
}
