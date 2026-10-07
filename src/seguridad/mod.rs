//! Tecnología de engaño, purga amnésica, mitigaciones reactivas e higiene del
//! contenido mostrado (texto y enlaces).

pub mod canarios;
pub mod enlaces;
pub mod honeypot;
pub mod panico;
pub mod sin_volcados;
pub mod texto;

pub use canarios::GestorCanarios;
pub use honeypot::{HoneypotRam, TrampaMemoria};
pub use panico::{purgar_cadenas, salida_inmediata};
