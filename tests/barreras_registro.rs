//! Barreras de CA-RS1–RS5: registro de la sesión con contenido (Seguridad o
//! Completo) y guardado (Manual o Automático) elegibles, explicados y con
//! confirmación al pasar a lo más revelador. Sin red.

use wodw::configuracion::{
    ConfiguracionRegistro, ConfiguracionWodw, ContenidoRegistro, GuardadoRegistro,
};
use wodw::ipc::mensajes::FamiliaMedio;
use wodw::maestro::{ContenidoPagina, EventoSesion, FalloNavegacion, ResultadoNavegacion};
use wodw::registro::{CambioRegistro, CategoriaRegistro, RegistroSesion};
use wodw::ui::textos::{explicacion_contenido_registro, explicacion_guardado_registro};
use wodw::ui::VentanaPrincipal;

fn registro(contenido: ContenidoRegistro, guardado: GuardadoRegistro) -> RegistroSesion {
    RegistroSesion::nuevo(&ConfiguracionRegistro {
        contenido,
        guardado,
        ..Default::default()
    })
}

/// CA-RS3: Seguridad no guarda navegación; Completo sí.
#[test]
fn seguridad_no_guarda_las_paginas_visitadas() {
    for (contenido, esperadas) in [
        (ContenidoRegistro::Seguridad, 1),
        (ContenidoRegistro::Completo, 2),
    ] {
        let mut r = registro(contenido, GuardadoRegistro::Manual);
        r.anotar(CategoriaRegistro::Navegacion, "http://secreto.onion/");
        r.anotar(CategoriaRegistro::Incidente, "sitio hostil bloqueado");
        assert_eq!(r.entradas().count(), esperadas, "{contenido:?}");
        let texto = r.texto();
        assert_eq!(
            texto.contains("secreto.onion"),
            contenido == ContenidoRegistro::Completo
        );
    }
}

/// CA-RS2: pasar a lo más revelador pide confirmación; volver a lo discreto no,
/// y al volver a Seguridad se borra lo que ya no corresponde.
#[test]
fn lo_mas_revelador_pide_confirmacion() {
    let mut r = registro(ContenidoRegistro::Seguridad, GuardadoRegistro::Manual);
    assert_eq!(
        r.solicitar_contenido(ContenidoRegistro::Completo),
        CambioRegistro::RequiereConfirmacion
    );
    assert_eq!(
        r.contenido(),
        ContenidoRegistro::Seguridad,
        "sin confirmar no cambia"
    );
    r.confirmar_contenido(ContenidoRegistro::Completo);
    r.anotar(CategoriaRegistro::Navegacion, "http://a.onion/");
    assert_eq!(
        r.solicitar_contenido(ContenidoRegistro::Seguridad),
        CambioRegistro::Aplicado
    );
    assert_eq!(
        r.entradas().count(),
        0,
        "la navegación se borra al volver a Seguridad"
    );
    assert_eq!(
        r.solicitar_guardado(GuardadoRegistro::Automatico),
        CambioRegistro::RequiereConfirmacion
    );
    assert_eq!(r.guardado(), GuardadoRegistro::Manual);
    r.confirmar_guardado(GuardadoRegistro::Automatico);
    assert_eq!(
        r.solicitar_guardado(GuardadoRegistro::Manual),
        CambioRegistro::Aplicado
    );
}

/// CA-RS2: cada opción tiene su explicación de utilidad y consecuencias.
#[test]
fn cada_opcion_se_explica() {
    let completo = explicacion_contenido_registro(ContenidoRegistro::Completo);
    assert!(completo.contains("direcciones"), "{completo}");
    assert!(!explicacion_contenido_registro(ContenidoRegistro::Seguridad).is_empty());
    let automatico = explicacion_guardado_registro(GuardadoRegistro::Automatico);
    assert!(automatico.contains("disco"), "{automatico}");
    assert!(automatico.contains("Tails"), "{automatico}");
    assert!(!explicacion_guardado_registro(GuardadoRegistro::Manual).is_empty());
}

/// Directorio temporal propio de una prueba.
fn temporal(nombre: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "wodw-prueba-registro-{nombre}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ventana(guardado: GuardadoRegistro, ruta: &std::path::Path) -> VentanaPrincipal {
    let mut cfg = ConfiguracionWodw::default();
    cfg.panico.abortar_proceso = false;
    cfg.registro.guardado = guardado;
    cfg.registro.ruta_automatica = Some(ruta.to_path_buf());
    VentanaPrincipal::nueva(cfg, None, Box::new(|| {}))
}

fn incidente(v: &mut VentanaPrincipal) {
    v.ir_a("http://malo.onion/");
    let p = v.estado().pestana_activa();
    let (id, solicitud) = (p.id(), p.solicitud());
    v.atender_evento(EventoSesion::Navegacion {
        id_pestana: id,
        solicitud,
        resultado: Err(FalloNavegacion {
            mensaje: "contenido hostil".to_string(),
            purgar_pestana: true,
        }),
    });
}

/// CA-RS4: Manual no escribe al cerrar; Automático sí.
#[test]
fn solo_el_automatico_escribe_al_cerrar() {
    for (guardado, escribe) in [
        (GuardadoRegistro::Manual, false),
        (GuardadoRegistro::Automatico, true),
    ] {
        let dir = temporal(&format!("{guardado:?}"));
        let ruta = dir.join("registro.txt");
        let mut v = ventana(guardado, &ruta);
        incidente(&mut v);
        v.al_cerrar();
        assert_eq!(ruta.exists(), escribe, "{guardado:?}");
        if escribe {
            let texto = std::fs::read_to_string(&ruta).unwrap();
            assert!(texto.contains("contenido hostil"), "{texto}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// CA-RS4: el pánico borra el registro y no escribe nada, ni en Automático.
#[test]
fn el_panico_borra_y_no_guarda() {
    let dir = temporal("panico");
    let ruta = dir.join("registro.txt");
    let mut v = ventana(GuardadoRegistro::Automatico, &ruta);
    incidente(&mut v);
    assert!(v.registro().entradas().count() > 0);
    v.activar_panico();
    assert_eq!(v.registro().entradas().count(), 0);
    assert!(!ruta.exists());
    let _ = std::fs::remove_dir_all(dir);
}

/// CA-RS3 en la ventana: en Seguridad no queda la dirección de una página
/// abierta con normalidad; el incidente sí queda.
#[test]
fn la_ventana_registra_segun_el_contenido_elegido() {
    let dir = temporal("ventana");
    let mut v = ventana(GuardadoRegistro::Manual, &dir.join("r.txt"));
    v.ir_a("http://secreto.onion/foto.mp3");
    let p = v.estado().pestana_activa();
    let (id, solicitud) = (p.id(), p.solicitud());
    v.atender_evento(EventoSesion::Navegacion {
        id_pestana: id,
        solicitud,
        resultado: Ok(ResultadoNavegacion {
            url: "http://secreto.onion/foto.mp3".to_string(),
            codigo_estado: 200,
            contenido: ContenidoPagina::Medio {
                familia: FamiliaMedio::Audio,
                tipo_mime: "audio/mpeg".to_string(),
                datos: vec![],
            },
        }),
    });
    incidente(&mut v);
    let texto = v.registro().texto();
    assert!(!texto.contains("secreto.onion"), "{texto}");
    assert!(texto.contains("contenido hostil"), "{texto}");
    let _ = std::fs::remove_dir_all(dir);
}
