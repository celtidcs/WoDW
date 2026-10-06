//! Indicador y panel de telemetría del IDS.

use crate::ids::eventos::NivelSeveridad;
use crate::ids::motor::ResumenTelemetria;
use crate::ui::textos;
use egui::{RichText, ScrollArea, Ui};

/// Altura máxima del registro de eventos en el panel.
const ALTURA_REGISTRO: f32 = 250.0;

/// Botón compacto de la cabecera; devuelve `true` si se pulsó.
pub fn indicador_cabecera(ui: &mut Ui, resumen: &ResumenTelemetria) -> bool {
    let texto = RichText::new(textos::indicador_ids(resumen.alerta_maxima))
        .color(textos::color_severidad(resumen.alerta_maxima))
        .strong();
    ui.button(texto).clicked()
}

/// Panel lateral; devuelve `true` si se pidió rotar los circuitos.
pub fn panel_telemetria(ui: &mut Ui, resumen: &ResumenTelemetria) -> bool {
    ui.heading("Panel defensivo (respuesta automática)");
    ui.group(|ui| {
        ui.label(format!("Eventos: {}", resumen.total_eventos));
        for nivel in [
            NivelSeveridad::Informativo,
            NivelSeveridad::Medio,
            NivelSeveridad::Alto,
            NivelSeveridad::Critico,
        ] {
            ui.colored_label(
                textos::color_severidad(nivel),
                format!(
                    "• {}: {}",
                    textos::etiqueta_severidad(nivel),
                    resumen.eventos_de(nivel)
                ),
            );
        }
        ui.label(format!(
            "Rotaciones automáticas de circuitos: {}",
            resumen.rotaciones_automaticas
        ));
    });
    let rotar = ui.button(textos::BOTON_ROTAR).clicked();
    ui.separator();
    ScrollArea::vertical()
        .max_height(ALTURA_REGISTRO)
        .show(ui, |ui| {
            if resumen.historial_reciente.is_empty() {
                ui.weak("Sin incidentes en esta sesión.");
            }
            for evento in resumen.historial_reciente.iter().rev() {
                ui.group(|ui| {
                    ui.colored_label(
                        textos::color_severidad(evento.severidad),
                        textos::etiqueta_severidad(evento.severidad),
                    );
                    ui.label(textos::describir_vector(&evento.vector));
                });
            }
        });
    rotar
}
