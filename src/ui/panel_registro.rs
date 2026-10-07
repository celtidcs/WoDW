//! Panel del registro de la sesión (CA-RS1–RS2): elegir qué se registra y
//! cómo se guarda, con la explicación de cada opción a la vista y una
//! confirmación expresa antes de activar la opción más reveladora.

use crate::configuracion::{ContenidoRegistro, GuardadoRegistro};
use crate::registro::{CambioRegistro, RegistroSesion};
use crate::ui::textos;
use egui::{RichText, ScrollArea, Ui};

/// Alto máximo de la lista de sucesos dentro del panel.
const ALTO_LISTA: f32 = 220.0;
/// Ancho del texto explicativo de cada opción.
const ANCHO_EXPLICACION: f32 = 520.0;
/// Color de la advertencia de confirmación (ámbar del nivel «Alto» del IDS).
const COLOR_CONFIRMACION: egui::Color32 = egui::Color32::from_rgb(230, 126, 34);

/// Opción pendiente de confirmar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pendiente {
    Contenido(ContenidoRegistro),
    Guardado(GuardadoRegistro),
}

/// Estado del panel.
#[derive(Debug, Clone, Default)]
pub struct PanelRegistro {
    /// Si el panel está abierto.
    pub abierto: bool,
    pendiente: Option<Pendiente>,
    ruta: String,
    mensaje: Option<String>,
}

impl PanelRegistro {
    /// Panel cerrado con la ruta de guardado propuesta.
    pub fn nuevo(ruta: String) -> Self {
        Self {
            ruta,
            ..Self::default()
        }
    }

    /// Ruta de guardado actual.
    pub fn ruta(&self) -> &str {
        &self.ruta
    }

    /// Dibuja el panel si está abierto.
    pub fn mostrar(&mut self, ctx: &egui::Context, registro: &mut RegistroSesion) {
        let mut abierto = self.abierto;
        egui::Window::new(textos::TITULO_REGISTRO)
            .open(&mut abierto)
            .collapsible(false)
            .show(ctx, |ui| self.contenido(ui, registro));
        self.abierto = abierto;
    }

    fn contenido(&mut self, ui: &mut Ui, registro: &mut RegistroSesion) {
        ui.label(RichText::new(textos::REGISTRO_QUE).strong());
        for opcion in [ContenidoRegistro::Seguridad, ContenidoRegistro::Completo] {
            let nombre = match opcion {
                ContenidoRegistro::Seguridad => textos::OPCION_SEGURIDAD,
                ContenidoRegistro::Completo => textos::OPCION_COMPLETO,
            };
            if ui.radio(registro.contenido() == opcion, nombre).clicked()
                && registro.solicitar_contenido(opcion) == CambioRegistro::RequiereConfirmacion
            {
                self.pendiente = Some(Pendiente::Contenido(opcion));
            }
            explicacion(ui, textos::explicacion_contenido_registro(opcion));
        }
        ui.separator();
        ui.label(RichText::new(textos::REGISTRO_COMO).strong());
        for opcion in [GuardadoRegistro::Manual, GuardadoRegistro::Automatico] {
            let nombre = match opcion {
                GuardadoRegistro::Manual => textos::OPCION_MANUAL,
                GuardadoRegistro::Automatico => textos::OPCION_AUTOMATICO,
            };
            if ui.radio(registro.guardado() == opcion, nombre).clicked()
                && registro.solicitar_guardado(opcion) == CambioRegistro::RequiereConfirmacion
            {
                self.pendiente = Some(Pendiente::Guardado(opcion));
            }
            explicacion(ui, textos::explicacion_guardado_registro(opcion));
        }
        self.confirmacion(ui, registro);
        ui.separator();
        self.acciones(ui, registro);
        ui.separator();
        ScrollArea::vertical()
            .max_height(ALTO_LISTA)
            .show(ui, |ui| {
                let texto = registro.texto();
                ui.label(if texto.is_empty() {
                    textos::REGISTRO_VACIO
                } else {
                    texto.as_str()
                });
            });
    }

    /// Confirmación de la opción más reveladora, con su explicación repetida.
    fn confirmacion(&mut self, ui: &mut Ui, registro: &mut RegistroSesion) {
        let Some(pendiente) = self.pendiente else {
            return;
        };
        let aviso =
            textos::aviso_confirmacion_registro(matches!(pendiente, Pendiente::Contenido(_)));
        ui.separator();
        ui.colored_label(COLOR_CONFIRMACION, aviso);
        ui.horizontal(|ui| {
            if ui.button(textos::BOTON_CONFIRMAR).clicked() {
                match pendiente {
                    Pendiente::Contenido(c) => registro.confirmar_contenido(c),
                    Pendiente::Guardado(g) => registro.confirmar_guardado(g),
                }
                self.pendiente = None;
            }
            if ui.button(textos::BOTON_CANCELAR).clicked() {
                self.pendiente = None;
            }
        });
    }

    /// Ruta, guardar y borrar.
    fn acciones(&mut self, ui: &mut Ui, registro: &mut RegistroSesion) {
        ui.horizontal(|ui| {
            ui.label(textos::ETIQUETA_RUTA_REGISTRO);
            ui.text_edit_singleline(&mut self.ruta);
        });
        ui.horizontal(|ui| {
            if ui.button(textos::BOTON_GUARDAR_REGISTRO).clicked() {
                let resultado = registro.guardar(std::path::Path::new(&self.ruta));
                self.mensaje = Some(textos::registro_guardado(&resultado, &self.ruta));
            }
            if ui.button(textos::BOTON_BORRAR_REGISTRO).clicked() {
                registro.vaciar();
                self.mensaje = None;
            }
        });
        ui.label(RichText::new(textos::NOTA_BORRAR_REGISTRO).italics());
        if let Some(mensaje) = &self.mensaje {
            ui.label(mensaje);
        }
    }
}

/// Texto explicativo sangrado bajo una opción.
fn explicacion(ui: &mut Ui, texto: &str) {
    ui.indent(texto, |ui| {
        ui.set_max_width(ANCHO_EXPLICACION);
        ui.label(texto);
    });
}
