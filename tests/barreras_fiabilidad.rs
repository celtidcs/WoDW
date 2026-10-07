//! Barreras de la fiabilidad de buscadores y accesos directos (CA-21-2 a
//! CA-21-4): configuración, lista por defecto, textos e interfaz. Sin red.

use wodw::configuracion::{ConfiguracionWodw, Fiabilidad, MOTIVO_SIN_COMPROBAR};
use wodw::ui::textos;
use wodw::ui::VentanaPrincipal;

const AHMIA: &str = "juhanurmihxlp77nkq76byazcldy2hlmovfu2epvl5ankdibsot4csyd.onion";

/// Configuración con un acceso propio y, opcionalmente, campos extra.
fn con_acceso(url: &str, extra: &str) -> String {
    format!(
        "[accesos]\n[[accesos.lista]]\nnombre = \"Prueba\"\ndescripcion = \"x\"\nurl = \"{url}\"\n{extra}"
    )
}

#[test]
fn accesos_se_validan_al_arrancar() {
    // Control: un acceso correcto se acepta.
    let bueno = con_acceso(&format!("http://{AHMIA}/"), "");
    assert!(
        ConfiguracionWodw::desde_texto(&bueno).is_ok(),
        "{:?}",
        ConfiguracionWodw::desde_texto(&bueno).err()
    );
    for malo in [
        con_acceso("javascript:alert(1)", ""),
        con_acceso("file:///C:/Windows", ""),
        con_acceso(
            "http://xmh57jrknzkhv6y3ls3ubitzfqnkrwxhopf5aygthi7d6rfdvgchu6ad.onion/",
            "",
        ),
        con_acceso(&format!("http://{AHMIA}/"), "motivo = \"  \"\n"),
    ] {
        assert!(ConfiguracionWodw::desde_texto(&malo).is_err(), "{malo}");
    }
}

#[test]
fn motor_sin_motivo_se_rechaza() {
    let toml = format!(
        "[motores]\npredeterminado = \"P\"\n[[motores.lista]]\nnombre = \"P\"\ndescripcion = \"x\"\n\
         plantilla = \"http://{AHMIA}/?q={{consulta}}\"\nmotivo = \"\"\n"
    );
    assert!(ConfiguracionWodw::desde_texto(&toml).is_err());
}

#[test]
fn lo_anadido_a_mano_sin_fiabilidad_queda_sin_verificar() {
    let cfg = ConfiguracionWodw::desde_texto(&con_acceso(&format!("http://{AHMIA}/"), ""))
        .expect("válida");
    let acceso = &cfg.accesos.lista[0];
    assert_eq!(acceso.fiabilidad, Fiabilidad::SinVerificar);
    assert_eq!(acceso.motivo, MOTIVO_SIN_COMPROBAR);
}

#[test]
fn lista_por_defecto() {
    let cfg = ConfiguracionWodw::default();
    let motores: Vec<(&str, Fiabilidad)> = cfg
        .motores
        .lista
        .iter()
        .map(|m| (m.nombre.as_str(), m.fiabilidad))
        .collect();
    use Fiabilidad::{SinVerificar as S, Verificado as V};
    assert_eq!(
        motores,
        [
            ("Ahmia", V),
            ("DuckDuckGo Onion", V),
            ("OnionLand", V),
            ("VormWeb", V),
            ("Tor66", S),
        ]
    );
    assert_eq!(cfg.motores.predeterminado, "Ahmia");
    let accesos: Vec<(&str, Fiabilidad)> = cfg
        .accesos
        .lista
        .iter()
        .map(|a| (a.nombre.as_str(), a.fiabilidad))
        .collect();
    assert_eq!(
        accesos,
        [("dark.fail", V), ("tor.taxi", V), ("Tor Project", V)]
    );
    for directorio in &cfg.accesos.lista[..2] {
        assert!(
            directorio.descripcion.contains("mercados ilegales"),
            "{}",
            directorio.nombre
        );
    }
    let todo = format!("{:?}{:?}", cfg.motores, cfg.accesos).to_lowercase();
    for excluido in ["excavator", "hidden wiki", "torch", "haystak", "phobos"] {
        assert!(!todo.contains(excluido), "{excluido}");
    }
    for m in &cfg.motores.lista {
        assert!(!m.motivo.trim().is_empty(), "{}", m.nombre);
    }
}

#[test]
fn textos_de_fiabilidad() {
    let cfg = ConfiguracionWodw::default();
    let ahmia = &cfg.motores.lista[0];
    let tor66 = cfg
        .motores
        .lista
        .iter()
        .find(|m| m.nombre == "Tor66")
        .expect("Tor66");
    assert_eq!(textos::nombre_motor(ahmia), "Ahmia");
    assert!(textos::nombre_motor(tor66).contains(textos::MARCA_SIN_VERIFICAR));
    assert!(textos::aviso_motor_sin_verificar(ahmia).is_none());
    let aviso = textos::aviso_motor_sin_verificar(tor66).expect("aviso");
    assert!(aviso.contains(&tor66.motivo), "{aviso}");
    assert!(textos::linea_motor_bienvenida(tor66).contains("sin verificar"));
}

/// Textos dibujados en una ventana de `ancho` puntos tras unos fotogramas.
fn textos_dibujados(v: &mut VentanaPrincipal, ancho: f32) -> Vec<(String, egui::Rect)> {
    let ctx = egui::Context::default();
    let entrada = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(ancho, 700.0),
        )),
        ..Default::default()
    };
    for _ in 0..2 {
        ctx.run_ui(entrada(), |ui| v.dibujar(ui))
            .drop_without_applying_deltas();
    }
    let salida = ctx.run_ui(entrada(), |ui| v.dibujar(ui));
    let textos = salida
        .shapes
        .iter()
        .filter_map(|f| match &f.shape {
            egui::Shape::Text(t) => {
                Some((t.galley.text().to_string(), f.shape.visual_bounding_rect()))
            }
            _ => None,
        })
        .collect();
    salida.drop_without_applying_deltas();
    textos
}

#[test]
fn el_panel_de_accesos_muestra_fiabilidad_y_motivo() {
    let cfg = ConfiguracionWodw::default();
    let mut v = VentanaPrincipal::nueva(cfg.clone(), None, Box::new(|| {}));
    assert!(!textos_dibujados(&mut v, 1000.0)
        .iter()
        .any(|(t, _)| t == textos::TITULO_ACCESOS));
    v.alternar_accesos();
    let dibujado: Vec<String> = textos_dibujados(&mut v, 1000.0)
        .into_iter()
        .map(|(t, _)| t)
        .collect();
    for acceso in &cfg.accesos.lista {
        assert!(dibujado.contains(&acceso.nombre), "{}", acceso.nombre);
        assert!(dibujado.contains(&acceso.motivo), "{}", acceso.motivo);
    }
    assert!(dibujado.iter().any(|t| t == "Verificado"));
}

#[test]
fn pulsar_un_acceso_lo_abre_en_la_pestana() {
    let cfg = ConfiguracionWodw::default();
    let mut v = VentanaPrincipal::nueva(cfg.clone(), None, Box::new(|| {}));
    v.alternar_accesos();
    assert!(v.abrir_acceso(1));
    assert_eq!(v.estado().pestana_activa().url(), cfg.accesos.lista[1].url);
    assert!(
        !v.accesos_abiertos(),
        "el panel se cierra al abrir un acceso"
    );
    assert!(!v.abrir_acceso(99));
}

/// Con un buscador sin verificar elegido, el selector lo muestra con la marca
/// (en ámbar, con el aviso al pasar el ratón) y, al ancho mínimo, el campo de
/// dirección conserva un ancho útil.
#[test]
fn buscador_sin_verificar_avisa_y_la_barra_sigue_usable() {
    let mut cfg = ConfiguracionWodw::default();
    cfg.motores.predeterminado = "Tor66".to_string();
    let ancho = cfg.interfaz.ancho_minimo_ventana;
    let mut v = VentanaPrincipal::nueva(cfg, None, Box::new(|| {}));
    let dibujado = textos_dibujados(&mut v, ancho);
    let tor66 = format!("Tor66 {}", textos::MARCA_SIN_VERIFICAR);
    assert!(dibujado.iter().any(|(t, _)| *t == tor66), "{dibujado:?}");
    let campo = dibujado
        .iter()
        .find(|(t, _)| t == textos::PISTA_BARRA)
        .map(|(_, r)| *r)
        .expect("campo de dirección");
    assert!(campo.width() >= 150.0, "campo de {} puntos", campo.width());
    // Con Ahmia (verificado), el selector muestra «Ahmia» sin marca. (La
    // bienvenida lista todos los buscadores con la suya, en líneas más largas.)
    let mut v = VentanaPrincipal::nueva(ConfiguracionWodw::default(), None, Box::new(|| {}));
    let dibujado = textos_dibujados(&mut v, ancho);
    assert!(dibujado.iter().any(|(t, _)| t == "Ahmia"));
    assert!(!dibujado.iter().any(|(t, _)| *t == tor66));
}
