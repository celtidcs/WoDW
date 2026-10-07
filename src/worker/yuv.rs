//! Conversión de fotogramas YUV planares a RGBA.
//!
//! Los decodificadores de vídeo (AV1, H.264) entregan luminancia y crominancia
//! en planos separados. Aquí se reconstruyen píxeles RGBA nuevos, que es lo
//! único que sale del Worker. Todo acceso a los planos está comprobado: unos
//! planos más cortos de lo que declaran sus dimensiones dan error, no pánico.

use crate::ipc::mensajes::BYTES_POR_PIXEL_RGBA;

/// (Kr, Kb) de la recomendación ITU-R BT.601 (definición estándar).
const COEFICIENTES_BT601: (f32, f32) = (0.299, 0.114);
/// (Kr, Kb) de la recomendación ITU-R BT.709 (alta definición).
const COEFICIENTES_BT709: (f32, f32) = (0.2126, 0.0722);

/// Submuestreo de la crominancia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Submuestreo {
    /// Solo luminancia (escala de grises).
    Monocromo,
    /// Crominancia a mitad de resolución en ambos ejes (4:2:0).
    Medio,
    /// Crominancia a mitad de resolución horizontal (4:2:2).
    MedioHorizontal,
    /// Crominancia a resolución completa (4:4:4).
    Completo,
}

impl Submuestreo {
    /// Divisores (horizontal, vertical) de la crominancia.
    fn divisores(self) -> (usize, usize) {
        match self {
            Self::Monocromo | Self::Completo => (1, 1),
            Self::Medio => (2, 2),
            Self::MedioHorizontal => (2, 1),
        }
    }
}

/// Matriz de conversión YUV→RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matriz {
    /// ITU-R BT.601 (vídeo de definición estándar y valor por defecto).
    Bt601,
    /// ITU-R BT.709 (alta definición).
    Bt709,
}

impl Matriz {
    /// Coeficientes (Kr, Kb) de la matriz.
    fn coeficientes(self) -> (f32, f32) {
        match self {
            Self::Bt601 => COEFICIENTES_BT601,
            Self::Bt709 => COEFICIENTES_BT709,
        }
    }
}

/// Un plano: sus bytes y la distancia en bytes entre filas.
#[derive(Debug, Clone, Copy)]
pub struct Plano<'a> {
    /// Muestras de 8 bits.
    pub datos: &'a [u8],
    /// Bytes por fila (puede ser mayor que el ancho por relleno).
    pub paso: usize,
}

/// Fotograma YUV de 8 bits.
#[derive(Debug, Clone, Copy)]
pub struct FotogramaYuv<'a> {
    /// Ancho en píxeles.
    pub ancho: usize,
    /// Alto en píxeles.
    pub alto: usize,
    /// Plano de luminancia.
    pub y: Plano<'a>,
    /// Plano de crominancia azul (ignorado en monocromo).
    pub u: Plano<'a>,
    /// Plano de crominancia roja (ignorado en monocromo).
    pub v: Plano<'a>,
    /// Submuestreo de la crominancia.
    pub submuestreo: Submuestreo,
    /// Matriz de color.
    pub matriz: Matriz,
    /// `true` si las muestras usan el rango completo 0–255; `false` para el
    /// rango limitado de vídeo (16–235 luminancia, 16–240 crominancia).
    pub rango_completo: bool,
}

/// Escala del rango limitado de luminancia (219 niveles en 8 bits).
const ESCALA_LUMA_LIMITADA: f32 = 255.0 / 219.0;
/// Escala del rango limitado de crominancia (224 niveles en 8 bits).
const ESCALA_CROMA_LIMITADA: f32 = 255.0 / 224.0;
/// Negro del rango limitado.
const NEGRO_LIMITADO: f32 = 16.0;
/// Valor neutro de la crominancia.
const CROMA_NEUTRA: f32 = 128.0;
/// Alfa opaco.
const OPACO: u8 = 255;

/// Convierte el fotograma a RGBA opaco.
///
/// # Errors
/// Devuelve una descripción si algún plano es más corto que lo que exigen las
/// dimensiones o el paso, o si las dimensiones desbordan.
pub fn a_rgba(f: &FotogramaYuv<'_>) -> Result<Vec<u8>, String> {
    let (dx, dy) = f.submuestreo.divisores();
    let mut rgba = Vec::with_capacity(
        f.ancho
            .checked_mul(f.alto)
            .and_then(|p| p.checked_mul(BYTES_POR_PIXEL_RGBA))
            .ok_or("dimensiones desbordadas")?,
    );
    for fila in 0..f.alto {
        for columna in 0..f.ancho {
            let y = muestra(&f.y, fila, columna)?;
            let (u, v) = if f.submuestreo == Submuestreo::Monocromo {
                (CROMA_NEUTRA as u8, CROMA_NEUTRA as u8)
            } else {
                (
                    muestra(&f.u, fila / dy, columna / dx)?,
                    muestra(&f.v, fila / dy, columna / dx)?,
                )
            };
            let [r, g, b] = convertir(y, u, v, f.matriz, f.rango_completo);
            rgba.extend_from_slice(&[r, g, b, OPACO]);
        }
    }
    Ok(rgba)
}

/// Lee una muestra comprobando los límites.
fn muestra(plano: &Plano<'_>, fila: usize, columna: usize) -> Result<u8, String> {
    fila.checked_mul(plano.paso)
        .and_then(|inicio| inicio.checked_add(columna))
        .and_then(|i| plano.datos.get(i).copied())
        .ok_or_else(|| "plano YUV más corto que sus dimensiones".to_string())
}

/// Convierte una muestra YUV a RGB.
fn convertir(y: u8, u: u8, v: u8, matriz: Matriz, rango_completo: bool) -> [u8; 3] {
    let (kr, kb) = matriz.coeficientes();
    let kg = 1.0 - kr - kb;
    let (y, u, v) = (
        f32::from(y),
        f32::from(u) - CROMA_NEUTRA,
        f32::from(v) - CROMA_NEUTRA,
    );
    let (y, u, v) = if rango_completo {
        (y, u, v)
    } else {
        (
            (y - NEGRO_LIMITADO) * ESCALA_LUMA_LIMITADA,
            u * ESCALA_CROMA_LIMITADA,
            v * ESCALA_CROMA_LIMITADA,
        )
    };
    let r = y + 2.0 * (1.0 - kr) * v;
    let b = y + 2.0 * (1.0 - kb) * u;
    let g = (y - kr * r - kb * b) / kg;
    [saturar(r), saturar(g), saturar(b)]
}

/// Redondea y satura a 0–255.
fn saturar(x: f32) -> u8 {
    x.round().clamp(0.0, f32::from(u8::MAX)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fotograma<'a>(y: &'a [u8], u: &'a [u8], v: &'a [u8], completo: bool) -> FotogramaYuv<'a> {
        FotogramaYuv {
            ancho: 2,
            alto: 2,
            y: Plano { datos: y, paso: 2 },
            u: Plano { datos: u, paso: 1 },
            v: Plano { datos: v, paso: 1 },
            submuestreo: Submuestreo::Medio,
            matriz: Matriz::Bt601,
            rango_completo: completo,
        }
    }

    #[test]
    fn negro_blanco_y_rojo_en_rango_limitado() {
        let negro = a_rgba(&fotograma(&[16; 4], &[128], &[128], false)).unwrap();
        assert_eq!(&negro[..4], &[0, 0, 0, 255]);
        let blanco = a_rgba(&fotograma(&[235; 4], &[128], &[128], false)).unwrap();
        assert_eq!(&blanco[..4], &[255, 255, 255, 255]);
        // Rojo puro BT.601 limitado: Y=81, U=90, V=240.
        let rojo = a_rgba(&fotograma(&[81; 4], &[90], &[240], false)).unwrap();
        assert!(
            rojo[0] > 250 && rojo[1] < 5 && rojo[2] < 5,
            "{:?}",
            &rojo[..4]
        );
    }

    #[test]
    fn plano_corto_es_error_no_panico() {
        assert!(a_rgba(&fotograma(&[16; 3], &[128], &[128], true)).is_err());
        assert!(a_rgba(&fotograma(&[16; 4], &[], &[128], true)).is_err());
    }
}
