//! CA-C6: en Windows, un proceso lanzado como sub-Worker (AppContainer sin
//! capacidades) no puede abrir conexiones de red.
//!
//! El propio binario de esta prueba hace de hijo: con la variable
//! [`VARIABLE_PUERTO`] presente, `hijo_intenta_conectar` intenta conectar al
//! puerto de escucha del padre y **falla la prueba si lo consigue**. Sin la
//! variable (o en el propio proceso padre) no hace nada. El padre comprueba el código de salida del hijo
//! dentro del AppContainer (debe ser 0: no conectó) y, como control, fuera de
//! él (debe ser distinto de 0: la misma sonda sí conecta). Sin red externa:
//! solo `127.0.0.1`.
#![cfg(windows)]

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;
use wodw::worker::sandbox::appcontainer;
use wodw::worker::sandbox::windows::JobObjectGuardian;

/// Variable de entorno con el puerto al que debe intentar conectar el hijo.
const VARIABLE_PUERTO: &str = "WODW_PRUEBA_PUERTO_RED";
/// Plazo de la conexión del hijo.
const PLAZO_CONEXION: Duration = Duration::from_secs(3);
/// Plazo para que el hijo termine (milisegundos).
const PLAZO_HIJO_MS: u32 = 60_000;
/// Argumentos para que el arnés de pruebas ejecute solo la sonda.
const ARGUMENTOS_SONDA: [&str; 4] = [
    "hijo_intenta_conectar",
    "--exact",
    "--test-threads=1",
    "--quiet",
];

/// Sonda del hijo: no hace nada salvo que el padre la active.
#[test]
fn hijo_intenta_conectar() {
    // Valor «puerto:pid del padre»; en el propio padre la sonda no actúa.
    let Some((puerto, padre)) = std::env::var(VARIABLE_PUERTO).ok().and_then(|v| {
        v.split_once(':')
            .map(|(a, b)| (a.to_string(), b.to_string()))
    }) else {
        return;
    };
    if padre == std::process::id().to_string() {
        return;
    }
    let destino: SocketAddr = format!("127.0.0.1:{puerto}").parse().unwrap();
    let conexion = TcpStream::connect_timeout(&destino, PLAZO_CONEXION);
    assert!(
        conexion.is_err(),
        "el proceso aislado abrió una conexión de red"
    );
}

#[test]
fn subworker_en_appcontainer_no_tiene_red() {
    let escucha = TcpListener::bind("127.0.0.1:0").unwrap();
    let puerto = format!(
        "{}:{}",
        escucha.local_addr().unwrap().port(),
        std::process::id()
    );
    // Única prueba que fija la variable; el resto de pruebas no la leen.
    std::env::set_var(VARIABLE_PUERTO, &puerto);
    let ejecutable = std::env::current_exe().unwrap();

    appcontainer::preparar(&ejecutable).unwrap();
    let job = JobObjectGuardian::nuevo(u64::MAX).unwrap();
    let hijo = appcontainer::lanzar(&ejecutable, &ARGUMENTOS_SONDA, &job).unwrap();
    let codigo_aislado = hijo.esperar(PLAZO_HIJO_MS);

    let control = std::process::Command::new(&ejecutable)
        .args(ARGUMENTOS_SONDA)
        .env(VARIABLE_PUERTO, &puerto)
        .output()
        .unwrap();
    std::env::remove_var(VARIABLE_PUERTO);

    assert!(
        !control.status.success(),
        "control: fuera del AppContainer la sonda debía conectar"
    );
    assert_eq!(
        codigo_aislado,
        Some(0),
        "dentro del AppContainer la sonda conectó o no terminó"
    );
}
