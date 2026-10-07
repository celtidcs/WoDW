//! Decodificación AV1 con `re_rav1d` (traducción a Rust de dav1d), sin
//! ensamblador: más lenta, pero todo el decodificador es código Rust.
//!
//! La usan las imágenes AVIF y el vídeo WebM. Cada fotograma sale como píxeles
//! RGBA reconstruidos ([`super::yuv`]); nada más del flujo original sale de aquí.

use super::yuv::{self, FotogramaYuv, Matriz, Plano, Submuestreo};
use re_rav1d::dav1d::pixel::{MatrixCoefficients, YUVRange};
use re_rav1d::dav1d::{Decoder, Picture, PixelLayout, PlanarImageComponent, Settings};

/// Profundidad de color admitida: la compilación solo incluye 8 bits.
const BITS_ADMITIDOS: usize = 8;
/// Un único hilo de decodificación: el Worker procesa un solo recurso y así el
/// comportamiento es determinista.
const HILOS_DECODIFICACION: u32 = 1;

/// Fotograma AV1 decodificado a RGBA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FotogramaRgba {
    /// Ancho en píxeles.
    pub ancho: u32,
    /// Alto en píxeles.
    pub alto: u32,
    /// Marca de tiempo del paquete de origen, si la tenía.
    pub marca_tiempo: Option<i64>,
    /// Píxeles RGBA.
    pub rgba: Vec<u8>,
}

/// Decodificador AV1 con límite de tamaño de fotograma.
pub struct DecodificadorAv1 {
    decodificador: Decoder,
}

impl DecodificadorAv1 {
    /// Crea un decodificador que rechaza fotogramas de más de `max_pixeles`.
    ///
    /// # Errors
    /// Descripción del fallo de inicialización.
    pub fn nuevo(max_pixeles: u32) -> Result<Self, String> {
        let mut ajustes = Settings::new();
        ajustes.set_n_threads(HILOS_DECODIFICACION);
        ajustes.set_max_frame_delay(1);
        ajustes.set_frame_size_limit(max_pixeles);
        ajustes.set_apply_grain(false);
        Decoder::with_settings(&ajustes)
            .map(|decodificador| Self { decodificador })
            .map_err(|e| format!("decodificador AV1: {e}"))
    }

    /// Entrega un paquete (unidad temporal de OBU) y devuelve los fotogramas
    /// que queden listos.
    ///
    /// # Errors
    /// Descripción del fallo si el flujo es inválido o un fotograma no se
    /// puede convertir.
    pub fn decodificar(
        &mut self,
        paquete: Vec<u8>,
        marca_tiempo: Option<i64>,
    ) -> Result<Vec<FotogramaRgba>, String> {
        let mut listos = Vec::new();
        let mut envio = self
            .decodificador
            .send_data(paquete, None, marca_tiempo, None);
        loop {
            match envio {
                Ok(()) => break,
                Err(e) if e.is_again() => {
                    self.recoger(&mut listos)?;
                    envio = self.decodificador.send_pending_data();
                }
                Err(e) => return Err(format!("flujo AV1 inválido: {e}")),
            }
        }
        self.recoger(&mut listos)?;
        Ok(listos)
    }

    /// Vacía los fotogramas retenidos al final del flujo.
    ///
    /// # Errors
    /// Como [`Self::decodificar`].
    pub fn terminar(&mut self) -> Result<Vec<FotogramaRgba>, String> {
        let mut listos = Vec::new();
        loop {
            match self.decodificador.get_picture() {
                Ok(imagen) => listos.push(convertir(&imagen)?),
                Err(e) if e.is_again() => return Ok(listos),
                Err(e) => return Err(format!("flujo AV1 inválido: {e}")),
            }
        }
    }

    /// Recoge los fotogramas disponibles sin bloquear.
    fn recoger(&mut self, listos: &mut Vec<FotogramaRgba>) -> Result<(), String> {
        loop {
            match self.decodificador.get_picture() {
                Ok(imagen) => listos.push(convertir(&imagen)?),
                Err(e) if e.is_again() => return Ok(()),
                Err(e) => return Err(format!("flujo AV1 inválido: {e}")),
            }
        }
    }
}

/// Convierte un fotograma decodificado a RGBA.
fn convertir(imagen: &Picture) -> Result<FotogramaRgba, String> {
    if imagen.bit_depth() != BITS_ADMITIDOS {
        return Err(format!(
            "AV1 de {} bits no soportado (solo {BITS_ADMITIDOS})",
            imagen.bit_depth()
        ));
    }
    let (y, u, v) = (
        imagen.plane(PlanarImageComponent::Y),
        imagen.plane(PlanarImageComponent::U),
        imagen.plane(PlanarImageComponent::V),
    );
    fn plano<'a>(imagen: &Picture, datos: &'a [u8], componente: PlanarImageComponent) -> Plano<'a> {
        Plano {
            datos,
            paso: imagen.stride(componente) as usize,
        }
    }
    let rgba = yuv::a_rgba(&FotogramaYuv {
        ancho: imagen.width() as usize,
        alto: imagen.height() as usize,
        y: plano(imagen, &y, PlanarImageComponent::Y),
        u: plano(imagen, &u, PlanarImageComponent::U),
        v: plano(imagen, &v, PlanarImageComponent::V),
        submuestreo: match imagen.pixel_layout() {
            PixelLayout::I400 => Submuestreo::Monocromo,
            PixelLayout::I420 => Submuestreo::Medio,
            PixelLayout::I422 => Submuestreo::MedioHorizontal,
            PixelLayout::I444 => Submuestreo::Completo,
        },
        matriz: match imagen.matrix_coefficients() {
            MatrixCoefficients::BT709 => Matriz::Bt709,
            _ => Matriz::Bt601,
        },
        rango_completo: imagen.color_range() == YUVRange::Full,
    })?;
    Ok(FotogramaRgba {
        ancho: imagen.width(),
        alto: imagen.height(),
        marca_tiempo: imagen.timestamp(),
        rgba,
    })
}
