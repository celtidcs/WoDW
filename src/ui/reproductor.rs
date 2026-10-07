//! Reproductor de la interfaz (CA-A4): nada suena solo; se reproduce al
//! pulsar «Reproducir» y se detiene al cambiar o cerrar la pestaña, al navegar
//! a otra cosa, en la purga por inactividad y con el pánico.
//!
//! Solo hay una reproducción a la vez. Su estado compartido lo llena la sesión
//! (que pide bloques a un sub-Worker sin red) y lo vacían la salida de audio y
//! el dibujo de fotogramas.

use crate::maestro::medios::reproduccion::VOLUMEN_MAXIMO_POR_CIENTO;
use crate::maestro::medios::{EstadoReproduccion, FaseReproduccion, ManejadorReproduccion};
use crate::ui::salida_audio::SalidaAudio;
use crate::ui::textos;
use egui::{TextureHandle, Ui};
use std::time::Instant;

/// Reproducción en curso.
struct Activa {
    id_pestana: u64,
    solicitud: u64,
    manejador: ManejadorReproduccion,
    salida: Option<SalidaAudio>,
    textura: Option<TextureHandle>,
    ultimo_avance: Instant,
}

/// Acción pedida desde los controles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccionReproductor {
    /// Empezar a reproducir el medio de la pestaña.
    Reproducir,
    /// Detener la reproducción.
    Parar,
}

/// Reproductor de la interfaz.
#[derive(Default)]
pub struct Reproductor {
    activa: Option<Activa>,
}

impl Reproductor {
    /// Empieza una reproducción para la pestaña (detiene la anterior) y
    /// devuelve su estado compartido. Si no hay dispositivo de audio, el reloj
    /// avanza con el tiempo real.
    pub fn iniciar(
        &mut self,
        id_pestana: u64,
        solicitud: u64,
        volumen: u32,
        con_sonido: bool,
    ) -> ManejadorReproduccion {
        self.parar();
        let estado = EstadoReproduccion::nuevo(volumen);
        let salida = if con_sonido {
            SalidaAudio::abrir(estado.clone())
                .map_err(|e| tracing::warn!(error = %e, "reproducción sin sonido"))
                .ok()
        } else {
            None
        };
        let manejador = ManejadorReproduccion(estado);
        self.activa = Some(Activa {
            id_pestana,
            solicitud,
            manejador: manejador.clone(),
            salida,
            textura: None,
            ultimo_avance: Instant::now(),
        });
        manejador
    }

    /// Detiene y suelta la reproducción (también el Worker, que deja de recibir
    /// peticiones y se destruye).
    pub fn parar(&mut self) {
        if let Some(activa) = self.activa.take() {
            activa.manejador.0.parar();
        }
    }

    /// Mantiene la reproducción solo si sigue siendo la de esa pestaña y esa
    /// solicitud con un medio a la vista; si no, la detiene.
    pub fn vigilar(&mut self, id_pestana: u64, solicitud: u64, muestra_medio: bool) {
        let sigue = self.activa.as_ref().is_some_and(|a| {
            a.id_pestana == id_pestana && a.solicitud == solicitud && muestra_medio
        });
        if !sigue {
            self.parar();
        }
    }

    /// Reproducción en curso, si la hay.
    pub fn activa(&self) -> Option<&ManejadorReproduccion> {
        self.activa.as_ref().map(|a| &a.manejador)
    }

    /// Avanza el reloj sin dispositivo de audio y actualiza el fotograma.
    pub fn avanzar(&mut self, ctx: &egui::Context, ahora: Instant) {
        let Some(activa) = self.activa.as_mut() else {
            return;
        };
        let estado = &activa.manejador.0;
        if activa.salida.is_none() {
            let ms = ahora.duration_since(activa.ultimo_avance).as_millis();
            estado.avanzar_sin_dispositivo(u64::try_from(ms).unwrap_or(u64::MAX));
        }
        activa.ultimo_avance = ahora;
        if let Some(f) = estado.tomar_fotograma_vigente() {
            let imagen = egui::ColorImage::from_rgba_unmultiplied(
                [f.ancho as usize, f.alto as usize],
                &f.rgba,
            );
            activa.textura = Some(ctx.load_texture("fotograma_video", imagen, Default::default()));
        }
        if estado.agotada() && estado.fase() == FaseReproduccion::Reproduciendo {
            estado.fijar_fase(FaseReproduccion::Terminada);
        }
        if !estado.parado() && estado.fase() != FaseReproduccion::Terminada {
            ctx.request_repaint();
        }
    }

    /// Dibuja los controles (y el vídeo, si lo hay) de la pestaña indicada.
    pub fn controles(
        &mut self,
        ui: &mut Ui,
        id_pestana: u64,
        solicitud: u64,
    ) -> Option<AccionReproductor> {
        let Some(activa) = self
            .activa
            .as_mut()
            .filter(|a| a.id_pestana == id_pestana && a.solicitud == solicitud)
        else {
            return ui
                .button(textos::BOTON_REPRODUCIR)
                .clicked()
                .then_some(AccionReproductor::Reproducir);
        };
        let estado = &activa.manejador.0;
        let mut accion = None;
        ui.horizontal(|ui| {
            let (texto, pausa) = if estado.en_pausa() {
                (textos::BOTON_REANUDAR, false)
            } else {
                (textos::BOTON_PAUSA, true)
            };
            if ui.button(texto).clicked() {
                estado.pausar(pausa);
            }
            if ui.button(textos::BOTON_PARAR).clicked() {
                accion = Some(AccionReproductor::Parar);
            }
            let mut volumen = estado.volumen();
            if ui
                .add(
                    egui::Slider::new(&mut volumen, 0..=VOLUMEN_MAXIMO_POR_CIENTO)
                        .text(textos::ETIQUETA_VOLUMEN),
                )
                .changed()
            {
                estado.fijar_volumen(volumen);
            }
            ui.label(textos::estado_reproduccion(
                &estado.fase(),
                estado.reloj_ms(),
                activa.salida.is_some(),
            ));
        });
        if let Some(textura) = &activa.textura {
            ui.add(egui::Image::new(textura).shrink_to_fit());
        }
        accion
    }
}
