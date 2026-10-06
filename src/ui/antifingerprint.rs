//! Mitigaciones de huella digital en la interfaz: *letterboxing* canónico.
//!
//! El área de contenido se redondea hacia abajo a múltiplos fijos (200×100 px
//! por defecto, como Tor Browser) para que el tamaño exacto de la ventana o del
//! monitor no sea deducible. Los parámetros vienen de `[interfaz]`.

use crate::configuracion::ConfiguracionInterfaz;
use egui::{Color32, Rect, Vec2};

/// Color neutro de las bandas de relleno.
pub const COLOR_FONDO_LETTERBOXING: Color32 = Color32::from_rgb(18, 18, 18);

/// Geometría calculada del letterboxing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MargenesLetterbox {
    /// Rectángulo canónico donde se dibuja el contenido.
    pub area_contenido: Rect,
    /// Margen superior e inferior.
    pub margen_vertical: f32,
    /// Margen lateral.
    pub margen_horizontal: f32,
}

/// Dimensión canónica para un espacio disponible.
pub fn calcular_dimension_canonica(
    ancho: f32,
    alto: f32,
    cfg: &ConfiguracionInterfaz,
) -> (f32, f32) {
    let canonico = |disponible: f32, paso: f32, minimo: f32, maximo: f32| {
        ((disponible / paso).floor() * paso).clamp(minimo, maximo)
    };
    (
        canonico(
            ancho,
            cfg.incremento_horizontal,
            cfg.ancho_minimo,
            cfg.ancho_maximo,
        ),
        canonico(
            alto,
            cfg.incremento_vertical,
            cfg.alto_minimo,
            cfg.alto_maximo,
        ),
    )
}

/// Área centrada y márgenes para `disponible`.
pub fn calcular_letterboxing(disponible: Rect, cfg: &ConfiguracionInterfaz) -> MargenesLetterbox {
    let (ancho, alto) = calcular_dimension_canonica(disponible.width(), disponible.height(), cfg);
    let margen_horizontal = ((disponible.width() - ancho) / 2.0).max(0.0);
    let margen_vertical = ((disponible.height() - alto) / 2.0).max(0.0);
    let origen = egui::pos2(
        disponible.min.x + margen_horizontal,
        disponible.min.y + margen_vertical,
    );
    MargenesLetterbox {
        area_contenido: Rect::from_min_size(origen, Vec2::new(ancho, alto)),
        margen_vertical,
        margen_horizontal,
    }
}

/// Opciones de ventana de `eframe` a partir de la configuración.
pub fn crear_opciones_nativas_seguras(
    cfg: &ConfiguracionInterfaz,
    titulo: &str,
) -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(titulo)
        .with_inner_size([cfg.ancho_inicial, cfg.alto_inicial])
        .with_min_inner_size([cfg.ancho_minimo, cfg.alto_minimo]);
    if let Some(icono) = super::icono::icono_aplicacion() {
        viewport = viewport.with_icon(icono);
    }
    eframe::NativeOptions {
        viewport,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redondea_a_multiplos_y_respeta_limites() {
        let cfg = ConfiguracionInterfaz::default();
        assert_eq!(
            calcular_dimension_canonica(1080.0, 750.0, &cfg),
            (1000.0, 700.0)
        );
        assert_eq!(
            calcular_dimension_canonica(400.0, 200.0, &cfg),
            (600.0, 400.0)
        );
        assert_eq!(
            calcular_dimension_canonica(3840.0, 2160.0, &cfg),
            (1400.0, 1000.0)
        );
    }

    #[test]
    fn centra_el_area_de_contenido() {
        let cfg = ConfiguracionInterfaz::default();
        let total = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(1100.0, 850.0));
        let l = calcular_letterboxing(total, &cfg);
        assert_eq!(l.area_contenido.size(), Vec2::new(1000.0, 800.0));
        assert_eq!((l.margen_horizontal, l.margen_vertical), (50.0, 25.0));
    }
}
