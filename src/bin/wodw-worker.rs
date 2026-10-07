//! WoDW — proceso aislado (sub-Worker).
//!
//! Ejecutable aparte del de la interfaz: no enlaza nada gráfico, así que no
//! carga `user32.dll` ni depende del escritorio desde el que se lance. El
//! Maestro (`wodw`) lo busca junto a sí mismo y lo lanza encerrado para cada
//! recurso. Por sí solo no hace nada útil: espera órdenes por su entrada
//! estándar.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::ExitCode;
use wodw::seguridad::sin_volcados::{impedir_volcados, AsignadorSinVolcado};
use wodw::worker;

/// Asignador que, si se agota la memoria, termina sin volcado.
#[global_allocator]
static ASIGNADOR: AsignadorSinVolcado = AsignadorSinVolcado;

fn main() -> ExitCode {
    impedir_volcados();
    let argumentos: Vec<String> = std::env::args().collect();
    match worker::ejecutar_proceso_worker(&argumentos) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Nadie la lee (la salida de errores no se conecta), pero queda
            // para quien lo ejecute a mano.
            eprintln!("WoDW (proceso aislado): {e}");
            ExitCode::FAILURE
        }
    }
}
