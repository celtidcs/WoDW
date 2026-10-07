//! Terminación sin volcados de memoria (CA-D6b).
//!
//! Un volcado de memoria (en Windows, los que escribe el informe de errores en
//! `%LOCALAPPDATA%\CrashDumps`; en Linux, un *core*) contiene todo lo que el
//! proceso tenía en memoria: páginas, direcciones, medios. Comprobado en
//! Windows: `abort()` (que usaba el pánico) escribía un volcado de ≈41 MB y un
//! informe de errores, y tardaba unos 3,5 s. Aquí se cierran todas las vías:
//!
//! - [`terminar`]: salida inmediata sin informes (`TerminateProcess` / `_exit`).
//! - [`impedir_volcados`]: sin diálogos de error y con un filtro de excepciones
//!   no controladas que termina sin pasar por el informe de errores (Windows);
//!   proceso no volcable y sin *core* (Linux).
//! - [`AsignadorSinVolcado`]: si se agota la memoria, termina en vez de llamar
//!   a `abort()`.

use std::alloc::{GlobalAlloc, Layout, System};

/// Código de salida de una terminación inmediata.
pub const CODIGO_TERMINACION: u32 = 3;

/// Termina el proceso al instante, sin destructores, sin vaciar búferes y sin
/// informe de errores ni volcado.
pub fn terminar() -> ! {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
        // SAFETY: pseudodescriptor del proceso actual; la llamada no vuelve.
        unsafe { TerminateProcess(GetCurrentProcess(), CODIGO_TERMINACION) };
    }
    #[cfg(unix)]
    {
        // SAFETY: `_exit` termina sin ejecutar manejadores ni vaciar búferes.
        unsafe { libc::_exit(CODIGO_TERMINACION as libc::c_int) };
    }
    #[allow(unreachable_code)]
    loop {
        std::hint::spin_loop();
    }
}

/// Impide que un fallo del proceso deje un volcado de memoria.
pub fn impedir_volcados() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Diagnostics::Debug::{
            SetErrorMode, SetUnhandledExceptionFilter, EXCEPTION_POINTERS, SEM_FAILCRITICALERRORS,
            SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX,
        };
        /// Ante una excepción no controlada, termina sin informe ni volcado.
        unsafe extern "system" fn filtro(_info: *const EXCEPTION_POINTERS) -> i32 {
            terminar()
        }
        // SAFETY: solo cambian el modo de error y el filtro del proceso actual.
        unsafe {
            SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
            SetUnhandledExceptionFilter(Some(filtro));
        }
    }
    #[cfg(target_os = "linux")]
    {
        let sin_core = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: llamadas sin punteros salvo la estructura válida de `setrlimit`.
        unsafe {
            libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0);
            libc::setrlimit(libc::RLIMIT_CORE, &sin_core);
        }
    }
}

/// Asignador global que, si el sistema no puede dar memoria, termina el
/// proceso con [`terminar`] en lugar de dejar que la biblioteca estándar
/// llame a `abort()` (que en Windows escribe un volcado).
pub struct AsignadorSinVolcado;

// SAFETY: delega en `System`, que cumple el contrato de `GlobalAlloc`; solo
// cambia qué ocurre cuando `System` no puede asignar (nunca devuelve nulo).
unsafe impl GlobalAlloc for AsignadorSinVolcado {
    unsafe fn alloc(&self, disposicion: Layout) -> *mut u8 {
        // SAFETY: mismo contrato que el llamador.
        let p = unsafe { System.alloc(disposicion) };
        if p.is_null() {
            terminar();
        }
        p
    }

    unsafe fn alloc_zeroed(&self, disposicion: Layout) -> *mut u8 {
        // SAFETY: mismo contrato que el llamador.
        let p = unsafe { System.alloc_zeroed(disposicion) };
        if p.is_null() {
            terminar();
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, disposicion: Layout) {
        // SAFETY: `p` vino de este asignador con la misma disposición.
        unsafe { System.dealloc(p, disposicion) }
    }

    unsafe fn realloc(&self, p: *mut u8, disposicion: Layout, nuevo: usize) -> *mut u8 {
        // SAFETY: mismo contrato que el llamador.
        let q = unsafe { System.realloc(p, disposicion, nuevo) };
        if q.is_null() {
            terminar();
        }
        q
    }
}
