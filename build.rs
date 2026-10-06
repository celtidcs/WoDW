//! Incrusta el icono de la aplicación en el ejecutable de Windows, para que el
//! Explorador y la barra de tareas lo muestren también con la app cerrada.

fn main() {
    println!("cargo:rerun-if-changed=recursos/icono.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        incrustar_icono_windows();
    }
}

/// Compila `recursos/icono.ico` como recurso del ejecutable. Un fallo detiene
/// la compilación: es preferible a publicar un ejecutable sin icono.
#[cfg(windows)]
fn incrustar_icono_windows() {
    let mut recurso = winresource::WindowsResource::new();
    recurso.set_icon("recursos/icono.ico");
    if let Err(e) = recurso.compile() {
        panic!("no se pudo incrustar el icono en el ejecutable: {e}");
    }
}

/// Compilando para Windows desde otro sistema no hay compilador de recursos.
#[cfg(not(windows))]
fn incrustar_icono_windows() {}
