//! Barrera del parche temporal de `saturating-time` (fallo de Arti en Windows, Arti #2678 y
//! #2726). Sin el parche, en Windows estas operaciones no terminan nunca y Tor no arranca.
//! En Linux el fallo no existe y la barrera pasa también sin el parche.

use saturating_time::SaturatingTime;
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

/// Plazo holgado: con el parche cada operación tarda microsegundos.
const PLAZO_MAXIMO: Duration = Duration::from_secs(5);

/// Ejecuta `operacion` en otro hilo y dice si terminó dentro del plazo. Un bucle infinito no se
/// puede interrumpir, así que el hilo colgado se abandona y la prueba falla en vez de colgarse.
fn termina_a_tiempo(operacion: fn() -> SystemTime) -> bool {
    let (emisor, receptor) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = emisor.send(operacion());
    });
    receptor.recv_timeout(PLAZO_MAXIMO).is_ok()
}

#[test]
fn restar_tiempo_termina() {
    assert!(
        termina_a_tiempo(|| SystemTime::now().saturating_sub(Duration::from_secs(1))),
        "saturating_sub no termina: falta el parche de parches/saturating-time"
    );
}

#[test]
fn sumar_tiempo_termina() {
    assert!(
        termina_a_tiempo(|| SystemTime::now().saturating_add(Duration::from_secs(1))),
        "saturating_add no termina: falta el parche de parches/saturating-time"
    );
}
