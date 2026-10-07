//! Tarea productora de una reproducción: abre el medio en un sub-Worker sin
//! red, le pide bloques mientras el búfer tenga hueco, valida cada bloque
//! ([`super::validar_bloque`]) y lo deja en el estado compartido.
//!
//! Depende de la abstracción [`CanalMedio`] (en producción, un sub-Worker
//! confinado y de vida limitada a la reproducción), de modo que se prueba sin
//! procesos ni tarjeta de sonido.

use super::reproduccion::{EstadoReproduccion, FaseReproduccion};
use super::validar_bloque;
use crate::configuracion::{ConfiguracionReproduccion, ConfiguracionWorker};
use crate::error::{ErrorApp, Resultado};
use crate::ipc::mensajes::{FamiliaMedio, OrdenWorker, RespuestaWorker};
use crate::maestro::navegacion::FuturoCaja;
use std::time::Duration;

/// Identificador de tarea de la reproducción dentro de su propio sub-Worker.
const ID_TAREA_MEDIO: u64 = 1;
/// Espera entre comprobaciones cuando el búfer está lleno o en pausa.
const ESPERA_BUFER_LLENO: Duration = Duration::from_millis(20);

/// Canal con el Worker que decodifica el medio.
pub trait CanalMedio: Send {
    /// Envía `orden` y espera su respuesta.
    fn pedir<'a>(
        &'a mut self,
        orden: &'a OrdenWorker,
    ) -> FuturoCaja<'a, Resultado<RespuestaWorker>>;
}

/// Reproduce `datos` hasta el final, hasta que se ordene parar o hasta un error.
///
/// # Errors
/// El error que detuvo la reproducción; la fase del estado ya lo refleja. Un
/// [`ErrorApp::ContenidoHostil`] o [`ErrorApp::WorkerTerminado`] es un
/// incidente que el llamador debe tratar como tal.
pub async fn producir(
    canal: &mut impl CanalMedio,
    familia: FamiliaMedio,
    datos: Vec<u8>,
    estado: &EstadoReproduccion,
    cfg_worker: &ConfiguracionWorker,
    cfg: &ConfiguracionReproduccion,
) -> Resultado<()> {
    let resultado = bucle(canal, familia, datos, estado, cfg_worker, cfg).await;
    match &resultado {
        Ok(()) => estado.marcar_produccion_terminada(),
        Err(e) => {
            estado.fijar_fase(FaseReproduccion::Error(e.to_string()));
            estado.parar();
        }
    }
    resultado
}

async fn bucle(
    canal: &mut impl CanalMedio,
    familia: FamiliaMedio,
    datos: Vec<u8>,
    estado: &EstadoReproduccion,
    cfg_worker: &ConfiguracionWorker,
    cfg: &ConfiguracionReproduccion,
) -> Resultado<()> {
    let apertura = canal
        .pedir(&OrdenWorker::AbrirMedio {
            id_tarea: ID_TAREA_MEDIO,
            familia,
            datos_crudos: datos,
        })
        .await?;
    exigir_apertura(apertura)?;
    estado.fijar_fase(FaseReproduccion::Reproduciendo);
    let siguiente = OrdenWorker::SiguienteBloque {
        id_tarea: ID_TAREA_MEDIO,
    };
    while !estado.parado() {
        if estado.audio_pendiente_ms() >= cfg.bufer_audio_ms
            || estado.fotogramas_pendientes() >= cfg.max_fotogramas_en_bufer
        {
            tokio::time::sleep(ESPERA_BUFER_LLENO).await;
            continue;
        }
        let respuesta = exigir_respuesta_valida(canal.pedir(&siguiente).await?)?;
        validar_bloque(&respuesta, cfg_worker)?;
        let RespuestaWorker::BloqueMedio {
            audio_pcm,
            fotogramas,
            fin,
            ..
        } = respuesta
        else {
            return Err(ErrorApp::Proceso(
                "respuesta inesperada del Worker".to_string(),
            ));
        };
        estado.anadir(audio_pcm, fotogramas);
        if fin {
            return Ok(());
        }
    }
    Ok(())
}

/// Exige que el Worker haya abierto el medio.
fn exigir_apertura(respuesta: RespuestaWorker) -> Resultado<()> {
    match exigir_respuesta_valida(respuesta)? {
        RespuestaWorker::MedioAbierto { .. } => Ok(()),
        _ => Err(ErrorApp::Proceso(
            "respuesta inesperada del Worker".to_string(),
        )),
    }
}

/// Convierte los avisos de error del Worker en errores tipados: una alerta de
/// seguridad es contenido hostil (incidente) y un error, un fallo ordinario.
fn exigir_respuesta_valida(respuesta: RespuestaWorker) -> Resultado<RespuestaWorker> {
    match respuesta {
        RespuestaWorker::ErrorTarea { mensaje, .. } => Err(ErrorApp::Proceso(mensaje)),
        RespuestaWorker::AlertaSeguridad {
            vector, mensaje, ..
        } => Err(ErrorApp::ContenidoHostil(format!("{vector}: {mensaje}"))),
        otra => Ok(otra),
    }
}
