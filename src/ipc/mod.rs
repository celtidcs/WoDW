//! Protocolo de comunicación inter-procesos (IPC) entre Maestro y Worker.

pub mod canal;
pub mod mensajes;

pub use canal::{escribir_mensaje_framed, leer_mensaje_framed, CanalIpc};
pub use mensajes::{
    Enlace, FormatoImagen, IdTareaIpc, MedioEnlazado, OrdenWorker, RespuestaWorker,
    TipoMedioEnlazado,
};
