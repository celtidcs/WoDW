//! Prueba del confinamiento del Worker. Confina **este** proceso de
//! prueba de forma irreversible, por eso vive sola en su propio binario.

/// Tras confinarse, el proceso no puede escribir en el temporal del usuario ni
/// lanzar procesos.
#[test]
fn proceso_confinado_no_escribe_disco_ni_lanza_procesos() {
    let ruta = std::env::temp_dir().join(format!("wodw_sandbox_{}.txt", std::process::id()));
    wodw::worker::sandbox::aplicar_sandbox_proceso_actual(u64::MAX)
        .expect("el confinamiento debe aplicarse");

    let escritura = std::fs::write(&ruta, b"escape");
    let _ = std::fs::remove_file(&ruta);
    assert!(
        escritura.is_err(),
        "el proceso confinado pudo escribir en {}",
        ruta.display()
    );

    let programa = if cfg!(windows) { "cmd" } else { "/bin/true" };
    let hijo = std::process::Command::new(programa).arg("/C").status();
    assert!(
        hijo.is_err(),
        "el proceso confinado pudo lanzar un proceso hijo"
    );

    // El Worker crea el runtime de Tokio después de confinarse: bloquear procesos
    // no puede bloquear también los hilos.
    let hilo = std::thread::Builder::new().spawn(|| 42).map(|h| h.join());
    assert!(
        matches!(hilo, Ok(Ok(42))),
        "el proceso confinado no pudo crear un hilo"
    );
}
