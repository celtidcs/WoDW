//! Confinamiento del Worker en Windows.
//!
//! El Worker se restringe a sí mismo antes de recibir datos no confiables:
//! 1. Políticas de mitigación del proceso: sin código dinámico, sin procesos
//!    hijo, sin puntos de extensión heredados, comprobación estricta de
//!    descriptores, solo DLL firmadas por Microsoft y sin imágenes remotas o de
//!    integridad baja. `DisallowWin32kSystemCalls` no se aplica: Windows lo
//!    rechaza en tiempo de ejecución si `user32.dll` ya está cargada, y el
//!    ejecutable único la carga al enlazar la interfaz gráfica.
//! 2. Nivel de integridad **bajo** en su token: el sistema deniega la escritura
//!    en casi todo el disco y el registro del usuario (integridad media).
//!
//! Si cualquiera de los pasos falla, el Worker no procesa nada (falla cerrado).
//! Limitación declarada: un proceso de integridad baja conserva el acceso a la
//! red; bloquearlo exige AppContainer (ver `documentacion/defectos-conocidos.md`).
//!
//! El Maestro, por su parte, asigna el Worker a un Job Object
//! ([`JobObjectGuardian`]) que lo destruye si el Maestro termina.

use crate::error::{ErrorApp, Resultado};
use std::ffi::c_void;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE, PSID,
};
use windows_sys::Win32::Security::{
    AllocateAndInitializeSid, FreeSid, GetLengthSid, SetTokenInformation, TokenIntegrityLevel,
    SECURITY_MANDATORY_LABEL_AUTHORITY, SID_AND_ATTRIBUTES, TOKEN_ADJUST_DEFAULT,
    TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::SystemServices::{SECURITY_MANDATORY_LOW_RID, SE_GROUP_INTEGRITY};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcessToken, ProcessChildProcessPolicy, ProcessDynamicCodePolicy,
    ProcessExtensionPointDisablePolicy, ProcessImageLoadPolicy, ProcessSignaturePolicy,
    ProcessStrictHandleCheckPolicy, SetProcessMitigationPolicy, PROCESS_MITIGATION_POLICY,
};

/// Bit 0 de las estructuras `PROCESS_MITIGATION_*_POLICY` que lo activan.
const BIT_0: u32 = 1;
/// Bits 0 y 1 (`RaiseExceptionOnInvalidHandleReference` y
/// `HandleExceptionsPermanentlyEnabled`).
const BITS_0_Y_1: u32 = 0b11;

/// Políticas de mitigación aplicadas, con sus banderas y nombre para el registro.
const POLITICAS_MITIGACION: &[(PROCESS_MITIGATION_POLICY, u32, &str)] = &[
    (ProcessDynamicCodePolicy, BIT_0, "ProhibitDynamicCode"),
    (ProcessChildProcessPolicy, BIT_0, "NoChildProcessCreation"),
    (
        ProcessExtensionPointDisablePolicy,
        BIT_0,
        "DisableExtensionPoints",
    ),
    (
        ProcessStrictHandleCheckPolicy,
        BITS_0_Y_1,
        "StrictHandleCheck",
    ),
    (ProcessSignaturePolicy, BIT_0, "MicrosoftSignedOnly"),
    (
        ProcessImageLoadPolicy,
        BITS_0_Y_1,
        "NoRemoteImages/NoLowMandatoryLabelImages",
    ),
];

/// Aplica todas las restricciones del Worker en Windows.
///
/// # Errors
/// [`ErrorApp::Sandbox`] con el código de Windows si algún paso falla.
pub fn aplicar_politicas_proceso_worker() -> Resultado<()> {
    for &(politica, banderas, nombre) in POLITICAS_MITIGACION {
        aplicar_politica(politica, banderas, nombre)?;
    }
    bajar_integridad_a_baja()
}

/// Aplica una política de mitigación cuyo descriptor es un `DWORD` de banderas.
fn aplicar_politica(
    politica: PROCESS_MITIGATION_POLICY,
    banderas: u32,
    nombre: &str,
) -> Resultado<()> {
    // SAFETY: todas las estructuras PROCESS_MITIGATION_*_POLICY usadas son una
    // unión de un único DWORD; se pasa su dirección y su tamaño exacto.
    let resultado = unsafe {
        SetProcessMitigationPolicy(
            politica,
            (&banderas as *const u32).cast::<c_void>(),
            std::mem::size_of::<u32>(),
        )
    };
    if resultado == 0 {
        return Err(error_windows(&format!("política {nombre}")));
    }
    Ok(())
}

/// Rebaja el token del proceso actual al nivel de integridad bajo (S-1-16-4096).
fn bajar_integridad_a_baja() -> Resultado<()> {
    let mut token: HANDLE = 0;
    // SAFETY: GetCurrentProcess devuelve un pseudodescriptor válido; `token` es
    // un puntero de salida válido.
    if unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_DEFAULT | TOKEN_QUERY,
            &mut token,
        )
    } == 0
    {
        return Err(error_windows("OpenProcessToken"));
    }
    let resultado = establecer_integridad_baja(token);
    // SAFETY: `token` se abrió arriba y no se usa después.
    unsafe { CloseHandle(token) };
    resultado
}

/// Crea el SID de integridad baja y lo fija en `token`.
fn establecer_integridad_baja(token: HANDLE) -> Resultado<()> {
    let mut sid: PSID = null_mut();
    // SAFETY: autoridad y puntero de salida válidos; una subautoridad.
    let creado = unsafe {
        AllocateAndInitializeSid(
            &SECURITY_MANDATORY_LABEL_AUTHORITY,
            1,
            SECURITY_MANDATORY_LOW_RID as u32,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut sid,
        )
    };
    if creado == 0 {
        return Err(error_windows("AllocateAndInitializeSid"));
    }
    let etiqueta = TOKEN_MANDATORY_LABEL {
        Label: SID_AND_ATTRIBUTES {
            Sid: sid,
            Attributes: SE_GROUP_INTEGRITY as u32,
        },
    };
    // SAFETY: `sid` es válido hasta FreeSid; la longitud incluye el SID.
    let fijado = unsafe {
        let longitud = std::mem::size_of::<TOKEN_MANDATORY_LABEL>() as u32 + GetLengthSid(sid);
        SetTokenInformation(
            token,
            TokenIntegrityLevel,
            (&etiqueta as *const TOKEN_MANDATORY_LABEL).cast::<c_void>(),
            longitud,
        )
    };
    let resultado = if fijado == 0 {
        Err(error_windows("SetTokenInformation(TokenIntegrityLevel)"))
    } else {
        Ok(())
    };
    // SAFETY: `sid` se reservó con AllocateAndInitializeSid.
    unsafe { FreeSid(sid) };
    resultado
}

/// Error de sandbox con el último código de error de Windows.
fn error_windows(operacion: &str) -> ErrorApp {
    // SAFETY: GetLastError no tiene precondiciones.
    let codigo = unsafe { GetLastError() };
    ErrorApp::Sandbox(format!("{operacion} falló (código de Windows {codigo})"))
}

/// Job Object que destruye los procesos asignados cuando se cierra su descriptor
/// (por ejemplo, si el Maestro termina abruptamente) e impide procesos hijo.
#[derive(Debug)]
pub struct JobObjectGuardian {
    manejador: HANDLE,
}

// SAFETY: un descriptor de Job Object es un identificador del núcleo usable
// desde cualquier hilo; la estructura no expone mutación interna.
unsafe impl Send for JobObjectGuardian {}
// SAFETY: ver arriba.
unsafe impl Sync for JobObjectGuardian {}

impl JobObjectGuardian {
    /// Crea el Job Object con `KILL_ON_JOB_CLOSE` y un único proceso activo.
    ///
    /// # Errors
    /// [`ErrorApp::Sandbox`] si Windows rechaza la creación o la configuración.
    pub fn nuevo() -> Resultado<Self> {
        // SAFETY: atributos y nombre nulos son válidos.
        let manejador = unsafe { CreateJobObjectW(null_mut(), null_mut()) };
        if manejador == 0 || manejador == INVALID_HANDLE_VALUE {
            return Err(error_windows("CreateJobObjectW"));
        }
        let guardian = Self { manejador };
        // SAFETY: estructura POD cuyo valor cero es válido.
        let mut limites: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limites.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        limites.BasicLimitInformation.ActiveProcessLimit = 1;
        // SAFETY: puntero y tamaño exactos de la estructura.
        let configurado = unsafe {
            SetInformationJobObject(
                guardian.manejador,
                JobObjectExtendedLimitInformation,
                (&limites as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast::<c_void>(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configurado == 0 {
            return Err(error_windows("SetInformationJobObject"));
        }
        Ok(guardian)
    }

    /// Asigna el proceso `proceso` (descriptor con `PROCESS_SET_QUOTA | PROCESS_TERMINATE`).
    ///
    /// # Errors
    /// [`ErrorApp::Sandbox`] si la asignación falla.
    pub fn asignar_proceso(&self, proceso: HANDLE) -> Resultado<()> {
        // SAFETY: ambos descriptores son válidos mientras dure la llamada.
        if unsafe { AssignProcessToJobObject(self.manejador, proceso) } == 0 {
            return Err(error_windows("AssignProcessToJobObject"));
        }
        Ok(())
    }
}

impl Drop for JobObjectGuardian {
    fn drop(&mut self) {
        // SAFETY: descriptor propio, cerrado una sola vez.
        unsafe { CloseHandle(self.manejador) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_object_se_crea() {
        assert!(JobObjectGuardian::nuevo().is_ok());
    }
}
