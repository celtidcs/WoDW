//! WoDW - Waves on Dark Web.
//! Punto de entrada del ejecutable de escritorio para Windows y Linux.
//!
//! El mismo binario actúa como Maestro (interfaz + red) o, con
//! `--modo-worker`, como sub-Worker confinado.

use std::process::ExitCode;
use wodw::configuracion::ConfiguracionWodw;
use wodw::maestro::iniciar_sesion;
use wodw::seguridad::salida_inmediata;
use wodw::ui::{self, textos, VentanaPrincipal};
use wodw::worker::{self, ARGUMENTO_MODO_WORKER};

fn main() -> ExitCode {
    let argumentos: Vec<String> = std::env::args().collect();
    let resultado = if argumentos.iter().any(|a| a == ARGUMENTO_MODO_WORKER) {
        worker::ejecutar_proceso_worker(&argumentos).map_err(|e| e.to_string())
    } else {
        ejecutar_maestro(&argumentos)
    };
    match resultado {
        Ok(()) => ExitCode::SUCCESS,
        Err(mensaje) => {
            eprintln!("WoDW: {mensaje}");
            ExitCode::FAILURE
        }
    }
}

/// Carga la configuración, arranca la sesión en segundo plano y abre la ventana.
fn ejecutar_maestro(argumentos: &[String]) -> Result<(), String> {
    tracing_subscriber::fmt::init();
    let cfg = ConfiguracionWodw::cargar(argumentos).map_err(|e| e.to_string())?;
    let ejecutable = std::env::current_exe().map_err(|e| e.to_string())?;
    let opciones = ui::crear_opciones_nativas_seguras(&cfg.interfaz, &textos::titulo_ventana());
    eframe::run_native(
        &textos::titulo_ventana(),
        opciones,
        Box::new(move |contexto| {
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
