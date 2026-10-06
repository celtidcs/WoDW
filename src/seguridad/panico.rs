//! Purga amnésica de datos sensibles y salida inmediata.
//!
//! La purga sobrescribe con ceros (`zeroize`) el contenido de las cadenas antes
//! de vaciarlas, para que no queden copias legibles en la memoria liberada.

use zeroize::Zeroize;

/// Sobrescribe con ceros y vacía cada cadena.
pub fn purgar_cadenas<'a>(cadenas: impl IntoIterator<Item = &'a mut String>) {
    for cadena in cadenas {
        cadena.zeroize();
    }
}

/// Termina el proceso de inmediato sin ejecutar destructores ni vaciar búferes.
///
/// Los sub-Workers mueren con el Maestro: en Windows por el Job Object con
/// `KILL_ON_JOB_CLOSE`; en Linux por `PR_SET_PDEATHSIG`.
pub fn salida_inmediata() -> ! {
    std::process::abort()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purga_vacia_las_cadenas() {
        let mut a = "secreto".to_string();
        let mut b = "otro".to_string();
        purgar_cadenas([&mut a, &mut b]);
        assert!(a.is_empty() && b.is_empty());
    }
}
