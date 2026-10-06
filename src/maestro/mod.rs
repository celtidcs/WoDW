//! Proceso Maestro: red Tor, navegación, sub-Workers y sesión en segundo plano.

pub mod navegacion;
pub mod proceso_worker;
pub mod red;
pub mod sesion;

pub use navegacion::{ContenidoPagina, FalloNavegacion, ResultadoNavegacion, ServicioNavegacion};
pub use proceso_worker::{ProcesadorSubworker, SubWorker};
pub use sesion::{iniciar_sesion, ConexionSesion, EstadoTor, EventoSesion, OrdenSesion};
