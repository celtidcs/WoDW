//! Barreras de CA-VN1: la respuesta de GitHub sobre la última versión se trata
//! como no confiable. Solo se acepta un número de versión limpio, el enlace se
//! construye a partir de la configuración (nunca del que trae la respuesta) y
//! solo se avisa si la versión es posterior a la instalada. Sin red.

use wodw::configuracion::ConfiguracionActualizaciones;
use wodw::maestro::versiones::{interpretar_ultima_version, VersionNueva};

fn json(etiqueta: &str, enlace: &str) -> Vec<u8> {
    format!(r#"{{"tag_name":"{etiqueta}","html_url":"{enlace}","name":"x","assets":[]}}"#)
        .into_bytes()
}

#[test]
fn version_posterior_se_avisa_con_enlace_propio() {
    let cfg = ConfiguracionActualizaciones::default();
    let r = interpretar_ultima_version(
        &json("v0.2.0", "https://malo.example/trampa"),
        "0.1.0",
        &cfg,
    )
    .unwrap();
    assert_eq!(
        r,
        Some(VersionNueva {
            version: "0.2.0".to_string(),
            enlace: format!("{}v0.2.0", cfg.url_publicaciones),
        })
    );
}

#[test]
fn misma_version_o_anterior_no_avisa() {
    let cfg = ConfiguracionActualizaciones::default();
    for etiqueta in ["v0.1.0", "0.1.0", "v0.0.9"] {
        assert_eq!(
            interpretar_ultima_version(&json(etiqueta, ""), "0.1.0", &cfg).unwrap(),
            None,
            "{etiqueta}"
        );
    }
    assert!(
        interpretar_ultima_version(&json("v0.10.0", ""), "0.9.0", &cfg)
            .unwrap()
            .is_some(),
        "comparación numérica"
    );
}

#[test]
fn etiquetas_o_respuestas_extranas_se_rechazan() {
    let cfg = ConfiguracionActualizaciones::default();
    for etiqueta in [
        "v1.0",
        "v+1.0.0",
        "v1.+2.0",
        "v1.0.0-rc1",
        "v1.0.0; rm -rf /",
        "<script>",
        "v99999999999999999999.0.0",
        "",
    ] {
        assert!(
            interpretar_ultima_version(&json(etiqueta, ""), "0.1.0", &cfg).is_err(),
            "{etiqueta}"
        );
    }
    assert!(interpretar_ultima_version(b"no es json", "0.1.0", &cfg).is_err());
    assert!(interpretar_ultima_version(b"{}", "0.1.0", &cfg).is_err());
}
