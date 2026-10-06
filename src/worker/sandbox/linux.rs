//! Confinamiento del Worker en Linux / Tails OS.
//!
//! Se aplica antes de crear el runtime asíncrono, de modo que todos los hilos
//! posteriores heredan las restricciones:
//! 1. `prctl`: sin volcados ni `ptrace` (`PR_SET_DUMPABLE=0`), sin elevación
//!    (`PR_SET_NO_NEW_PRIVS`) y muerte al morir el Maestro (`PR_SET_PDEATHSIG`).
//! 2. Landlock: deniega todo acceso al sistema de archivos y, con ABI ≥ 4,
//!    `bind`/`connect` TCP. Los descriptores ya abiertos (stdin/stdout) siguen
//!    funcionando.
//! 3. seccomp-BPF: mata el proceso (SIGSYS) si intenta crear sockets, ejecutar
//!    binarios, depurar o leer memoria de otros procesos. La única excepción es
//!    `socketpair(AF_UNIX)`, que Tokio usa al crear el runtime: un par local sin
//!    nombre no alcanza ni la red ni el disco.
//! 4. seccomp-BPF: la creación de procesos (`fork`, `vfork`, `clone` sin
//!    `CLONE_THREAD`, `clone3`) falla con `ENOSYS`. Un hijo no hereda
//!    `PR_SET_PDEATHSIG` y sobreviviría a la autodestrucción del Worker; los
//!    hilos (`clone` con `CLONE_THREAD`) siguen permitidos.

use crate::error::{ErrorApp, Resultado};
use landlock::{Access, AccessFs, AccessNet, Ruleset, RulesetAttr, RulesetStatus, ABI};
use seccompiler::{
    apply_filter_all_threads, BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp,
    SeccompCondition, SeccompFilter, SeccompRule, TargetArch,
};
use std::collections::BTreeMap;

/// ABI de Landlock solicitada; en núcleos anteriores se aplica lo disponible.
const ABI_LANDLOCK: ABI = ABI::V5;

/// Llamadas al sistema prohibidas al Worker. Cualquiera de ellas indica un
/// intento de escape: el procesamiento de contenido no necesita ninguna.
const LLAMADAS_PROHIBIDAS: &[libc::c_long] = &[
    libc::SYS_socket,
    libc::SYS_connect,
    libc::SYS_bind,
    libc::SYS_listen,
    libc::SYS_accept,
    libc::SYS_accept4,
    libc::SYS_execve,
    libc::SYS_execveat,
    libc::SYS_ptrace,
    libc::SYS_process_vm_readv,
    libc::SYS_process_vm_writev,
];

/// Error devuelto a toda llamada que crearía un proceso. Es `ENOSYS` y no
/// `EPERM` porque `clone3` pasa sus banderas en memoria, donde BPF no puede
/// leerlas: se rechaza entero y glibc, solo ante `ENOSYS`, reintenta con
/// `clone`, cuyas banderas sí se filtran. Así los hilos siguen funcionando.
const ERRNO_CREAR_PROCESO: u32 = libc::ENOSYS as u32;

/// Aplica todas las restricciones del Worker en Linux.
///
/// # Errors
/// [`ErrorApp::Sandbox`] si alguna restricción no puede aplicarse o si
/// Landlock no está disponible en el núcleo (falla cerrado).
pub fn aplicar_sandbox_worker_linux() -> Resultado<()> {
    aplicar_prctl()?;
    aplicar_landlock()?;
    aplicar_seccomp()?;
    impedir_crear_procesos()
}

/// Restricciones básicas con `prctl`.
fn aplicar_prctl() -> Resultado<()> {
    let opciones: [(libc::c_int, libc::c_ulong, &str); 3] = [
        (libc::PR_SET_DUMPABLE, 0, "PR_SET_DUMPABLE"),
        (libc::PR_SET_NO_NEW_PRIVS, 1, "PR_SET_NO_NEW_PRIVS"),
        (
            libc::PR_SET_PDEATHSIG,
            libc::SIGKILL as libc::c_ulong,
            "PR_SET_PDEATHSIG",
        ),
    ];
    for (opcion, valor, nombre) in opciones {
        // SAFETY: prctl con estas opciones solo lee argumentos enteros.
        if unsafe { libc::prctl(opcion, valor, 0, 0, 0) } != 0 {
            return Err(ErrorApp::Sandbox(format!("fallo al aplicar {nombre}")));
        }
    }
    Ok(())
}

/// Deniega sistema de archivos y red TCP con Landlock.
fn aplicar_landlock() -> Resultado<()> {
    let error = |e: landlock::RulesetError| ErrorApp::Sandbox(format!("Landlock: {e}"));
    let estado = Ruleset::default()
        .handle_access(AccessFs::from_all(ABI_LANDLOCK))
        .map_err(error)?
        .handle_access(AccessNet::from_all(ABI_LANDLOCK))
        .map_err(error)?
        .create()
        .map_err(error)?
        .restrict_self()
        .map_err(error)?;
    if estado.ruleset == RulesetStatus::NotEnforced {
        return Err(ErrorApp::Sandbox(
            "el núcleo no aplica Landlock; el Worker se niega a procesar contenido".to_string(),
        ));
    }
    Ok(())
}

/// Mata el proceso ante cualquier [`LLAMADAS_PROHIBIDAS`].
fn aplicar_seccomp() -> Resultado<()> {
    let socketpair_no_local = SeccompCondition::new(
        0,
        SeccompCmpArgLen::Dword,
        SeccompCmpOp::Ne,
        libc::AF_UNIX as u64,
    )
    .and_then(|condicion| SeccompRule::new(vec![condicion]))
    .map_err(|e| ErrorApp::Sandbox(format!("regla seccomp de socketpair inválida: {e}")))?;
    let mut reglas: BTreeMap<_, _> = LLAMADAS_PROHIBIDAS
        .iter()
        .map(|&llamada| (llamada, Vec::new()))
        .collect();
    reglas.insert(libc::SYS_socketpair, vec![socketpair_no_local]);
    instalar_filtro(reglas, SeccompAction::KillProcess)
}

/// Hace fallar toda creación de procesos sin impedir la de hilos.
fn impedir_crear_procesos() -> Resultado<()> {
    let sin_clone_thread = SeccompCondition::new(
        0,
        SeccompCmpArgLen::Qword,
        SeccompCmpOp::MaskedEq(libc::CLONE_THREAD as u64),
        0,
    )
    .and_then(|condicion| SeccompRule::new(vec![condicion]))
    .map_err(|e| ErrorApp::Sandbox(format!("regla seccomp de clone inválida: {e}")))?;
    let mut reglas = BTreeMap::from([
        (libc::SYS_clone, vec![sin_clone_thread]),
        (libc::SYS_clone3, Vec::new()),
    ]);
    #[cfg(target_arch = "x86_64")]
    reglas.extend([(libc::SYS_fork, Vec::new()), (libc::SYS_vfork, Vec::new())]);
    instalar_filtro(reglas, SeccompAction::Errno(ERRNO_CREAR_PROCESO))
}

/// Instala en todos los hilos un filtro que aplica `accion` a las llamadas de
/// `reglas` y permite el resto. Los filtros se apilan: el núcleo aplica la
/// acción más restrictiva de todos ellos.
fn instalar_filtro(
    reglas: BTreeMap<i64, Vec<SeccompRule>>,
    accion: SeccompAction,
) -> Resultado<()> {
    let arquitectura = TargetArch::try_from(std::env::consts::ARCH)
        .map_err(|e| ErrorApp::Sandbox(format!("arquitectura no soportada por seccomp: {e}")))?;
    let filtro = SeccompFilter::new(reglas, SeccompAction::Allow, accion, arquitectura)
        .map_err(|e| ErrorApp::Sandbox(format!("filtro seccomp inválido: {e}")))?;
    let programa = BpfProgram::try_from(filtro)
        .map_err(|e| ErrorApp::Sandbox(format!("compilación BPF fallida: {e}")))?;
    apply_filter_all_threads(&programa)
        .map_err(|e| ErrorApp::Sandbox(format!("no se pudo instalar seccomp: {e}")))
}
