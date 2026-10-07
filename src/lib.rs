//! WoDW - Waves on Dark Web
//!
//! Biblioteca central para navegación y búsqueda de solo lectura sobre Tor con
//! cliente Arti embebido, procesamiento de contenido en sub-Workers confinados,
//! tecnología de engaño, IDS con respuesta automática y visor nativo «Safest».

#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

pub mod audit_checks;
pub mod configuracion;
pub mod error;
pub mod ids;
pub mod ipc;
pub mod maestro;
pub mod registro;
pub mod seguridad;
pub mod ui;
pub mod unidades;
pub mod worker;
