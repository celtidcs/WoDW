//! Subsistema de confinamiento multiplataforma del Worker.

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

use crate::error::Resultado;

/// Aplica el confinamiento sobre el proceso actual. Es irreversible.
///
/// # Errors
/// [`crate::error::ErrorApp::Sandbox`] si alguna restricción no puede
/// aplicarse, o si la plataforma no tiene confinamiento implementado
/// (el Worker nunca se ejecuta sin confinar).
pub fn aplicar_sandbox_proceso_actual() -> Resultado<()> {
    #[cfg(target_os = "windows")]
    {
        windows::aplicar_politicas_proceso_worker()
    }

    #[cfg(target_os = "linux")]
    {
        linux::aplicar_sandbox_worker_linux()
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Err(crate::error::ErrorApp::Sandbox(
            "plataforma sin confinamiento implementado".to_string(),
        ))
    }
}
