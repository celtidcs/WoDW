//! Comportamiento de la ventana sin red: pánico, purgas automáticas, botones y viñetas.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use wodw::configuracion::ConfiguracionWodw;
use wodw::maestro::{EventoSesion, FalloNavegacion};
use wodw::ui::{EstadoContenido, VentanaPrincipal};

/// Ventana sin sesión con una acción de salida que cuenta invocaciones.
fn ventana(cfg: ConfiguracionWodw) -> (VentanaPrincipal, Rc<Cell<u32>>) {
    let salidas = Rc::new(Cell::new(0));
    let contador = salidas.clone();
    let v = VentanaPrincipal::nueva(
        cfg,
        None,
        Box::new(move || contador.set(contador.get() + 1)),
    );
    (v, salidas)
}

fn con_rastro(v: &mut VentanaPrincipal) {
    v.ir_a("http://secreto.onion/");
    v.ir_a("consulta comprometedora");
}

fn sin_rastro(v: &VentanaPrincipal) -> bool {
    let e = v.estado();
    let p = e.pestana_activa();
    e.pestanas().len() == 1
        && p.url().is_empty()
        && p.titulo().is_empty()
        && p.historial_atras().is_empty()
        && e.barra().is_empty()
        && *p.contenido() == EstadoContenido::Bienvenida
}

#[test]
fn panico_manual_purga_todo_y_sale() {
    let (mut v, salidas) = ventana(ConfiguracionWodw::default());
    con_rastro(&mut v);
    assert!(!sin_rastro(&v));
    v.activar_panico();
    assert!(sin_rastro(&v));
    assert_eq!(salidas.get(), 1);
}

#[test]
fn panico_automatico_del_ids_no_requiere_pulsar_nada() {
    let (mut v, salidas) = ventana(ConfiguracionWodw::default());
    con_rastro(&mut v);
    v.atender_evento(EventoSesion::PanicoAutomatico);
    assert!(sin_rastro(&v));
    assert_eq!(salidas.get(), 1);
}

#[test]
fn panico_sin_abortar_si_asi_se_configura() {
    let mut cfg = ConfiguracionWodw::default();
    cfg.panico.abortar_proceso = false;
    let (mut v, salidas) = ventana(cfg);
    con_rastro(&mut v);
    v.activar_panico();
    assert!(sin_rastro(&v));
    assert_eq!(salidas.get(), 0);
}

#[test]
fn inactividad_purga_automaticamente() {
    let (mut v, salidas) = ventana(ConfiguracionWodw::default());
    con_rastro(&mut v);
    v.comprobar_inactividad(Instant::now() + Duration::from_secs(29 * 60));
    assert!(!sin_rastro(&v));
    v.comprobar_inactividad(Instant::now() + Duration::from_secs(31 * 60));
    assert!(sin_rastro(&v));
    assert_eq!(
        salidas.get(),
        0,
        "la inactividad purga pero no cierra la aplicación"
    );
}

#[test]
fn inactividad_desactivada_con_cero() {
    let mut cfg = ConfiguracionWodw::default();
    cfg.automatizacion.minutos_inactividad_purga = 0;
    let (mut v, _) = ventana(cfg);
    con_rastro(&mut v);
    v.comprobar_inactividad(Instant::now() + Duration::from_secs(24 * 3600));
    assert!(!sin_rastro(&v));
}

#[test]
fn fallo_hostil_purga_la_pestana_afectada() {
    let (mut v, _) = ventana(ConfiguracionWodw::default());
    v.ir_a("http://malo.onion/");
    let id = v.estado().pestana_activa().id();
    v.atender_evento(EventoSesion::Navegacion {
        id_pestana: id,
        solicitud: 1,
        resultado: Err(FalloNavegacion {
            mensaje: "hostil".into(),
            purgar_pestana: true,
        }),
    });
    assert!(v.estado().pestana_activa().url().is_empty());
}

#[test]
fn titulo_no_ascii_no_rompe_la_interfaz() {
    let (mut v, _) = ventana(ConfiguracionWodw::default());
    v.ir_a("http://aaaaaaaaañññ.onion/");
    // Se prueban todas las longitudes para atravesar fronteras de carácter multibyte.
    for maximo in 1..40 {
        let corto = v.estado().pestana_activa().titulo_corto(maximo);
        assert!(corto.chars().count() <= maximo);
    }
}

/// Tamaño de ventana de la comprobación de desbordamiento, en puntos: el
/// tamaño inicial habitual de la ventana en un portátil.
const ANCHO_VENTANA: f32 = 1200.0;
const ALTO_VENTANA: f32 = 900.0;

/// Dibuja la ventana sin pantalla y devuelve el borde derecho del texto más
/// alejado. Solo cuenta el texto: los bordes de los paneles de egui sobresalen
/// medio punto por diseño y no se recortan. Se dibujan varios fotogramas porque
/// egui ajusta tamaños en el segundo.
fn borde_derecho_dibujado(v: &mut VentanaPrincipal) -> f32 {
    let ctx = egui::Context::default();
    let entrada = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(ANCHO_VENTANA, ALTO_VENTANA),
        )),
        ..Default::default()
    };
    for _ in 0..2 {
        ctx.run_ui(entrada(), |ui| v.dibujar(ui))
            .drop_without_applying_deltas();
    }
    let salida = ctx.run_ui(entrada(), |ui| v.dibujar(ui));
    let borde = salida
        .shapes
        .iter()
        .filter(|s| matches!(s.shape, egui::Shape::Text(_)))
        .map(|s| s.shape.visual_bounding_rect().max.x)
        .filter(|x| x.is_finite())
        .fold(0.0, f32::max);
    salida.drop_without_applying_deltas();
    borde
}

#[test]
fn la_barra_de_navegacion_cabe_en_la_ventana() {
    let (mut v, _) = ventana(ConfiguracionWodw::default());
    let borde = borde_derecho_dibujado(&mut v);
    assert!(
        borde <= ANCHO_VENTANA,
        "la interfaz se sale de la ventana: llega a x={borde} con ancho {ANCHO_VENTANA}"
    );
}

#[test]
fn todos_los_botones_se_pueden_dibujar_con_las_fuentes_de_la_app() {
    let ctx = egui::Context::default();
    ctx.run_ui(egui::RawInput::default(), |_| {})
        .drop_without_applying_deltas();
    let fuente = egui::FontId::proportional(14.0);
    for texto in wodw::ui::textos::TEXTOS_DE_BOTONES {
        let faltan: String = texto
            .chars()
            // `has_glyph` no ve los emojis aunque se dibujen; un carácter sin
            // dibujo es el que mide cero de ancho (calibrado con «⮜», que sale «□»).
            .filter(|&c| !c.is_whitespace() && ctx.fonts_mut(|f| f.glyph_width(&fuente, c)) <= 0.0)
            .collect();
        assert!(
            faltan.is_empty(),
            "el botón «{texto}» tiene caracteres sin dibujo en las fuentes: «{faltan}»"
        );
    }
}

/// Busca un texto entre lo dibujado tras colocar el puntero en `punto`.
fn texto_dibujado_con_puntero_en(v: &mut VentanaPrincipal, punto: egui::Pos2) -> String {
    let ctx = egui::Context::default();
    let mut tiempo = 0.0;
    let mut entrada = |eventos: Vec<egui::Event>| {
        tiempo += 1.0;
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(ANCHO_VENTANA, ALTO_VENTANA),
            )),
            time: Some(tiempo),
            events: eventos,
            ..Default::default()
        }
    };
    let mut textos = String::new();
    // El puntero se mueve una sola vez y luego se queda quieto: egui solo muestra
    // la viñeta cuando el ratón lleva un rato parado.
    for fotograma in 0..4 {
        let eventos = if fotograma == 0 {
            vec![egui::Event::PointerMoved(punto)]
        } else {
            Vec::new()
        };
        let salida = ctx.run_ui(entrada(eventos), |ui| v.dibujar(ui));
        for forma in &salida.shapes {
            if let egui::Shape::Text(t) = &forma.shape {
                textos.push_str(t.galley.text());
                textos.push('\n');
            }
        }
        salida.drop_without_applying_deltas();
    }
    textos
}

#[test]
fn el_boton_del_panico_explica_lo_que_hace_al_pasar_el_raton() {
    let cfg = ConfiguracionWodw::default();
    let ayuda = wodw::ui::textos::ayuda_panico(cfg.panico.pulsaciones);
    let (mut v, _) = ventana(cfg);
    // El botón del pánico es el último de la barra: a la derecha, en la segunda fila.
    let sobre_el_boton = egui::pos2(ANCHO_VENTANA - 30.0, 41.0);
    let dibujado = texto_dibujado_con_puntero_en(&mut v, sobre_el_boton);
    assert!(
        dibujado.contains(&ayuda),
        "no aparece la viñeta del pánico al pasar el ratón; texto dibujado:\n{dibujado}"
    );
}
