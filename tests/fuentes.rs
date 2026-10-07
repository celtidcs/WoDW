//! Barrera de las fuentes embebidas: todos los alfabetos de las páginas
//! reales probadas (portada de Tor Project) tienen dibujo: la fuente tiene su
//! glifo o mide más de cero (calibrado con «⮜», que sale «□»).

/// Muestras por idioma.
const MUESTRAS: &[(&str, &str)] = &[
    ("ruso", "Русский язык, беларуская, українська"),
    ("griego", "Ελληνικά"),
    ("vietnamita", "Tiếng Việt"),
    ("árabe", "العربية"),
    ("persa", "فارسی"),
    ("japonés", "日本語 ひらがな カタカナ"),
    ("chino simplificado", "简体中文"),
    ("chino tradicional", "繁體中文"),
    ("coreano", "한국어"),
    ("georgiano", "ქართული"),
    ("jemer", "ភាសាខ្មែរ"),
];

/// Caracteres de `texto` sin dibujo en el contexto.
fn sin_dibujo(ctx: &egui::Context, texto: &str) -> String {
    let fuente = egui::FontId::proportional(14.0);
    texto
        .chars()
        .filter(|&c| !c.is_whitespace() && !c.is_ascii_punctuation())
        // Las marcas que se combinan (como el «្» jemer) miden cero por diseño:
        // cuenta también si la fuente tiene su glifo.
        .filter(|&c| !ctx.fonts_mut(|f| f.has_glyph(&fuente, c) || f.glyph_width(&fuente, c) > 0.0))
        .collect()
}

fn contexto(con_fuentes: bool) -> egui::Context {
    let ctx = egui::Context::default();
    if con_fuentes {
        wodw::ui::fuentes::instalar(&ctx);
    }
    ctx.run_ui(egui::RawInput::default(), |_| {})
        .drop_without_applying_deltas();
    ctx
}

#[test]
fn todos_los_alfabetos_tienen_dibujo() {
    let ctx = contexto(true);
    for (idioma, texto) in MUESTRAS {
        let faltan = sin_dibujo(&ctx, texto);
        assert!(faltan.is_empty(), "{idioma}: sin dibujo «{faltan}»");
    }
}

/// Control: sin las fuentes embebidas, el árabe no se puede dibujar.
#[test]
fn control_sin_fuentes_falta_el_arabe() {
    let ctx = contexto(false);
    assert!(!sin_dibujo(&ctx, "العربية").is_empty());
}
