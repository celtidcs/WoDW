//! Incrusta el icono de la aplicación en el ejecutable de Windows, para que el
//! Explorador y la barra de tareas lo muestren también con la app cerrada.

use std::error::Error;

/// Un error del script de compilación detiene la compilación con su mensaje.
type ResultadoCompilacion = Result<(), Box<dyn Error>>;

fn main() -> ResultadoCompilacion {
    println!("cargo:rerun-if-changed=recursos/icono.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        incrustar_icono_windows()?;
    }
    Ok(())
}

/// Compila `recursos/icono.ico` como recurso del ejecutable. Un fallo detiene
/// la compilación: es preferible a publicar un ejecutable sin icono.
#[cfg(windows)]
fn incrustar_icono_windows() -> ResultadoCompilacion {
    let mut recurso = winresource::WindowsResource::new();
    recurso.set_icon("recursos/icono.ico");
    recurso
        .compile()
        .map_err(|e| format!("no se pudo incrustar el icono en el ejecutable: {e}").into())
}

/// Compilando para Windows desde otro sistema no hay compilador de recursos.
#[cfg(not(windows))]
fn incrustar_icono_windows() -> ResultadoCompilacion {
    Ok(())
}
