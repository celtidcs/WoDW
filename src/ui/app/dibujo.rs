//! Dibujado de la ventana en cada fotograma y recogida de lo que pulsa el
//! usuario. Lo que esas pulsaciones provocan vive en el módulo padre.

use super::VentanaPrincipal;
use crate::ids::motor::ResumenTelemetria;
use crate::maestro::navegacion::ContenidoPagina;
use crate::maestro::sesion::OrdenSesion;
use crate::ui::antifingerprint::{calcular_letterboxing, COLOR_FONDO_LETTERBOXING};
use crate::ui::contenido::mostrar_pestana;
use crate::ui::estado::EstadoContenido;
use crate::ui::reproductor::AccionReproductor;
use crate::ui::telemetria::{indicador_cabecera, panel_telemetria};
use crate::ui::textos;
use egui::{Align, Layout, RichText, ScrollArea, Ui};
use std::time::{Duration, Instant};

/// Frecuencia mínima de repintado para comprobar la inactividad sin eventos.
const INTERVALO_REVISION: Duration = Duration::from_secs(1);
/// Ancho mínimo del panel de telemetría.
const ANCHO_PANEL_TELEMETRIA: f32 = 300.0;
/// Radio de las esquinas del área de contenido.
const RADIO_CONTENIDO: f32 = 4.0;
/// Separación superior del contenido.
const MARGEN_CONTENIDO: f32 = 12.0;

impl VentanaPrincipal {
    /// Procesa el teclado y la actividad del usuario.
    fn atender_entrada(&mut self, ctx: &egui::Context) {
        let ahora = Instant::now();
        let (escape, actividad) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Escape),
                !i.events.is_empty() || i.pointer.is_moving(),
            )
        });
        if actividad {
            self.estado.registrar_actividad(ahora);
        }
        if escape && self.estado.registrar_escape(ahora) {
            self.activar_panico();
        }
        self.comprobar_inactividad(ahora);
    }

    /// Resumen del IDS de la sesión (vacío sin sesión).
    fn resumen_ids(&self) -> ResumenTelemetria {
        self.sesion
            .as_ref()
            .map(|s| s.control_ids().obtener_resumen())
            .unwrap_or_default()
    }

    /// Pestañas abiertas, con sus botones de cerrar y de abrir otra.
    fn fila_pestanas(&mut self, ui: &mut Ui) {
        let maximo = self.cfg.interfaz.longitud_titulo_pestana;
        let mut activar = None;
        let mut cerrar = None;
        for (indice, pestana) in self.estado.pestanas().iter().enumerate() {
            let activa = indice == self.estado.indice_activo();
            if ui
                .selectable_label(activa, pestana.titulo_corto(maximo))
                .clicked()
            {
                activar = Some(indice);
            }
            if self.estado.pestanas().len() > 1
                && ui.small_button(textos::BOTON_CERRAR_PESTANA).clicked()
            {
                cerrar = Some(indice);
            }
            ui.separator();
        }
        if let Some(indice) = activar {
            self.estado.activar(indice);
        }
        if let Some(id) = cerrar.and_then(|i| self.estado.cerrar_pestana(i)) {
            if let Some(sesion) = &self.sesion {
                sesion.ordenar(OrdenSesion::CerrarPestana(id));
            }
        }
        if ui.button(textos::BOTON_NUEVA_PESTANA).clicked() {
            self.estado.abrir_pestana();
        }
    }

    /// Atrás, adelante, motor, barra de direcciones y botones de la derecha.
    fn fila_navegacion(&mut self, ui: &mut Ui, resumen: &ResumenTelemetria) {
        if ui.button(textos::BOTON_ATRAS).clicked() {
            if let Some(s) = self.estado.retroceder() {
                self.enviar(s);
            }
        }
        if ui.button(textos::BOTON_ADELANTE).clicked() {
            if let Some(s) = self.estado.avanzar() {
                self.enviar(s);
            }
        }
        self.selector_motor(ui);
        // De derecha a izquierda: los botones se colocan primero y la barra de
        // direcciones ocupa exactamente lo que queda, sea cual sea el ancho.
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let panico = egui::Button::new(RichText::new(textos::BOTON_PANICO).strong())
                .fill(textos::COLOR_PANICO);
            let ayuda = textos::ayuda_panico(self.cfg.panico.pulsaciones);
            if ui.add(panico).on_hover_text(ayuda).clicked() {
                self.activar_panico();
            }
            if indicador_cabecera(ui, resumen) {
                self.mostrar_telemetria = !self.mostrar_telemetria;
            }
            if ui.button(textos::BOTON_REGISTRO).clicked() {
                self.panel_registro.abierto = !self.panel_registro.abierto;
            }
            let ir = ui.button(textos::BOTON_IR).clicked();
            let campo = ui.add_sized(
                [ui.available_width(), ui.spacing().interact_size.y],
                egui::TextEdit::singleline(self.estado.barra_mut()).hint_text(textos::PISTA_BARRA),
            );
            let enter = campo.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter || ir {
                let entrada = self.estado.barra().to_string();
                self.ir_a(&entrada);
            }
        });
    }

    /// Desplegable del motor de búsqueda.
    fn selector_motor(&mut self, ui: &mut Ui) {
        let mut seleccionado = self.catalogo.seleccionado();
        egui::ComboBox::from_id_salt("selector_motor")
            .selected_text(self.catalogo.nombre_seleccionado().to_string())
            .show_ui(ui, |ui| {
                for (indice, motor) in self.catalogo.motores().iter().enumerate() {
                    ui.selectable_value(&mut seleccionado, indice, &motor.nombre);
                }
            });
        self.catalogo.seleccionar(seleccionado);
    }

    /// Contenido de la pestaña activa, con letterboxing y reproductor.
    fn panel_central(&mut self, ui: &mut Ui) {
        let disponible = ui.available_rect_before_wrap();
        let caja = calcular_letterboxing(disponible, &self.cfg.interfaz);
        ui.painter()
            .rect_filled(disponible, 0.0, COLOR_FONDO_LETTERBOXING);
        let mut pulsado = None;
        let mut accion = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(caja.area_contenido), |ui| {
            ui.painter().rect_filled(
                caja.area_contenido,
                RADIO_CONTENIDO,
                textos::COLOR_FONDO_CONTENIDO,
            );
            ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(MARGEN_CONTENIDO);
                pulsado = mostrar_pestana(
                    ui,
                    self.estado.pestana_activa(),
                    &self.catalogo,
                    &mut self.cache_imagen,
                );
                accion = self.controles_reproduccion(ui);
            });
        });
        if let Some(url) = pulsado {
            self.ir_a(&url);
        }
        match accion {
            Some(AccionReproductor::Reproducir) => self.reproducir(),
            Some(AccionReproductor::Parar) => self.reproductor.parar(),
            None => {}
        }
    }

    /// Controles del reproductor si la pestaña activa muestra un medio.
    fn controles_reproduccion(&mut self, ui: &mut Ui) -> Option<AccionReproductor> {
        let pestana = self.estado.pestana_activa();
        if !matches!(
            pestana.contenido(),
            EstadoContenido::Pagina(ContenidoPagina::Medio { .. })
        ) {
            return None;
        }
        let (id, solicitud) = (pestana.id(), pestana.solicitud());
        ui.add_space(MARGEN_CONTENIDO);
        self.reproductor.controles(ui, id, solicitud)
    }
}

impl eframe::App for VentanaPrincipal {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.dibujar(ui);
    }

    fn on_exit(&mut self) {
        self.al_cerrar();
    }
}

impl VentanaPrincipal {
    /// Dibuja un fotograma completo de la ventana y atiende eventos y teclado.
    /// Es lo que hace `eframe` en cada fotograma; público para poder dibujar la
    /// ventana sin pantalla en las pruebas.
    pub fn dibujar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.atender_eventos_sesion();
        self.atender_entrada(&ctx);
        self.vigilar_reproduccion();
        self.reproductor.avanzar(&ctx, Instant::now());
        let resumen = self.resumen_ids();
        self.anotar_eventos_ids(&resumen);
        self.panel_registro.mostrar(&ctx, &mut self.registro);
        egui::Panel::top("panel_superior").show(ui, |ui| {
            ui.horizontal(|ui| self.fila_pestanas(ui));
            ui.separator();
            ui.horizontal(|ui| self.fila_navegacion(ui, &resumen));
        });
        egui::Panel::bottom("panel_inferior").show(ui, |ui| self.panel_inferior(ui));
        if self.mostrar_telemetria {
            egui::Panel::right("panel_telemetria")
                .min_size(ANCHO_PANEL_TELEMETRIA)
                .show(ui, |ui| self.panel_derecho(ui, &resumen));
        }
        egui::CentralPanel::default().show(ui, |ui| self.panel_central(ui));
        ctx.request_repaint_after(INTERVALO_REVISION);
    }

    /// Barra de estado y, si la hay, el aviso de versión nueva.
    fn panel_inferior(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(textos::pie_estado()).italics());
            ui.separator();
            ui.label(&self.mensaje_estado);
        });
        let Some(nueva) = &self.version_nueva else {
            return;
        };
        let mut cerrar = false;
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(
                textos::COLOR_AVISO_VERSION,
                textos::aviso_version(&nueva.version),
            );
            ui.add(
                egui::TextEdit::singleline(&mut nueva.enlace.as_str()).desired_width(f32::INFINITY),
            );
            cerrar = ui.small_button(textos::BOTON_CERRAR_PESTANA).clicked();
        });
        if cerrar {
            self.version_nueva = None;
        }
    }

    /// Panel de telemetría; su botón pide circuitos nuevos para todo.
    fn panel_derecho(&self, ui: &mut Ui, resumen: &ResumenTelemetria) {
        if panel_telemetria(ui, resumen) {
            if let Some(sesion) = &self.sesion {
                sesion.ordenar(OrdenSesion::RotarAislamiento);
            }
        }
    }
}
