//! Imágenes AVIF: contenedor leído con `avif-parse` y fotograma decodificado
//! con el decodificador AV1 en Rust ([`super::av1`]).
//!
//! La resolución se comprueba en la cabecera de la secuencia AV1 antes de
//! decodificar. La transparencia (fotograma alfa aparte) se descarta: la
//! imagen se entrega opaca.

use super::av1::DecodificadorAv1;
use super::imagen::{ImagenRgba, RechazoImagen};
use crate::configuracion::ConfiguracionWorker;
use std::io::Cursor;

/// Profundidad admitida (la compilación del decodificador solo incluye 8 bits).
const BITS_ADMITIDOS: u8 = 8;

/// Decodifica un AVIF a RGBA.
///
/// # Errors
/// [`RechazoImagen::ExcedeLimites`] si la cabecera AV1 supera el límite de
/// resolución y [`RechazoImagen::Invalida`] en cualquier otro fallo.
pub fn decodificar_avif(
    datos: &[u8],
    cfg: &ConfiguracionWorker,
) -> Result<ImagenRgba, RechazoImagen> {
    let invalida = |e: &dyn std::fmt::Display| RechazoImagen::Invalida(format!("AVIF: {e}"));
    let avif = avif_parse::read_avif(&mut Cursor::new(datos)).map_err(|e| invalida(&e))?;
    let meta = avif.primary_item_metadata().map_err(|e| invalida(&e))?;
    let (ancho, alto) = (meta.max_frame_width.get(), meta.max_frame_height.get());
    if !cfg.admite_resolucion(ancho, alto) {
        return Err(RechazoImagen::ExcedeLimites(format!(
            "AVIF de {ancho}×{alto} supera el límite de {}×{}",
            cfg.lado_largo_maximo_px, cfg.lado_corto_maximo_px
        )));
    }
    if meta.bit_depth != BITS_ADMITIDOS {
        return Err(invalida(&format!("{} bits no soportado", meta.bit_depth)));
    }
    let mut decodificador = DecodificadorAv1::nuevo(
        cfg.lado_largo_maximo_px
            .saturating_mul(cfg.lado_corto_maximo_px),
    )
    .map_err(|e| invalida(&e))?;
    let mut fotogramas = decodificador
        .decodificar(avif.primary_item.to_vec(), None)
        .map_err(|e| invalida(&e))?;
    fotogramas.extend(decodificador.terminar().map_err(|e| invalida(&e))?);
    let fotograma = fotogramas
        .into_iter()
        .next()
        .ok_or_else(|| invalida(&"sin fotograma"))?;
    if !cfg.admite_resolucion(fotograma.ancho, fotograma.alto) {
        return Err(RechazoImagen::ExcedeLimites(
            "el fotograma AVIF no coincide con su cabecera".to_string(),
        ));
    }
    Ok(ImagenRgba {
        ancho: fotograma.ancho,
        alto: fotograma.alto,
        rgba: fotograma.rgba,
    })
}
