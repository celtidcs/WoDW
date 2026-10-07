//! WoDW - Waves on Dark Web.
//! Punto de entrada del ejecutable de escritorio para Windows y Linux.
//!
//! Es el Maestro (interfaz y red). Los recursos descargados los procesa el
//! ejecutable aparte `wodw-worker`, que debe estar junto a este.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// En Windows, programa de ventana: sin consola junto a la interfaz (CA-21-5).
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::process::ExitCode;
use wodw::configuracion::ConfiguracionWodw;
use wodw::maestro::iniciar_sesion;
use wodw::maestro::proceso_worker::localizar_worker;
use wodw::seguridad::salida_inmediata;
use wodw::seguridad::sin_volcados::{impedir_volcados, AsignadorSinVolcado};
use wodw::ui::{self, textos, VentanaPrincipal};

/// Asignador que, si se agota la memoria, termina sin volcado (ver
/// `seguridad::sin_volcados`).
#[global_allocator]
static ASIGNADOR: AsignadorSinVolcado = AsignadorSinVolcado;

fn main() -> ExitCode {
    impedir_volcados();
    let argumentos: Vec<String> = std::env::args().collect();
    let resultado = ejecutar_maestro(&argumentos);
    // Cierre normal: se borra el perfil vacío del AppContainer de los Workers.
    #[cfg(windows)]
    wodw::worker::sandbox::appcontainer::borrar_perfil();
    match resultado {
        Ok(()) => ExitCode::SUCCESS,
        Err(mensaje) => {
            ui::error_arranque::mostrar(&mensaje);
            ExitCode::FAILURE
        }
    }
}

/// Carga la configuración, arranca la sesión en segundo plano y abre la ventana.
fn ejecutar_maestro(argumentos: &[String]) -> Result<(), String> {
    tracing_subscriber::fmt::init();
    let cfg = ConfiguracionWodw::cargar(argumentos).map_err(|e| e.to_string())?;
    let maestro = std::env::current_exe().map_err(|e| e.to_string())?;
    let ejecutable = localizar_worker(&maestro).map_err(|e| e.to_string())?;
    let opciones = ui::crear_opciones_nativas_seguras(&cfg.interfaz, &textos::titulo_ventana());
    eframe::run_native(
        &textos::titulo_ventana(),
        opciones,
        Box::new(move |contexto| {
            ui::fuentes::instalar(&contexto.egui_ctx);
            let repintar = contexto.egui_ctx.clone();
            let sesion =
                iniciar_sesion(cfg.clone(), ejecutable, move || repintar.request_repaint())
                    .map_err(|e| tracing::error!(error = %e, "sesión de red no disponible"))
                    .ok();
            Ok(Box::new(VentanaPrincipal::nueva(
                cfg,
                sesion,
                Box::new(|| salida_inmediata()),
            )))
        }),
    )
    .map_err(|e| format!("no se pudo iniciar la interfaz gráfica: {e}"))
}
