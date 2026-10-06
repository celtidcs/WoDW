//! Presentación del contenido sanitizado de la pestaña activa.

use crate::maestro::navegacion::ContenidoPagina;
use crate::ui::estado::{EstadoContenido, Pestana};
use crate::ui::motores::CatalogoMotores;
use crate::ui::textos;
use egui::{RichText, TextureHandle, Ui};

/// Separación vertical entre bloques.
const ESPACIO_BLOQUE: f32 = 8.0;

/// Caché de la textura de la imagen mostrada (se regenera al cambiar de imagen).
#[derive(Default)]
pub struct CacheImagen {
    clave: Option<(u64, usize)>,
    textura: Option<TextureHandle>,
}

impl CacheImagen {
    /// Suelta la textura (purga).
    pub fn vaciar(&mut self) {
        self.clave = None;
        self.textura = None;
    }
}

/// Dibuja la pestaña; devuelve la URL de un enlace pulsado, si alguno.
pub fn mostrar_pestana(
    ui: &mut Ui,
    pestana: &Pestana,
    catalogo: &CatalogoMotores,
    cache: &mut CacheImagen,
    corte_audio: u32,
) -> Option<String> {
    match pestana.contenido() {
        EstadoContenido::Bienvenida => {
            bienvenida(ui, catalogo);
            None
        }
        EstadoContenido::Cargando => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(textos::CARGANDO);
            });
            None
        }
        EstadoContenido::Error(mensaje) => {
            ui.colored_label(egui::Color32::RED, mensaje);
            None
        }
        EstadoContenido::Pagina(contenido) => {
            pagina(ui, pestana.id(), contenido, cache, corte_audio)
        }
    }
}

fn bienvenida(ui: &mut Ui, catalogo: &CatalogoMotores) {
    ui.heading(textos::NOMBRE_PRODUCTO);
    ui.label(textos::BIENVENIDA);
    ui.add_space(ESPACIO_BLOQUE);
    ui.label(RichText::new(textos::MOTORES_DISPONIBLES).strong());
    for motor in catalogo.motores() {
        ui.label(format!("• {}: {}", motor.nombre, motor.descripcion));
    }
}

fn pagina(
    ui: &mut Ui,
    id_pestana: u64,
    contenido: &ContenidoPagina,
    cache: &mut CacheImagen,
    corte_audio: u32,
) -> Option<String> {
    match contenido {
        ContenidoPagina::Documento {
            titulo,
            texto,
            enlaces,
        } => {
            ui.heading(titulo);
            ui.separator();
            ui.label(texto);
            if enlaces.is_empty() {
                return None;
            }
            ui.add_space(ESPACIO_BLOQUE);
            ui.label(RichText::new(textos::ENLACES).strong());
            let mut pulsado = None;
            for enlace in enlaces {
                if ui.link(&enlace.texto).on_hover_text(&enlace.url).clicked() {
                    pulsado = Some(enlace.url.clone());
                }
            }
            pulsado
        }
        ContenidoPagina::Imagen { ancho, alto, rgba } => {
            ui.label(textos::imagen(*ancho, *alto));
            mostrar_imagen(ui, (id_pestana, rgba.len()), *ancho, *alto, rgba, cache);
            None
        }
        ContenidoPagina::Audio {
            frecuencia_muestreo,
            canales,
            muestras_por_canal,
        } => {
            ui.label(textos::audio(
                *frecuencia_muestreo,
                *canales,
                *muestras_por_canal,
                corte_audio,
            ));
            None
        }
        ContenidoPagina::NoSoportado { tipo_mime } => {
            ui.label(textos::no_soportado(tipo_mime));
            None
        }
    }
}

fn mostrar_imagen(
    ui: &mut Ui,
    clave: (u64, usize),
    ancho: u32,
    alto: u32,
    rgba: &[u8],
    cache: &mut CacheImagen,
) {
    if cache.clave != Some(clave) {
        let tamano = [ancho as usize, alto as usize];
        let imagen = egui::ColorImage::from_rgba_unmultiplied(tamano, rgba);
        cache.textura = Some(
            ui.ctx()
                .load_texture("imagen_pestana", imagen, Default::default()),
        );
        cache.clave = Some(clave);
    }
    if let Some(textura) = &cache.textura {
        ui.add(egui::Image::new(textura).shrink_to_fit());
    }
}
