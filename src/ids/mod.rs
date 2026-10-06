//! Módulo del Sistema de Detección de Intrusiones (IDS) en tiempo real.

pub mod eventos;
pub mod motor;

pub use eventos::{EventoDefensivo, NivelSeveridad, VectorAmenaza};
pub use motor::{ControlIds, EmisorIds, MotorIds, ResumenTelemetria};
