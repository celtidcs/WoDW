//! Ventana principal: une el estado del navegador, la sesión del Maestro y los
//! paneles de `egui`.
//!
//! La ventana reacciona sola, sin que el usuario tenga que pulsar nada: aplica
//! los resultados de la sesión, ejecuta el pánico automático que ordene el IDS,
//! purga por inactividad y purga al cerrarse.

use crate::configuracion::ConfiguracionWodw;
use crate::ids::motor::ResumenTelemetria;
use crate::maestro::sesion::{ConexionSesion, EventoSesion, OrdenSesion};
use crate::ui::antifingerprint::{calcular_letterboxing, COLOR_FONDO_LETTERBOXING};
use crate::ui::contenido::{mostrar_pestana, CacheImagen};
use crate::ui::estado::{EstadoNavegador, ParametrosEstado, SolicitudNavegacion};
use crate::ui::motores::CatalogoMotores;
use crate::ui::telemetria::{indicador_cabecera, panel_telemetria};
use crate::ui::textos;
use egui::{Align, Layout, RichText, ScrollArea, Ui};
use std::time::{Duration, Instant};

/// Acción ejecutada tras la purga del pánico cuando `panico.abortar_proceso` es `true`.
pub type AccionSalida = Box<dyn Fn()>;

/// Frecuencia mínima de repintado para comprobar la inactividad sin eventos.
const INTERVALO_REVISION: Duration = Duration::from_secs(1);
/// Ancho mínimo del panel de telemetría.
const ANCHO_PANEL_TELEMETRIA: f32 = 300.0;
/// Radio de las esquinas del área de contenido.
const RADIO_CONTENIDO: f32 = 4.0;
/// Separación superior del contenido.
const MARGEN_CONTENIDO: f32 = 12.0;

/// Ventana principal de WoDW.
pub struct VentanaPrincipal {
    estado: EstadoNavegador,
    catalogo: CatalogoMotores,
    sesion: Option<ConexionSesion>,
    cfg: ConfiguracionWodw,
    mensaje_estado: String,
    mostrar_telemetria: bool,
    cache_imagen: CacheImagen,
    accion_salida: AccionSalida,
}

impl VentanaPrincipal {
    /// Crea la ventana. Sin sesión, la interfaz funciona pero no navega.
    pub fn nueva(
        cfg: ConfiguracionWodw,
        sesion: Option<ConexionSesion>,
        accion_salida: AccionSalida,
    ) -> Self {
        let minutos = cfg.automatizacion.minutos_inactividad_purga;
        let parametros = ParametrosEstado {
            pulsaciones_panico: cfg.panico.pulsaciones,
            ventana_panico: Duration::from_millis(cfg.panico.ventana_ms),
            inactividad_purga: (minutos > 0).then(|| Duration::from_secs(minutos * 60)),
        };
        let mensaje_estado = if sesion.is_some() {
            String::new()
        } else {
            textos::SIN_SESION.to_string()
        };
        Self {
            estado: EstadoNavegador::nuevo(parametros, Instant::now()),
            catalogo: CatalogoMotores::desde_configuracion(&cfg.motores),
            sesion,
            cfg,
            mensaje_estado,
            mostrar_telemetria: false,
            cache_imagen: CacheImagen::default(),
            accion_salida,
        }
    }

    /// Estado del navegador (solo lectura).
    pub fn estado(&self) -> &EstadoNavegador {
        &self.estado
    }

    /// Mensaje de la barra de estado.
    pub fn mensaje_estado(&self) -> &str {
        &self.mensaje_estado
    }

    /// Navega a lo escrito en la barra (URL o búsqueda).
    pub fn ir_a(&mut self, entrada: &str) {
        if let Some(direccion) = self.catalogo.resolver_entrada(entrada) {
            let solicitud = self.estado.navegar(direccion);
            self.enviar(solicitud);
        }
    }

    fn enviar(&self, solicitud: SolicitudNavegacion) {
        if let Some(sesion) = &self.sesion {
            sesion.ordenar(OrdenSesion::Navegar {
                id_pestana: solicitud.id_pestana,
                solicitud: solicitud.solicitud,
                direccion: solicitud.direccion,
            });
        }
    }

    /// Pánico: purga todo, renueva circuitos, olvida bloqueos y sale si está configurado.
    pub fn activar_panico(&mut self) {
        self.purgar_sesion();
        if self.cfg.panico.abortar_proceso {
            (self.accion_salida)();
        }
    }

    fn purgar_sesion(&mut self) {
        self.estado.purgar_todo();
        self.cache_imagen.vaciar();
        if let Some(sesion) = &self.sesion {
            sesion.ordenar(OrdenSesion::Purgar);
        }
    }

    /// Aplica los eventos pendientes de la sesión.
    pub fn atender_eventos_sesion(&mut self) {
        let eventos: Vec<EventoSesion> = self
            .sesion
            .as_ref()
            .map(|s| std::iter::from_fn(|| s.siguiente_evento()).collect())
            .unwrap_or_default();
        for evento in eventos {
            self.atender_evento(evento);
        }
    }

    /// Aplica un evento de la sesión.
    pub fn atender_evento(&mut self, evento: EventoSesion) {
        match evento {
            EventoSesion::Tor(estado) => self.mensaje_estado = textos::estado_tor(&estado),
            EventoSesion::Navegacion {
                id_pestana,
                solicitud,
                resultado,
            } => {
                self.estado
                    .aplicar_resultado(id_pestana, solicitud, resultado);
            }
            EventoSesion::AislamientoRotado => {
                self.mensaje_estado = textos::AISLAMIENTO_ROTADO.to_string()
            }
            EventoSesion::PanicoAutomatico => self.activar_panico(),
        }
    }

    /// Purga por inactividad si ha vencido el plazo.
    pub fn comprobar_inactividad(&mut self, ahora: Instant) {
        if self.estado.inactividad_vencida(ahora) {
            self.purgar_sesion();
            self.mensaje_estado = textos::PURGA_INACTIVIDAD.to_string();
        }
    }

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

    fn resumen_ids(&self) -> ResumenTelemetria {
        self.sesion
            .as_ref()
            .map(|s| s.control_ids().obtener_resumen())
            .unwrap_or_default()
    }

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

    fn panel_central(&mut self, ui: &mut Ui) {
        let disponible = ui.available_rect_before_wrap();
        let caja = calcular_letterboxing(disponible, &self.cfg.interfaz);
        ui.painter()
            .rect_filled(disponible, 0.0, COLOR_FONDO_LETTERBOXING);
        let mut pulsado = None;
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
                    self.cfg.worker.frecuencia_corte_audio_hz,
                );
            });
        });
        if let Some(url) = pulsado {
            self.ir_a(&url);
        }
    }
}

impl eframe::App for VentanaPrincipal {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.dibujar(ui);
    }

    fn on_exit(&mut self) {
        self.purgar_sesion();
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
        let resumen = self.resumen_ids();
        egui::Panel::top("panel_superior").show(ui, |ui| {
            ui.horizontal(|ui| self.fila_pestanas(ui));
            ui.separator();
            ui.horizontal(|ui| self.fila_navegacion(ui, &resumen));
        });
        egui::Panel::bottom("panel_inferior").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(textos::pie_estado()).italics());
                ui.separator();
                ui.label(&self.mensaje_estado);
            });
        });
        if self.mostrar_telemetria {
            egui::Panel::right("panel_telemetria")
                .min_size(ANCHO_PANEL_TELEMETRIA)
                .show(ui, |ui| {
                    if panel_telemetria(ui, &resumen) {
                        if let Some(sesion) = &self.sesion {
                            sesion.ordenar(OrdenSesion::RotarAislamiento);
                        }
                    }
                });
        }
        egui::CentralPanel::default().show(ui, |ui| self.panel_central(ui));
        ctx.request_repaint_after(INTERVALO_REVISION);
    }
}
