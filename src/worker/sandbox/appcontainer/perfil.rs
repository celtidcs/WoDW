//! Perfil del AppContainer y permiso de ejecución del ejecutable para su SID.

use super::{ancho, SidAppContainer, NOMBRE_APPCONTAINER};
use crate::error::{ErrorApp, Resultado};
use std::ffi::OsStr;
use std::path::Path;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{LocalFree, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, PSID};
use windows_sys::Win32::Security::Authorization::{
    GetNamedSecurityInfoW, SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W,
    GRANT_ACCESS, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN,
    TRUSTEE_W,
};
use windows_sys::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeleteAppContainerProfile,
};
use windows_sys::Win32::Security::{
    FreeSid, ACL, DACL_SECURITY_INFORMATION, NO_INHERITANCE, PSECURITY_DESCRIPTOR,
};
use windows_sys::Win32::Storage::FileSystem::{FILE_GENERIC_EXECUTE, FILE_GENERIC_READ};

/// Nombre visible y descripción del perfil.
const DESCRIPCION_PERFIL: &str = "WoDW: proceso aislado sin red que decodifica el contenido";
/// `HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS)`: el perfil ya existía.
const HRESULT_YA_EXISTE: i32 = (0x8007_0000_u32 | ERROR_ALREADY_EXISTS) as i32;

/// Crea el perfil del AppContainer si no existe y concede al AppContainer la
/// ejecución de `ejecutable`. Es idempotente.
///
/// # Errors
/// [`ErrorApp::Sandbox`] si el perfil no se puede crear o el permiso no se
/// puede conceder.
pub fn preparar(ejecutable: &Path) -> Resultado<()> {
    // Dos preparaciones simultáneas (varios procesadores en el mismo proceso)
    // escribirían a la vez el perfil y los permisos del mismo archivo.
    static EN_CURSO: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _turno = EN_CURSO
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    crear_perfil()?;
    conceder_ejecucion(ejecutable)
}

/// Borra el perfil del AppContainer (sin efecto si no existe).
pub fn borrar_perfil() {
    let nombre = ancho(OsStr::new(NOMBRE_APPCONTAINER));
    // SAFETY: cadena terminada en nulo.
    unsafe { DeleteAppContainerProfile(nombre.as_ptr()) };
}

/// Crea el perfil vacío del AppContainer; que ya exista no es un error.
fn crear_perfil() -> Resultado<()> {
    let nombre = ancho(OsStr::new(NOMBRE_APPCONTAINER));
    let descripcion = ancho(OsStr::new(DESCRIPCION_PERFIL));
    let mut sid: PSID = null_mut();
    // SAFETY: cadenas terminadas en nulo, sin capacidades, salida válida.
    let resultado = unsafe {
        CreateAppContainerProfile(
            nombre.as_ptr(),
            descripcion.as_ptr(),
            descripcion.as_ptr(),
            null(),
            0,
            &mut sid,
        )
    };
    if resultado >= 0 && !sid.is_null() {
        // SAFETY: SID devuelto por CreateAppContainerProfile; se libera con FreeSid.
        unsafe { FreeSid(sid) };
        return Ok(());
    }
    if resultado == HRESULT_YA_EXISTE {
        return Ok(());
    }
    Err(ErrorApp::Sandbox(format!(
        "no se pudo crear el perfil del AppContainer (HRESULT {resultado:#x})"
    )))
}

/// Concede lectura y ejecución del `ejecutable` al AppContainer de los Workers.
/// Es idempotente: una entrada ya presente se combina, no se duplica.
///
/// # Errors
/// [`ErrorApp::Sandbox`] si no se puede leer o escribir la lista de permisos
/// (por ejemplo, en un sistema de archivos sin permisos de Windows).
pub fn conceder_ejecucion(ejecutable: &Path) -> Resultado<()> {
    let sid = SidAppContainer::derivar()?;
    let ruta = ancho(ejecutable.as_os_str());
    let mut dacl_actual: *mut ACL = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: ruta terminada en nulo; punteros de salida válidos.
    let leido = unsafe {
        GetNamedSecurityInfoW(
            ruta.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut dacl_actual,
            null_mut(),
            &mut descriptor,
        )
    };
    if leido != ERROR_SUCCESS {
        return Err(ErrorApp::Sandbox(format!(
            "no se pudieron leer los permisos del ejecutable (código {leido})"
        )));
    }
    let resultado = combinar_y_escribir(&ruta, dacl_actual, &sid);
    // SAFETY: descriptor reservado por GetNamedSecurityInfoW; `dacl_actual`
    // apunta dentro de él y ya no se usa.
    unsafe { LocalFree(descriptor as _) };
    resultado
}

/// Añade la entrada del SID a `dacl_actual` y la escribe en `ruta`.
fn combinar_y_escribir(
    ruta: &[u16],
    dacl_actual: *mut ACL,
    sid: &SidAppContainer,
) -> Resultado<()> {
    let entrada = EXPLICIT_ACCESS_W {
        grfAccessPermissions: FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: NO_INHERITANCE,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: sid.0.cast(),
        },
    };
    let mut dacl_nueva: *mut ACL = null_mut();
    // SAFETY: una entrada válida; `dacl_actual` válida o nula; salida válida.
    let combinado = unsafe { SetEntriesInAclW(1, &entrada, dacl_actual, &mut dacl_nueva) };
    if combinado != ERROR_SUCCESS {
        return Err(ErrorApp::Sandbox(format!(
            "no se pudo componer el permiso del AppContainer (código {combinado})"
        )));
    }
    // SAFETY: ruta terminada en nulo y DACL recién compuesta.
    let escrito = unsafe {
        SetNamedSecurityInfoW(
            ruta.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            dacl_nueva,
            null(),
        )
    };
    // SAFETY: DACL reservada por SetEntriesInAclW.
    unsafe { LocalFree(dacl_nueva as _) };
    if escrito != ERROR_SUCCESS {
        return Err(ErrorApp::Sandbox(format!(
            "no se pudo conceder ejecución al AppContainer (código {escrito})"
        )));
    }
    Ok(())
}
