//! Barreras de los defectos hallados al verificar la
//! interfaz (CA-D1–D5, CA-D7). Sin red.

use wodw::configuracion::{ConfiguracionWodw, ConfiguracionWorker, ContenidoRegistro};
use wodw::ipc::mensajes::{OrdenWorker, RespuestaWorker};
use wodw::maestro::red::cliente::mensaje_conexion_fallida;
use wodw::ui::textos;
use wodw::ui::VentanaPrincipal;

/// CA-D1: las entidades con nombre habituales se traducen.
#[test]
fn entidades_html_con_nombre() {
    let r = wodw::worker::procesar_orden(
        OrdenWorker::ProcesarHtml {
            id_tarea: 1,
            url_origen: "http://a.onion/".to_string(),
            contenido_html: "<title>Ahmia &mdash; Búsqueda</title><p>a&ndash;b &hellip; &laquo;x&raquo; &copy; &euro; &aacute;&ntilde;&Uacute; &#233; &#xE9; &noexiste;</p>".as_bytes().to_vec(),
        },
        &ConfiguracionWorker::default(),
    );
    let Some(RespuestaWorker::HtmlProcesado {
        titulo,
        texto_limpio,
        ..
    }) = r
    else {
        panic!("se esperaba HtmlProcesado");
    };
    assert_eq!(titulo, "Ahmia — Búsqueda");
    assert_eq!(texto_limpio, "a–b … «x» © € áñÚ é é &noexiste;");
}

/// CA-D2: la viñeta del pánico no tiene espacios repetidos.
#[test]
fn vineta_del_panico_bien_formada() {
    let texto = textos::ayuda_panico(3);
    assert!(!texto.contains("  "), "{texto:?}");
    assert!(texto.lines().count() == 2, "{texto:?}");
}

/// CA-D3/D5: la confirmación tiene su propio aviso y hay nota sobre borrar.
#[test]
fn confirmacion_y_nota_del_registro() {
    for completo in [true, false] {
        let aviso = textos::aviso_confirmacion_registro(completo);
        assert!(aviso.starts_with("Atención"), "{aviso}");
    }
    assert_ne!(
        textos::aviso_confirmacion_registro(true),
        textos::explicacion_contenido_registro(ContenidoRegistro::Completo)
    );
    assert!(textos::NOTA_BORRAR_REGISTRO.contains("archivos"));
}

/// Ancho visible mínimo del campo de dirección al ancho mínimo de ventana.
const ANCHO_MINIMO_CAMPO: f32 = 150.0;

/// CA-D4: al ancho mínimo de la ventana, la barra no se solapa ni se sale y el
/// campo de dirección conserva un ancho útil (medido por su texto de ayuda).
#[test]
fn barra_usable_al_ancho_minimo() {
    let cfg = ConfiguracionWodw::default();
    let ancho = cfg.interfaz.ancho_minimo_ventana;
    let mut v = VentanaPrincipal::nueva(cfg, None, Box::new(|| {}));
    let ctx = egui::Context::default();
    let entrada = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(ancho, 600.0),
        )),
        ..Default::default()
    };
    for _ in 0..2 {
        ctx.run_ui(entrada(), |ui| v.dibujar(ui))
            .drop_without_applying_deltas();
    }
    let salida = ctx.run_ui(entrada(), |ui| v.dibujar(ui));
    let mut fila = Vec::new();
    let mut campo = None;
    for forma in &salida.shapes {
        if let egui::Shape::Text(t) = &forma.shape {
            let r = forma.shape.visual_bounding_rect();
            if r.min.y > 25.0 && r.max.y < 60.0 {
                if t.galley.text() == textos::PISTA_BARRA {
                    campo = Some(r);
                }
                fila.push(r);
            }
        }
    }
    salida.drop_without_applying_deltas();
    let campo = campo.expect("el texto de ayuda del campo de dirección se dibuja");
    assert!(
        campo.width() >= ANCHO_MINIMO_CAMPO,
        "campo de {} puntos",
        campo.width()
    );
    for (i, a) in fila.iter().enumerate() {
        assert!(a.max.x <= ancho, "se sale: {a:?}");
        for b in &fila[i + 1..] {
            assert!(!a.intersects(*b), "solape {a:?} {b:?}");
        }
    }
}

/// CA-D7: los errores conocidos de Arti se explican en español.
#[test]
fn errores_de_arti_en_espanol() {
    let m = mensaje_conexion_fallida(
        "x.onion",
        "tor: target address was invalid: Invalid onion address",
    );
    assert!(m.contains("la dirección .onion no es válida"), "{m}");
    assert!(
        m.contains("Invalid onion address"),
        "conserva el detalle: {m}"
    );
    let otro = mensaje_conexion_fallida("x.onion", "algo raro");
    assert!(otro.contains("error de la red Tor"), "{otro}");
}

/// N1: el área de contenido nunca es mayor que el espacio disponible (con el
/// panel IDS abierto y la ventana al mínimo quedan menos de 600 puntos y el
/// texto se dibujaba fuera, cortado).
#[test]
fn el_contenido_no_se_sale_del_espacio_disponible() {
    use wodw::ui::antifingerprint::calcular_letterboxing;
    let cfg = ConfiguracionWodw::default().interfaz;
    for ancho in [300.0, 480.0, 599.0, 600.0, 1100.0] {
        let disponible =
            egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(ancho, 350.0));
        let l = calcular_letterboxing(disponible, &cfg);
        assert!(
            disponible.contains_rect(l.area_contenido),
            "ancho {ancho}: {:?} fuera de {disponible:?}",
            l.area_contenido
        );
    }
}
