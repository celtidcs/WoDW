//! Icono de la aplicación para la ventana y la barra de tareas.
//!
//! El PNG va incrustado en el ejecutable: el binario sigue siendo un único
//! archivo portable. El icono del propio `.exe` en Windows lo incrusta `build.rs`
//! a partir de `recursos/icono.ico`.

/// Imagen del icono (512×512 px, RGBA).
const ICONO_PNG: &[u8] = include_bytes!("../../recursos/icono.png");

/// Icono de la aplicación ya decodificado, o `None` si no se pudo decodificar.
///
/// Un icono ilegible no debe impedir abrir la ventana: se registra el fallo y
/// la ventana se abre con el icono por defecto del sistema.
pub fn icono_aplicacion() -> Option<egui::IconData> {
    match image::load_from_memory_with_format(ICONO_PNG, image::ImageFormat::Png) {
        Ok(imagen) => {
            let rgba = imagen.into_rgba8();
            let (width, height) = rgba.dimensions();
            Some(egui::IconData {
                rgba: rgba.into_raw(),
                width,
                height,
            })
        }
        Err(e) => {
            tracing::warn!(error = %e, "no se pudo decodificar el icono de la aplicación");
            None
        }
    }
}
