//! Lanzamiento del sub-Worker dentro de un **AppContainer** sin capacidades.
//!
//! Un AppContainer es el aislamiento de Windows de las aplicaciones de la
//! tienda: sin la capacidad `internetClient` (ni ninguna otra) el proceso no
//! puede abrir conexiones de red, ni siquiera a `127.0.0.1`, y solo accede a
//! los objetos que concedan permiso explícito a su SID. Así, un decodificador
//! engañado por un archivo malicioso no puede saltarse Tor para revelar la IP.
//!
//! Windows solo arranca un proceso en un AppContainer cuyo perfil exista (sin
//! él, `CreateProcessW` falla con el código 2). [`preparar`] crea el perfil
//! —vacío: un nombre en el registro del usuario y una carpeta sin contenido—
//! si no existe, y concede al SID lectura y ejecución del ejecutable;
//! [`borrar_perfil`] lo elimina al cerrar la aplicación con normalidad. Si la
//! aplicación se cierra por el pánico, el perfil vacío queda y se reutiliza.
//!
//! El proceso se crea suspendido, se asigna al Job Object y solo entonces se
//! reanuda: no ejecuta ni una instrucción fuera del Job.
//!
//! Piezas: `perfil` (perfil y permiso del ejecutable), `lanzamiento`
//! (creación del proceso) y `linea_ordenes` (citado de los argumentos).

mod lanzamiento;
mod linea_ordenes;
mod perfil;

pub use lanzamiento::{lanzar, ProcesoAppContainer};
pub use linea_ordenes::linea_de_ordenes;
pub use perfil::{borrar_perfil, conceder_ejecucion, preparar};

use crate::error::{ErrorApp, Resultado};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, PSID};
use windows_sys::Win32::Security::FreeSid;
use windows_sys::Win32::Security::Isolation::DeriveAppContainerSidFromAppContainerName;

/// Nombre del AppContainer de los sub-Workers (de él se deriva su SID).
pub const NOMBRE_APPCONTAINER: &str = "WoDW.Worker";

/// SID de AppContainer liberado al soltarse.
struct SidAppContainer(PSID);

impl SidAppContainer {
    /// Deriva el SID del nombre fijo, sin crear perfil.
    fn derivar() -> Resultado<Self> {
        let nombre = ancho(OsStr::new(NOMBRE_APPCONTAINER));
        let mut sid: PSID = null_mut();
        // SAFETY: cadena terminada en nulo y puntero de salida válido.
        let resultado =
            unsafe { DeriveAppContainerSidFromAppContainerName(nombre.as_ptr(), &mut sid) };
        if resultado < 0 || sid.is_null() {
            return Err(ErrorApp::Sandbox(format!(
                "DeriveAppContainerSidFromAppContainerName falló (HRESULT {resultado:#x})"
            )));
        }
        Ok(Self(sid))
    }
}

impl Drop for SidAppContainer {
    fn drop(&mut self) {
        // SAFETY: SID devuelto por DeriveAppContainerSidFromAppContainerName,
        // que se libera con FreeSid según su documentación.
        unsafe { FreeSid(self.0) };
    }
}

/// Descriptor de Windows cerrado al soltarse.
struct Descriptor(HANDLE);

impl Descriptor {
    /// Cede el descriptor sin cerrarlo.
    fn ceder(self) -> HANDLE {
        let h = self.0;
        std::mem::forget(self);
        h
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        if self.0 != 0 {
            // SAFETY: descriptor propio, cerrado una sola vez.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Cadena ancha terminada en nulo.
fn ancho(texto: &OsStr) -> Vec<u16> {
    texto.encode_wide().chain(std::iter::once(0)).collect()
}

/// Error con el último código de Windows.
fn error_windows(operacion: &str) -> ErrorApp {
    // SAFETY: GetLastError no tiene precondiciones.
    let codigo = unsafe { GetLastError() };
    ErrorApp::Sandbox(format!("{operacion} falló (código de Windows {codigo})"))
}
