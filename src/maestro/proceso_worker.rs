//! Sub-Workers efímeros: un proceso confinado por recurso.
//!
//! El Maestro lanza el propio ejecutable en modo Worker, le entrega una orden,
//! espera la respuesta con un plazo y lo destruye. Si el proceso muere sin
//! responder, se informa del estado de salida para que el IDS distinga una
//! caída de una violación de seccomp.
//!
//! En Windows el Worker nace suspendido dentro de un AppContainer sin
//! capacidades (sin red) y de un Job Object, y solo entonces arranca
//! (módulo `worker::sandbox::appcontainer`). En Linux su propio confinamiento
//! (seccomp) le prohíbe crear sockets de red.

use crate::configuracion::ConfiguracionWorker;
use crate::error::{ErrorApp, Resultado};
use crate::ipc::mensajes::{OrdenWorker, RespuestaWorker};
use crate::ipc::CanalIpc;
use crate::maestro::medios::productor::CanalMedio;
use crate::maestro::navegacion::FuturoCaja;
use crate::worker::{ARGUMENTO_MODO_WORKER, ARGUMENTO_PARAMETROS_WORKER};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(target_os = "windows")]
use crate::worker::sandbox::appcontainer::{self, ProcesoAppContainer};
#[cfg(target_os = "windows")]
use crate::worker::sandbox::windows::JobObjectGuardian;
#[cfg(not(target_os = "windows"))]
use std::process::{ExitStatus, Stdio};
#[cfg(not(target_os = "windows"))]
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// Plazo para recoger el estado de salida de un Worker que cerró el canal.
const PLAZO_ESTADO_SALIDA: Duration = Duration::from_secs(2);

/// Extremo de lectura del canal con el Worker.
#[cfg(target_os = "windows")]
type Lector = tokio::fs::File;
/// Extremo de escritura del canal con el Worker.
#[cfg(target_os = "windows")]
type Escritor = tokio::fs::File;
/// Extremo de lectura del canal con el Worker.
#[cfg(not(target_os = "windows"))]
type Lector = ChildStdout;
/// Extremo de escritura del canal con el Worker.
#[cfg(not(target_os = "windows"))]
type Escritor = ChildStdin;

/// Proceso Worker en Windows: el AppContainer y el Job que lo contiene. El
/// orden de los campos importa: el proceso se destruye antes de cerrar el Job.
#[cfg(target_os = "windows")]
struct ProcesoWorker {
    proceso: ProcesoAppContainer,
    _job: JobObjectGuardian,
}

/// Proceso Worker en Linux.
#[cfg(not(target_os = "windows"))]
type ProcesoWorker = Child;

/// Subproceso Worker en ejecución.
pub struct SubWorker {
    hijo: ProcesoWorker,
    canal: CanalIpc<Lector, Escritor>,
}

impl SubWorker {
    /// Lanza `ejecutable` en modo Worker con los parámetros serializados.
    ///
    /// # Errors
    /// [`ErrorApp::Proceso`] si no se puede lanzar y [`ErrorApp::Sandbox`] si no
    /// se puede asignar al Job Object (en ese caso el proceso se destruye).
    pub fn lanzar(
        ejecutable: &Path,
        parametros: &str,
        limite_respuesta: usize,
        limite_orden: usize,
        memoria_maxima_bytes: u64,
    ) -> Resultado<Self> {
        let (hijo, salida, entrada) = lanzar_proceso(ejecutable, parametros, memoria_maxima_bytes)?;
        Ok(Self {
            hijo,
            canal: CanalIpc::con_limites(salida, entrada, limite_respuesta, limite_orden),
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
        self.consultar(orden, plazo).await
    }

    /// Entrega `orden` y espera la respuesta como mucho `plazo`, sin consumir
    /// el Worker (para medios, que se piden por bloques al mismo Worker).
    ///
    /// # Errors
    /// Los mismos que [`Self::procesar`].
    pub async fn consultar(
        &mut self,
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
    #[cfg(not(target_os = "windows"))]
    async fn terminacion_anomala(&mut self) -> ErrorApp {
        match tokio::time::timeout(PLAZO_ESTADO_SALIDA, self.hijo.wait()).await {
            Ok(Ok(estado)) => ErrorApp::WorkerTerminado {
                violacion_llamada_sistema: muerto_por_seccomp(estado),
                detalle: estado.to_string(),
            },
            _ => sin_estado_de_salida(),
        }
    }

    /// Construye el error a partir del código de salida del proceso.
    #[cfg(target_os = "windows")]
    async fn terminacion_anomala(&mut self) -> ErrorApp {
        /// Intervalo de sondeo del código de salida (sin bloquear el runtime).
        const SONDEO: Duration = Duration::from_millis(10);
        let limite = tokio::time::Instant::now() + PLAZO_ESTADO_SALIDA;
        while tokio::time::Instant::now() < limite {
            if let Some(codigo) = self.hijo.proceso.esperar(0) {
                return ErrorApp::WorkerTerminado {
                    violacion_llamada_sistema: false,
                    detalle: format!("código de salida {codigo:#x}"),
                };
            }
            tokio::time::sleep(SONDEO).await;
        }
        sin_estado_de_salida()
    }
}

/// Error de un Worker que cerró el canal sin dejar estado de salida.
fn sin_estado_de_salida() -> ErrorApp {
    ErrorApp::WorkerTerminado {
        violacion_llamada_sistema: false,
        detalle: "canal cerrado sin estado de salida".to_string(),
    }
}

/// Lanza el Worker en Linux con tuberías propias.
#[cfg(not(target_os = "windows"))]
fn lanzar_proceso(
    ejecutable: &Path,
    parametros: &str,
    _memoria_maxima_bytes: u64,
) -> Resultado<(ProcesoWorker, Lector, Escritor)> {
    // En Linux el Worker se limita a sí mismo (RLIMIT_AS) al confinarse.
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
    let (Some(entrada), Some(salida)) = (hijo.stdin.take(), hijo.stdout.take()) else {
        return Err(ErrorApp::Proceso(
            "el Worker no expone stdin/stdout".to_string(),
        ));
    };
    Ok((hijo, salida, entrada))
}

/// Lanza el Worker en Windows: AppContainer sin red, dentro de un Job Object.
#[cfg(target_os = "windows")]
fn lanzar_proceso(
    ejecutable: &Path,
    parametros: &str,
    memoria_maxima_bytes: u64,
) -> Resultado<(ProcesoWorker, Lector, Escritor)> {
    let job = JobObjectGuardian::nuevo(memoria_maxima_bytes)?;
    let mut proceso = appcontainer::lanzar(
        ejecutable,
        &[
            ARGUMENTO_MODO_WORKER,
            ARGUMENTO_PARAMETROS_WORKER,
            parametros,
        ],
        &job,
    )?;
    let (Some(entrada), Some(salida)) = (proceso.tomar_entrada(), proceso.tomar_salida()) else {
        return Err(ErrorApp::Proceso(
            "el Worker no expone stdin/stdout".to_string(),
        ));
    };
    Ok((
        ProcesoWorker { proceso, _job: job },
        tokio::fs::File::from_std(salida),
        tokio::fs::File::from_std(entrada),
    ))
}

/// `true` si el proceso murió por SIGSYS (acción `KillProcess` de seccomp).
#[cfg(all(unix, not(target_os = "windows")))]
fn muerto_por_seccomp(estado: ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt;
    estado.signal() == Some(libc::SIGSYS)
}

/// Sin seccomp fuera de Unix.
#[cfg(not(any(unix, target_os = "windows")))]
fn muerto_por_seccomp(_estado: ExitStatus) -> bool {
    false
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
    /// [`ErrorApp::Configuracion`] si la configuración no se puede serializar y
    /// [`ErrorApp::Sandbox`] si no se puede preparar el AppContainer.
    /// En Windows concede además al AppContainer de los Workers permiso para
    /// ejecutar `ejecutable`; si no se puede (por ejemplo, en una unidad sin
    /// permisos de Windows), falla: ningún Worker se ejecuta sin aislamiento.
    pub fn nuevo(ejecutable: PathBuf, configuracion: ConfiguracionWorker) -> Resultado<Self> {
        #[cfg(target_os = "windows")]
        appcontainer::preparar(&ejecutable)?;
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
        self.lanzar()?.procesar(orden, self.plazo()).await
    }

    /// Lanza un Worker dedicado a un medio: vive mientras dure la reproducción
    /// y se destruye al soltar el canal.
    ///
    /// # Errors
    /// Los de [`SubWorker::lanzar`].
    pub fn canal_medio(&self) -> Resultado<CanalSubworker> {
        Ok(CanalSubworker {
            worker: self.lanzar()?,
            plazo: self.plazo(),
        })
    }

    /// Plazo de cada respuesta del Worker.
    fn plazo(&self) -> Duration {
        Duration::from_millis(self.configuracion.tiempo_espera_ms)
    }

    /// Lanza un Worker nuevo.
    fn lanzar(&self) -> Resultado<SubWorker> {
        SubWorker::lanzar(
            &self.ejecutable,
            &self.parametros,
            self.configuracion.limite_mensaje_ipc_bytes,
            self.configuracion.limite_orden_ipc_bytes,
            self.configuracion.memoria_maxima_worker_bytes,
        )
    }
}

/// Canal con un Worker dedicado a un medio.
pub struct CanalSubworker {
    worker: SubWorker,
    plazo: Duration,
}

impl CanalMedio for CanalSubworker {
    fn pedir<'a>(
        &'a mut self,
        orden: &'a OrdenWorker,
    ) -> FuturoCaja<'a, Resultado<RespuestaWorker>> {
        Box::pin(self.worker.consultar(orden, self.plazo))
    }
}
