//! Barrera de CA-D6b: la salida inmediata (la del pánico) termina en menos de
//! un segundo y no deja volcado de memoria. El propio binario de esta prueba
//! hace de hijo: con la variable [`VARIABLE`] ejecuta la salida pedida.

use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Variable que indica al hijo qué salida ejecutar.
const VARIABLE: &str = "WODW_PRUEBA_SALIDA";
/// Plazo máximo de la salida inmediata.
const PLAZO: Duration = Duration::from_secs(1);
/// Argumentos para que el arnés ejecute solo el hijo.
const ARGUMENTOS_HIJO: [&str; 3] = ["hijo_sale", "--exact", "--quiet"];

/// Hijo: no hace nada salvo que el padre lo active.
#[test]
fn hijo_sale() {
    match std::env::var(VARIABLE).as_deref() {
        Ok("inmediata") => wodw::seguridad::salida_inmediata(),
        Ok("abort") => std::process::abort(),
        _ => {}
    }
}

/// Carpeta donde Windows deja los volcados locales (si están activados).
fn carpeta_volcados() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("CrashDumps"))
}

/// Volcados que mencionan el PID `pid`.
fn volcados_de(pid: u32) -> Vec<PathBuf> {
    let Some(carpeta) = carpeta_volcados() else {
        return vec![];
    };
    std::fs::read_dir(carpeta)
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.to_string_lossy().ends_with(&format!(".{pid}.dmp")))
                .collect()
        })
        .unwrap_or_default()
}

/// Lanza el hijo con `modo` y devuelve (duración, código, volcados creados).
fn lanzar(modo: &str) -> (Duration, Option<i32>, Vec<PathBuf>) {
    let inicio = Instant::now();
    let mut hijo = std::process::Command::new(std::env::current_exe().unwrap())
        .args(ARGUMENTOS_HIJO)
        .env(VARIABLE, modo)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let pid = hijo.id();
    let estado = hijo.wait().unwrap();
    let duracion = inicio.elapsed();
    // Margen para que un informe de errores termine de escribir su volcado.
    std::thread::sleep(Duration::from_secs(2));
    (duracion, estado.code(), volcados_de(pid))
}

#[test]
fn la_salida_inmediata_es_rapida_y_sin_volcado() {
    let (duracion, codigo, volcados) = lanzar("inmediata");
    assert_eq!(codigo, Some(3), "código de la terminación inmediata");
    assert!(
        duracion < PLAZO + Duration::from_millis(500),
        "tardó {duracion:?}"
    );
    assert!(volcados.is_empty(), "dejó volcados: {volcados:?}");
}
