//! Presentación del contenido sanitizado de la pestaña activa.

use crate::ipc::mensajes::MedioEnlazado;
use crate::maestro::navegacion::{ContenidoPagina, EnlaceRevisado};
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
        EstadoContenido::Pagina(contenido) => pagina(ui, pestana.id(), contenido, cache),
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

/// Medios incrustados en la página; devuelve la dirección del que se pulse.
fn lista_de_medios(ui: &mut Ui, medios: &[MedioEnlazado]) -> Option<String> {
    if medios.is_empty() {
        return None;
    }
    ui.add_space(ESPACIO_BLOQUE);
    ui.label(RichText::new(textos::MEDIOS_DE_LA_PAGINA).strong());
    let mut pulsado = None;
    for medio in medios {
        let etiqueta = textos::medio_enlazado(medio);
        if ui.link(etiqueta).on_hover_text(&medio.url).clicked() {
            pulsado = Some(medio.url.clone());
        }
    }
    pulsado
}

/// Enlaces de la página; devuelve el destino del que se pulse.
fn lista_de_enlaces(ui: &mut Ui, enlaces: &[EnlaceRevisado]) -> Option<String> {
    if enlaces.is_empty() {
        return None;
    }
    ui.add_space(ESPACIO_BLOQUE);
    ui.label(RichText::new(textos::ENLACES).strong());
    let mut pulsado = None;
    for enlace in enlaces {
        pulsado = mostrar_enlace(ui, enlace).or(pulsado);
    }
    pulsado
}

/// Contenido de una pestaña con página; devuelve la dirección pulsada.
fn pagina(
    ui: &mut Ui,
    id_pestana: u64,
    contenido: &ContenidoPagina,
    cache: &mut CacheImagen,
) -> Option<String> {
    match contenido {
        ContenidoPagina::Documento {
            titulo,
            texto,
            enlaces,
            medios,
            recortado,
        } => {
            ui.heading(titulo);
            ui.separator();
            ui.label(texto);
            if *recortado {
                ui.label(RichText::new(textos::TEXTO_RECORTADO).italics());
            }
            let medio = lista_de_medios(ui, medios);
            lista_de_enlaces(ui, enlaces).or(medio)
        }
        ContenidoPagina::Imagen { ancho, alto, rgba } => {
            ui.label(textos::imagen(*ancho, *alto));
            mostrar_imagen(ui, (id_pestana, rgba.len()), *ancho, *alto, rgba, cache);
            None
        }
        ContenidoPagina::Medio {
            familia,
            tipo_mime,
            datos,
        } => {
            ui.label(textos::medio(*familia, tipo_mime, datos.len()));
            None
        }
        ContenidoPagina::NoSoportado { tipo_mime } => {
            ui.label(textos::no_soportado(tipo_mime));
            None
        }
    }
}

/// Dibuja un enlace con su aviso; devuelve su URL si se pulsa.
fn mostrar_enlace(ui: &mut Ui, enlace: &EnlaceRevisado) -> Option<String> {
    ui.horizontal_wrapped(|ui| {
        let pulsado = ui.link(&enlace.texto).on_hover_text(&enlace.url).clicked();
        if let Some(aviso) = &enlace.aviso {
            ui.colored_label(textos::COLOR_AVISO_ENLACE, textos::aviso_enlace(aviso));
        }
        pulsado.then(|| enlace.url.clone())
    })
    .inner
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
