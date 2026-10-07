//! Barrera de CA-21-5: en Windows, `wodw.exe` es un programa de ventana y no
//! abre una consola junto a la interfaz. Se lee el campo `Subsystem` de la
//! cabecera PE del binario que compila Cargo para las pruebas. Además, el error
//! de arranque (que ya no sale por una consola) se explica al usuario.

#[cfg(windows)]
/// Posición del desplazamiento de la cabecera PE dentro de la cabecera DOS.
const POSICION_E_LFANEW: usize = 0x3C;
#[cfg(windows)]
/// Firma `PE\0\0` más la cabecera COFF (20 bytes): inicio de la cabecera opcional.
const DESDE_PE_A_OPCIONAL: usize = 4 + 20;
#[cfg(windows)]
/// Posición de `Subsystem` en la cabecera opcional (igual en PE32 y PE32+).
const POSICION_SUBSISTEMA: usize = 68;
#[cfg(windows)]
/// `IMAGE_SUBSYSTEM_WINDOWS_GUI`.
const SUBSISTEMA_VENTANA: u16 = 2;
#[cfg(windows)]
/// `IMAGE_SUBSYSTEM_WINDOWS_CUI` (consola).
const SUBSISTEMA_CONSOLA: u16 = 3;

#[cfg(windows)]
/// Subsistema declarado por un ejecutable PE.
fn subsistema(bytes: &[u8]) -> u16 {
    let leer_u32 = |p: usize| u32::from_le_bytes(bytes[p..p + 4].try_into().expect("u32"));
    let pe = leer_u32(POSICION_E_LFANEW) as usize;
    assert_eq!(&bytes[pe..pe + 4], b"PE\0\0", "no es un ejecutable PE");
    let p = pe + DESDE_PE_A_OPCIONAL + POSICION_SUBSISTEMA;
    u16::from_le_bytes([bytes[p], bytes[p + 1]])
}

#[cfg(windows)]
#[test]
fn la_medicion_distingue_consola_de_ventana() {
    // Calibración: el intérprete de órdenes de Windows es de consola.
    let cmd = std::path::Path::new(&std::env::var("SystemRoot").expect("SystemRoot"))
        .join("System32")
        .join("cmd.exe");
    assert_eq!(
        subsistema(&std::fs::read(cmd).expect("cmd.exe")),
        SUBSISTEMA_CONSOLA
    );
}

#[cfg(windows)]
#[test]
fn wodw_es_un_programa_de_ventana() {
    let bytes = std::fs::read(env!("CARGO_BIN_EXE_wodw")).expect("ejecutable de wodw");
    assert_eq!(
        subsistema(&bytes),
        SUBSISTEMA_VENTANA,
        "wodw.exe abriría una consola junto a la ventana"
    );
}

/// El aviso de error de arranque conserva el detalle y orienta al usuario.
#[test]
fn el_error_de_arranque_se_explica() {
    let texto = wodw::ui::textos::error_de_arranque(
        "motores.lista.motivo: «X»: hay que explicar su fiabilidad",
    );
    assert!(texto.contains("motores.lista.motivo"), "{texto}");
    assert!(texto.contains("wodw.ejemplo.toml"), "{texto}");
}
