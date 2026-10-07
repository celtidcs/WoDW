//! Fuentes embebidas para los alfabetos que las de `egui` no cubren.
//!
//! Van dentro del ejecutable (nunca se usan fuentes del sistema: serían una
//! huella del equipo). Se añaden como **respaldo** tras las de serie, así que
//! solo se usan para los caracteres que aquellas no tienen.
//!
//! Límite conocido: `egui` no hace modelado de texto complejo, así que el árabe,
//! el persa y el jemer muestran sus letras pero sin unirlas ni reordenarlas
//! como lo haría un navegador (se leen las letras sueltas).

use egui::{FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

/// Fuentes de respaldo: (nombre interno, datos). Licencias en `recursos/fuentes/`.
const FUENTES: &[(&str, &[u8])] = &[
    // Latino extendido (vietnamita), griego y cirílico. OFL 1.1.
    (
        "noto-sans",
        include_bytes!("../../recursos/fuentes/NotoSans-Regular.ttf"),
    ),
    // Árabe y persa. OFL 1.1.
    (
        "noto-sans-arabic",
        include_bytes!("../../recursos/fuentes/NotoSansArabic-Regular.ttf"),
    ),
    // Georgiano. OFL 1.1.
    (
        "noto-sans-georgian",
        include_bytes!("../../recursos/fuentes/NotoSansGeorgian-Regular.ttf"),
    ),
    // Jemer. OFL 1.1.
    (
        "noto-sans-khmer",
        include_bytes!("../../recursos/fuentes/NotoSansKhmer-Regular.ttf"),
    ),
    // Chino y japonés (≈4 MB). Apache 2.0 (AOSP).
    (
        "droid-sans-fallback",
        include_bytes!("../../recursos/fuentes/DroidSansFallbackFull.ttf"),
    ),
    // Coreano (hangul), que la anterior no trae. OFL 1.1.
    (
        "nanum-gothic",
        include_bytes!("../../recursos/fuentes/NanumGothic-Regular.ttf"),
    ),
];

/// Instala las fuentes de respaldo en el contexto.
pub fn instalar(ctx: &egui::Context) {
    let mut definiciones = FontDefinitions::default();
    for (nombre, datos) in FUENTES {
        definiciones.font_data.insert(
            (*nombre).to_string(),
            Arc::new(FontData::from_static(datos)),
        );
        for familia in [FontFamily::Proportional, FontFamily::Monospace] {
            definiciones
                .families
                .entry(familia)
                .or_default()
                .push((*nombre).to_string());
        }
    }
    ctx.set_fonts(definiciones);
}
