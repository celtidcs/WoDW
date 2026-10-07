//! Decodificación de imágenes a RGBA puro con límites previos a la reserva de memoria.
//!
//! El resultado son solo píxeles: EXIF, perfiles ICC y cualquier fragmento
//! auxiliar se pierden al no reconstruirse el contenedor original. Una imagen
//! por encima del límite de resolución se rechaza leyendo solo su cabecera.

use crate::configuracion::ConfiguracionWorker;
use crate::ipc::mensajes::FormatoImagen;
use image::{ImageFormat, ImageReader, Limits};
use std::io::Cursor;

/// Imagen decodificada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagenRgba {
    /// Ancho en píxeles.
    pub ancho: u32,
    /// Alto en píxeles.
    pub alto: u32,
    /// Píxeles RGBA, 4 bytes por píxel, por filas.
    pub rgba: Vec<u8>,
}

/// Motivo por el que una imagen no se entrega.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RechazoImagen {
    /// Datos vacíos, corruptos o de otro formato que el declarado.
    Invalida(String),
    /// Dimensiones o memoria por encima de los límites: posible bomba de descompresión.
    ExcedeLimites(String),
}

/// Decodifica `datos` exigiendo que sean del `formato` declarado.
///
/// # Errors
/// [`RechazoImagen::ExcedeLimites`] si la cabecera anuncia dimensiones o una
/// reserva por encima de la configuración (se comprueba antes de decodificar) y
/// [`RechazoImagen::Invalida`] en cualquier otro fallo.
pub fn decodificar_imagen(
    formato: FormatoImagen,
    datos: &[u8],
    cfg: &ConfiguracionWorker,
) -> Result<ImagenRgba, RechazoImagen> {
    comprobar_firma(formato, datos)?;
    if formato == FormatoImagen::Avif {
        return super::avif::decodificar_avif(datos, cfg);
    }
    let (ancho, alto) = ImageReader::with_format(Cursor::new(datos), formato_image(formato))
        .into_dimensions()
        .map_err(|e| RechazoImagen::Invalida(e.to_string()))?;
    if !cfg.admite_resolucion(ancho, alto) {
        return Err(RechazoImagen::ExcedeLimites(format!(
            "{ancho}×{alto} supera el límite de {}×{}",
            cfg.lado_largo_maximo_px, cfg.lado_corto_maximo_px
        )));
    }
    let mut lector = ImageReader::with_format(Cursor::new(datos), formato_image(formato));
    lector.limits(limites(cfg));
    let imagen = lector.decode().map_err(|e| match e {
        image::ImageError::Limits(detalle) => RechazoImagen::ExcedeLimites(detalle.to_string()),
        otro => RechazoImagen::Invalida(otro.to_string()),
    })?;
    // `decode` solo lee el primer fotograma: un GIF animado se queda quieto.
    let rgba = imagen.into_rgba8();
    Ok(ImagenRgba {
        ancho: rgba.width(),
        alto: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

/// `true` si la firma del contenido corresponde al formato declarado.
pub fn firma_coincide(formato: FormatoImagen, datos: &[u8]) -> bool {
    comprobar_firma(formato, datos).is_ok()
}

/// Exige que el contenido real (su firma) sea el formato declarado: un archivo
/// disfrazado o políglota se rechaza antes de llegar a ningún decodificador.
fn comprobar_firma(formato: FormatoImagen, datos: &[u8]) -> Result<(), RechazoImagen> {
    if datos.is_empty() {
        return Err(RechazoImagen::Invalida(
            "datos de imagen vacíos".to_string(),
        ));
    }
    match image::guess_format(datos) {
        Ok(real) if real == formato_image(formato) => Ok(()),
        Ok(real) => Err(RechazoImagen::Invalida(format!(
            "declarada como {formato:?} pero el contenido es {real:?}"
        ))),
        Err(_) => Err(RechazoImagen::Invalida(format!(
            "el contenido no tiene la firma de {formato:?}"
        ))),
    }
}

/// Límites de decodificación derivados de la configuración.
fn limites(cfg: &ConfiguracionWorker) -> Limits {
    let mut limites = Limits::default();
    // Segunda barrera, por si un decodificador descubriera otras dimensiones.
    limites.max_image_width = Some(cfg.lado_largo_maximo_px);
    limites.max_image_height = Some(cfg.lado_largo_maximo_px);
    limites.max_alloc = Some(cfg.memoria_maxima_imagen_bytes);
    limites
}

/// Equivalencia con los formatos de la biblioteca `image`.
fn formato_image(formato: FormatoImagen) -> ImageFormat {
    match formato {
        FormatoImagen::Png => ImageFormat::Png,
        FormatoImagen::Jpeg => ImageFormat::Jpeg,
        FormatoImagen::Gif => ImageFormat::Gif,
        FormatoImagen::Webp => ImageFormat::WebP,
        FormatoImagen::Bmp => ImageFormat::Bmp,
        FormatoImagen::Tiff => ImageFormat::Tiff,
        FormatoImagen::Ico => ImageFormat::Ico,
        FormatoImagen::Qoi => ImageFormat::Qoi,
        FormatoImagen::Avif => ImageFormat::Avif,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    /// PNG de 2×3 con un píxel distinto en cada posición.
    fn png_de_prueba() -> Vec<u8> {
        let img = ImageBuffer::from_fn(2, 3, |x, y| Rgba([x as u8 * 100, y as u8 * 50, 7, 255]));
        let mut salida = Cursor::new(Vec::new());
        img.write_to(&mut salida, ImageFormat::Png).unwrap();
        salida.into_inner()
    }

    #[test]
    fn png_real_se_decodifica_con_sus_pixeles() {
        let img = decodificar_imagen(
            FormatoImagen::Png,
            &png_de_prueba(),
            &ConfiguracionWorker::default(),
        )
        .unwrap();
        assert_eq!((img.ancho, img.alto), (2, 3));
        assert_eq!(&img.rgba[..8], &[0, 0, 7, 255, 100, 0, 7, 255]);
        assert_eq!(&img.rgba[20..24], &[100, 100, 7, 255]);
    }

    #[test]
    fn basura_y_formato_falso_se_rechazan() {
        let cfg = ConfiguracionWorker::default();
        assert!(matches!(
            decodificar_imagen(FormatoImagen::Png, &[1, 2, 3], &cfg),
            Err(RechazoImagen::Invalida(_))
        ));
        assert!(matches!(
            decodificar_imagen(FormatoImagen::Jpeg, &png_de_prueba(), &cfg),
            Err(RechazoImagen::Invalida(_))
        ));
    }

    #[test]
    fn dimensiones_sobre_el_limite_se_rechazan_sin_decodificar() {
        let cfg = ConfiguracionWorker {
            lado_largo_maximo_px: 1,
            lado_corto_maximo_px: 1,
            ..Default::default()
        };
        assert!(matches!(
            decodificar_imagen(FormatoImagen::Png, &png_de_prueba(), &cfg),
            Err(RechazoImagen::ExcedeLimites(_))
        ));
    }
}
