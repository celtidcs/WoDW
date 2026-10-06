//! Gestión centralizada y tipada de errores para WoDW.
//!
//! Define los tipos de error del sistema para garantizar robustez,
//! fallo temprano (fail-fast) y trazabilidad contextual. Las variantes que
//! describen límites, plazos o entradas inválidas llevan campos estructurados
//! para que la capa exterior pueda decidir sin analizar cadenas de texto.

use thiserror::Error;

/// Error principal de la aplicación WoDW.
#[derive(Debug, Error)]
pub enum ErrorApp {
    /// Error en la capa de red Tor o cliente Arti embebido.
    #[error("Error en motor de red Arti: {0}")]
    RedArti(String),

    /// Error de configuración o inicialización de circuitos Tor.
    #[error("Error de configuración de circuitos Tor: {0}")]
    CircuitoTor(String),

    /// Respuesta HTTP que viola el protocolo o es imposible de interpretar.
    #[error("Respuesta HTTP inválida: {0}")]
    ProtocoloHttp(String),

    /// Un recurso remoto o local superó un límite de seguridad.
    #[error("Límite de seguridad excedido en {recurso}: máximo {limite} bytes")]
    LimiteExcedido {
        /// Recurso que superó el límite (cabeceras, cuerpo, mensaje IPC…).
        recurso: &'static str,
        /// Límite configurado que se ha superado.
        limite: usize,
    },

    /// Una operación de E/S no terminó dentro del plazo configurado.
    #[error("Tiempo agotado en {operacion} tras {milisegundos} ms")]
    TiempoAgotado {
        /// Operación que se interrumpió.
        operacion: &'static str,
        /// Plazo configurado en milisegundos.
        milisegundos: u64,
    },

    /// Entrada rechazada en la frontera del sistema (fail fast).
    #[error("Entrada inválida en {campo}: {motivo}")]
    EntradaInvalida {
        /// Campo o parámetro rechazado.
        campo: &'static str,
        /// Motivo legible del rechazo.
        motivo: String,
    },

    /// Configuración TOML inválida o ilegible.
    #[error("Configuración inválida en {campo}: {motivo}")]
    Configuracion {
        /// Ruta del campo dentro del TOML (por ejemplo `red.limite_cuerpo_bytes`).
        campo: String,
        /// Motivo legible del rechazo.
        motivo: String,
    },

    /// Fallo al lanzar, supervisar o terminar un subproceso Worker.
    #[error("Fallo en subproceso Worker: {0}")]
    Proceso(String),

    /// El sub-Worker terminó sin responder mientras procesaba contenido.
    #[error("El Worker terminó de forma anómala: {detalle}")]
    WorkerTerminado {
        /// `true` si lo mató el filtro seccomp (intento de llamada al sistema prohibida).
        violacion_llamada_sistema: bool,
        /// Estado de salida legible.
        detalle: String,
    },

    /// El Worker rechazó el contenido por considerarlo hostil.
    #[error("Contenido hostil detectado por el Worker: {0}")]
    ContenidoHostil(String),

    /// El host está bloqueado en esta sesión por un incidente previo.
    #[error("Host bloqueado en esta sesión por un incidente de seguridad: {0}")]
    HostBloqueado(String),

    /// Error en la comunicación interproceso (IPC) entre Maestro y Worker.
    #[error("Fallo en canal IPC: {0}")]
    Ipc(String),

    /// Error en el confinamiento o sandboxing del sistema operativo.
    #[error("Fallo al aplicar sandboxing del sistema operativo: {0}")]
    Sandbox(String),

    /// Violación de política de seguridad detectada por el IDS.
    #[error("Violación de política de seguridad IDS: {0}")]
    IdsViolacion(String),

    /// Error de entrada/salida (I/O).
    #[error("Error de E/S del sistema: {0}")]
    Io(#[from] std::io::Error),
}

/// Alias de Result estándar para el proyecto WoDW.
pub type Resultado<T> = Result<T, ErrorApp>;
