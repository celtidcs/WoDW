//! Panel «Accesos»: los sitios de `[accesos]` con su fiabilidad y el motivo,
//! que se abren con un clic en la pestaña activa.

use crate::configuracion::{AccesoDirecto, Fiabilidad};
use crate::ui::textos;
use egui::{RichText, Ui};

/// Panel flotante de accesos directos.
#[derive(Debug, Default)]
pub struct PanelAccesos {
    /// Si el panel está abierto.
    pub abierto: bool,
}

impl PanelAccesos {
    /// Dibuja el panel si está abierto y devuelve la dirección del acceso
    /// pulsado, si lo hay. Al pulsar uno, el panel se cierra.
    pub fn mostrar(&mut self, ctx: &egui::Context, accesos: &[AccesoDirecto]) -> Option<String> {
        let mut abierto = self.abierto;
        let mut pulsado = None;
        egui::Window::new(textos::TITULO_ACCESOS)
            .open(&mut abierto)
            .collapsible(false)
            .show(ctx, |ui| pulsado = contenido(ui, accesos));
        self.abierto = abierto && pulsado.is_none();
        pulsado
    }
}

/// Lista de accesos; devuelve la dirección del que se pulse.
fn contenido(ui: &mut Ui, accesos: &[AccesoDirecto]) -> Option<String> {
    ui.label(textos::INTRO_ACCESOS);
    let mut pulsado = None;
    for acceso in accesos {
        ui.separator();
        ui.horizontal(|ui| {
            if ui
                .link(RichText::new(&acceso.nombre).strong())
                .on_hover_text(&acceso.url)
                .clicked()
            {
                pulsado = Some(acceso.url.clone());
            }
            ui.colored_label(
                color_fiabilidad(acceso.fiabilidad),
                textos::etiqueta_fiabilidad(acceso.fiabilidad),
            );
        });
        ui.label(&acceso.descripcion);
        ui.label(RichText::new(&acceso.motivo).small().italics());
    }
    pulsado
}

/// Verde para lo verificado; ámbar para lo que no.
fn color_fiabilidad(fiabilidad: Fiabilidad) -> egui::Color32 {
    match fiabilidad {
        Fiabilidad::Verificado => textos::COLOR_INFORMATIVO,
        Fiabilidad::SinVerificar => textos::COLOR_ALTO,
    }
}
