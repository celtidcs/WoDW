//! Constantes de conversión de unidades, reunidas en un único sitio para que
//! ninguna cifra de conversión aparezca suelta por el código.

/// Milisegundos en un segundo.
pub const MILISEGUNDOS_POR_SEGUNDO: u64 = 1_000;
/// Segundos en un minuto.
pub const SEGUNDOS_POR_MINUTO: u64 = 60;
/// Segundos en una hora.
pub const SEGUNDOS_POR_HORA: u64 = 3_600;
/// Segundos en un día.
pub const SEGUNDOS_POR_DIA: u64 = 86_400;
/// Denominador de una proporción en tanto por ciento.
pub const POR_CIENTO: u32 = 100;
/// Denominador de una proporción en tanto por mil.
pub const POR_MIL: u16 = 1_000;
/// Bytes en un kibibyte.
pub const BYTES_POR_KIB: usize = 1_024;
/// Bytes en un mebibyte.
pub const BYTES_POR_MIB: usize = 1_024 * BYTES_POR_KIB;
/// Valor que corresponde a ±1,0 al normalizar una muestra PCM de 16 bits (2¹⁵).
pub const ESCALA_MUESTRA_I16: f32 = 32_768.0;
