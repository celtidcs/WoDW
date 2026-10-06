//! Sub-Workers efímeros: un proceso confinado por recurso.
//!
//! El Maestro lanza el propio ejecutable en modo Worker, le entrega una orden,
//! espera la respuesta con un plazo y lo destruye. Si el proceso muere sin
//! responder, se informa del estado de salida para que el IDS distinga una
//! caída de una violación de seccomp.

use crate::configuracion::ConfiguracionWorker;
use crate::error::{ErrorApp, Resultado};
use crate::ipc::mensajes::{OrdenWorker, RespuestaWorker};
use crate::ipc::CanalIpc;
use crate::worker::{ARGUMENTO_MODO_WORKER, ARGUMENTO_PARAMETROS_WORKER};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::time::Duration;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

#[cfg(target_os = "windows")]
use crate::worker::sandbox::windows::JobObjectGuardian;

/// Plazo para recoger el estado de salida de un Worker que cerró el canal.
const PLAZO_ESTADO_SALIDA: Duration = Duration::from_secs(2);

/// Subproceso Worker en ejecución.
pub struct SubWorker {
    hijo: Child,
    canal: CanalIpc<ChildStdout, ChildStdin>,
    #[cfg(target_os = "windows")]
    _job: JobObjectGuardian,
}

impl SubWorker {
    /// Lanza `ejecutable` en modo Worker con los parámetros serializados.
    ///
    /// # Errors
    /// [`ErrorApp::Proceso`] si no se puede lanzar y [`ErrorApp::Sandbox`] si no
    /// se puede asignar al Job Object (en ese caso el proceso se destruye).
    pub fn lanzar(ejecutable: &Path, parametros: &str, limite_ipc: usize) -> Resultado<Self> {
        let mut hijo = Command::new(ejecutable)
            .arg(ARGUMENTO_MODO_WORKER)
            .arg(ARGUMENTO_PARAMETROS_WORKER)
            .arg(parametros)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ErrorApp::Proceso(format!("no se pudo lanzar el Worker: {e}")))?;
        #[cfg(target_os = "windows")]
        let job = asignar_a_job(&mut hijo)?;
        let entrada = hijo.stdin.take();
        let salida = hijo.stdout.take();
        let (Some(entrada), Some(salida)) = (entrada, salida) else {
            return Err(ErrorApp::Proceso(
                "el Worker no expone stdin/stdout".to_string(),
            ));
        };
        Ok(Self {
            hijo,
            canal: CanalIpc::nuevo(salida, entrada, limite_ipc),
            #[cfg(target_os = "windows")]
            _job: job,
        })
    }

    /// Entrega `orden` y espera la respuesta como mucho `plazo`. Consume el Worker.
    ///
    /// # Errors
    /// [`ErrorApp::TiempoAgotado`] si no responde a tiempo,
    /// [`ErrorApp::WorkerTerminado`] si muere sin responder y los errores IPC.
    pub async fn procesar(
        mut self,
        orden: &OrdenWorker,
        plazo: Duration,
    ) -> Resultado<RespuestaWorker> {
        self.canal.enviar(orden).await?;
        let recepcion = tokio::time::timeout(plazo, self.canal.recibir::<RespuestaWorker>())
            .await
            .map_err(|_| ErrorApp::TiempoAgotado {
                operacion: "procesamiento en el Worker",
                milisegundos: u64::try_from(plazo.as_millis()).unwrap_or(u64::MAX),
            })?;
        match recepcion {
            Ok(Some(respuesta)) => Ok(respuesta),
            Ok(None) | Err(_) => Err(self.terminacion_anomala().await),
        }
    }

    /// Construye el error a partir del estado de salida del proceso.
    async fn terminacion_anomala(&mut self) -> ErrorApp {
        match tokio::time::timeout(PLAZO_ESTADO_SALIDA, self.hijo.wait()).await {
            Ok(Ok(estado)) => ErrorApp::WorkerTerminado {
                violacion_llamada_sistema: muerto_por_seccomp(estado),
                detalle: estado.to_string(),
            },
            _ => ErrorApp::WorkerTerminado {
                violacion_llamada_sistema: false,
                detalle: "canal cerrado sin estado de salida".to_string(),
            },
        }
    }
}

/// `true` si el proceso murió por SIGSYS (acción `KillProcess` de seccomp).
#[cfg(unix)]
fn muerto_por_seccomp(estado: ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt;
    estado.signal() == Some(libc::SIGSYS)
}

/// Sin seccomp fuera de Linux.
#[cfg(not(unix))]
fn muerto_por_seccomp(_estado: ExitStatus) -> bool {
    false
}

/// Asigna el Worker recién lanzado al Job Object; si falla, lo destruye.
#[cfg(target_os = "windows")]
fn asignar_a_job(hijo: &mut Child) -> Resultado<JobObjectGuardian> {
    let asignacion = JobObjectGuardian::nuevo().and_then(|job| {
        let manejador = hijo
            .raw_handle()
            .ok_or_else(|| ErrorApp::Sandbox("el Worker ya terminó".to_string()))?;
        job.asignar_proceso(manejador as isize)?;
        Ok(job)
    });
    if asignacion.is_err() {
        let _ = hijo.start_kill();
    }
    asignacion
}

/// Ejecuta órdenes en sub-Workers efímeros confinados.
#[derive(Debug, Clone)]
pub struct ProcesadorSubworker {
    ejecutable: PathBuf,
    parametros: String,
    configuracion: ConfiguracionWorker,
}

impl ProcesadorSubworker {
    /// Crea el procesador con el ejecutable de WoDW y su configuración de Worker.
    ///
    /// # Errors
    /// [`ErrorApp::Configuracion`] si la configuración no se puede serializar.
    pub fn nuevo(ejecutable: PathBuf, configuracion: ConfiguracionWorker) -> Resultado<Self> {
        let parametros = toml::to_string(&configuracion).map_err(|e| ErrorApp::Configuracion {
            campo: "worker".to_string(),
            motivo: e.to_string(),
        })?;
        Ok(Self {
            ejecutable,
            parametros,
            configuracion,
        })
    }

    /// Procesa una orden en un Worker nuevo que se destruye al terminar.
    ///
    /// # Errors
    /// Los de [`SubWorker::lanzar`] y [`SubWorker::procesar`].
    pub async fn procesar(&self, orden: &OrdenWorker) -> Resultado<RespuestaWorker> {
        let worker = SubWorker::lanzar(
            &self.ejecutable,
            &self.parametros,
            self.configuracion.limite_mensaje_ipc_bytes,
        )?;
        worker
            .procesar(
                orden,
                Duration::from_millis(self.configuracion.tiempo_espera_ms),
            )
            .await
    }
}
