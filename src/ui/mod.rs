//! Interfaz gráfica nativa en `egui`/`eframe`, sin WebView ni JavaScript.

pub mod antifingerprint;
pub mod app;
pub mod contenido;
pub mod error_arranque;
pub mod estado;
pub mod fuentes;
pub mod icono;
pub mod motores;
pub mod panel_accesos;
pub mod panel_registro;
pub mod reproductor;
pub mod salida_audio;
pub mod telemetria;
pub mod textos;

pub use antifingerprint::{
    calcular_letterboxing, crear_opciones_nativas_seguras, MargenesLetterbox,
};
pub use app::{AccionSalida, VentanaPrincipal};
pub use estado::{EstadoContenido, EstadoNavegador, Pestana};
pub use motores::CatalogoMotores;
