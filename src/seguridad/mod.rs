//! Tecnología de engaño, purga amnésica y mitigaciones reactivas.

pub mod canarios;
pub mod honeypot;
pub mod panico;

pub use canarios::GestorCanarios;
pub use honeypot::{HoneypotRam, TrampaMemoria};
pub use panico::{purgar_cadenas, salida_inmediata};
