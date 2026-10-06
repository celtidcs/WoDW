//! Proceso Worker (no privilegiado, confinado).
//!
//! Procesa el contenido recibido de la red en un subproceso confinado que se
//! lanza para cada recurso (sub-Worker efímero) y se destruye al entregar la
//! respuesta. Un exploit en los decodificadores queda atrapado en un proceso
//! sin privilegios y de vida corta.

pub mod audio;
pub mod html;
pub mod imagen;
pub mod sandbox;

use crate::configuracion::{ConfiguracionWodw, ConfiguracionWorker};
use crate::error::{ErrorApp, Resultado};
use crate::ipc::mensajes::{FormatoImagen, IdTareaIpc, OrdenWorker, RespuestaWorker};
use crate::ipc::CanalIpc;
use imagen::RechazoImagen;
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
    sandbox::aplicar_sandbox_proceso_actual()?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(bucle_eventos_worker(
            tokio::io::stdin(),
            tokio::io::stdout(),
            &cfg,
        ))
}

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
    let mut canal = CanalIpc::nuevo(lector, escritor, cfg.limite_mensaje_ipc_bytes);
    while let Some(orden) = canal.recibir::<OrdenWorker>().await? {
        match procesar_orden(orden, cfg) {
            Some(respuesta) => canal.enviar(&respuesta).await?,
            None => break,
        }
    }
    Ok(())
}

/// Procesa una orden. Devuelve `None` ante [`OrdenWorker::Terminar`].
pub fn procesar_orden(orden: OrdenWorker, cfg: &ConfiguracionWorker) -> Option<RespuestaWorker> {
    let respuesta = match orden {
        OrdenWorker::Terminar => return None,
        OrdenWorker::Ping { marca_tiempo } => RespuestaWorker::Pong { marca_tiempo },
        OrdenWorker::ProcesarHtml {
            id_tarea,
            url_origen,
            contenido_html,
        } => {
            let doc = html::sanitizar_html(&contenido_html, &url_origen);
            RespuestaWorker::HtmlProcesado {
                id_tarea,
                titulo: doc.titulo,
                texto_limpio: doc.texto,
                enlaces: doc.enlaces,
            }
        }
        OrdenWorker::ProcesarImagen {
            id_tarea,
            formato,
            datos_crudos,
        } => procesar_imagen(id_tarea, formato, &datos_crudos, cfg),
        OrdenWorker::ProcesarAudio {
            id_tarea,
            datos_crudos,
        } => procesar_audio(id_tarea, &datos_crudos, cfg),
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

/// Decodifica un WAV y aplica el paso bajo configurado.
fn procesar_audio(
    id_tarea: IdTareaIpc,
    datos: &[u8],
    cfg: &ConfiguracionWorker,
) -> RespuestaWorker {
    match audio::decodificar_wav(datos) {
        Ok(pcm) => RespuestaWorker::AudioProcesado {
            id_tarea,
            frecuencia_muestreo: pcm.frecuencia_muestreo,
            canales: pcm.canales,
            muestras_pcm: audio::filtrar_paso_bajo(&pcm, cfg.frecuencia_corte_audio_hz),
        },
        Err(mensaje) => RespuestaWorker::ErrorTarea { id_tarea, mensaje },
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
            ancho_maximo_imagen: 1,
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
