//! Barreras de las direcciones onion v3 (CA-21-1): además del formato, la
//! suma de control de rend-spec-v3 §6 (SHA3-256 de «.onion checksum», clave
//! y versión). Una dirección mal copiada tiene buen aspecto pero no existe.

use wodw::configuracion::{es_direccion_onion_v3, ConfiguracionWodw};

/// Direcciones auténticas, publicadas por sus propios servicios.
const VALIDAS: [&str; 3] = [
    "juhanurmihxlp77nkq76byazcldy2hlmovfu2epvl5ankdibsot4csyd.onion", // Ahmia
    "duckduckgogg42xjoc72x3sjasowoarfbgcmvfimaftt6twagswzczad.onion", // DuckDuckGo
    "2gzyxa5ihm7nsggfxnu52rck2vv4rvmdlkiu3zzui5du4xyclen53wid.onion", // Tor Project
];

#[test]
fn direcciones_autenticas_se_aceptan() {
    for d in VALIDAS {
        assert!(es_direccion_onion_v3(d), "{d}");
        assert!(
            es_direccion_onion_v3(&format!("www.{d}")),
            "con subdominio: {d}"
        );
    }
}

#[test]
fn una_letra_cambiada_rompe_la_suma_de_control() {
    // La dirección de Torch que traían la 0.1.0 y la 0.2.0: formato correcto,
    // suma incorrecta (Arti también la rechaza).
    assert!(!es_direccion_onion_v3(
        "xmh57jrknzkhv6y3ls3ubitzfqnkrwxhopf5aygthi7d6rfdvgchu6ad.onion"
    ));
    // Ahmia con la última letra de la etiqueta cambiada.
    assert!(!es_direccion_onion_v3(
        "juhanurmihxlp77nkq76byazcldy2hlmovfu2epvl5ankdibsot4csye.onion"
    ));
}

#[test]
fn formato_incorrecto_se_rechaza() {
    for d in [
        "phobosxikamwc366g4v5e6w6e2q4j2o4v4z3e3x.onion",
        "JUHANURMIHXLP77NKQ76BYAZCLDY2HLMOVFU2EPVL5ANKDIBSOT4CSYD.onion",
        "juhanurmihxlp77nkq76byazcldy2hlmovfu2epvl5ankdibsot4csyd",
        "ejemplo.com",
        ".onion",
    ] {
        assert!(!es_direccion_onion_v3(d), "{d}");
    }
}

#[test]
fn la_configuracion_rechaza_un_motor_con_suma_incorrecta() {
    // Control: la misma configuración con una dirección auténtica es válida,
    // así que el rechazo solo puede deberse a la suma de control.
    let con = |onion: &str| {
        format!(
            "[motores]\npredeterminado = \"Prueba\"\n[[motores.lista]]\nnombre = \"Prueba\"\n\
             descripcion = \"x\"\nplantilla = \"http://{onion}/?q={{consulta}}\"\n"
        )
    };
    let autentica = con("juhanurmihxlp77nkq76byazcldy2hlmovfu2epvl5ankdibsot4csyd.onion");
    assert!(
        ConfiguracionWodw::desde_texto(&autentica).is_ok(),
        "{:?}",
        ConfiguracionWodw::desde_texto(&autentica).err()
    );
    let mala = con("xmh57jrknzkhv6y3ls3ubitzfqnkrwxhopf5aygthi7d6rfdvgchu6ad.onion");
    assert!(ConfiguracionWodw::desde_texto(&mala).is_err());
}

#[test]
fn todas_las_direcciones_por_defecto_superan_la_suma() {
    let cfg = ConfiguracionWodw::default();
    for motor in &cfg.motores.lista {
        let url = url::Url::parse(&motor.plantilla.replace("{consulta}", "x")).expect("plantilla");
        let host = url.host_str().unwrap_or_default();
        assert!(
            !host.ends_with(".onion") || es_direccion_onion_v3(host),
            "{}: {host}",
            motor.nombre
        );
    }
}
