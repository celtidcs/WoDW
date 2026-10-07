//! Aviso de un error que impide arrancar (por ejemplo, un `wodw.toml`
//! inválido). En Windows el ejecutable no tiene consola, así que el aviso sale
//! en una ventana del sistema; en Linux, por la salida de errores.

use crate::ui::textos;

/// Muestra el error de arranque al usuario. Bloquea hasta que lo cierra (en
/// Windows). Solo para el proceso principal: el aislado no abre ventanas.
pub fn mostrar(detalle: &str) {
    let texto = textos::error_de_arranque(detalle);
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        let ancho = |t: &str| -> Vec<u16> {
            OsStr::new(t)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        };
        let (cuerpo, titulo) = (ancho(&texto), ancho(textos::TITULO_ERROR_ARRANQUE));
        // SAFETY: cadenas anchas terminadas en nulo que viven durante la llamada;
        // sin ventana propietaria.
        unsafe { MessageBoxW(0, cuerpo.as_ptr(), titulo.as_ptr(), MB_OK | MB_ICONERROR) };
    }
    #[cfg(not(windows))]
    eprintln!("{texto}");
}
