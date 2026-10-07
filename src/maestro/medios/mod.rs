//! Medios en el Maestro: segunda validación de lo que entrega el Worker.
//!
//! El Worker que decodifica un medio puede haber sido engañado por el archivo,
//! así que el Maestro no se fía de sus bloques: comprueba que el audio tiene
//! la forma fija pactada, que no supera el pico máximo ni la duración pedida y
//! que cada fotograma cuadra con sus dimensiones y está dentro del límite de
//! resolución.

pub mod productor;
pub mod reproduccion;

pub use reproduccion::{EstadoReproduccion, FaseReproduccion, ManejadorReproduccion};

use crate::configuracion::ConfiguracionWorker;
use crate::error::{ErrorApp, Resultado};
use crate::ipc::mensajes::{
    rgba_cuadra, FotogramaVideo, RespuestaWorker, CANALES_SALIDA, FRECUENCIA_SALIDA_HZ,
};
use crate::unidades::MILISEGUNDOS_POR_SEGUNDO;

/// Factor de holgura sobre la duración de bloque pedida: el Worker cierra el
/// bloque tras el paquete que la alcanza, así que puede pasarse algo.
const HOLGURA_DURACION_BLOQUE: u64 = 2;

/// Valida un bloque del Worker.
///
/// # Errors
/// [`ErrorApp::ContenidoHostil`] si el bloque no tiene la forma pactada (es un
/// incidente: el Worker está comprometido o miente) y [`ErrorApp::Proceso`] si
/// la respuesta no es un bloque.
pub fn validar_bloque(respuesta: &RespuestaWorker, cfg: &ConfiguracionWorker) -> Resultado<()> {
    let RespuestaWorker::BloqueMedio {
        audio_pcm,
        fotogramas,
        ..
    } = respuesta
    else {
        return Err(ErrorApp::Proceso(
            "se esperaba un bloque de medio".to_string(),
        ));
    };
    validar_audio(audio_pcm, cfg)
        .and_then(|()| validar_fotogramas(fotogramas, cfg))
        .map_err(|motivo| ErrorApp::ContenidoHostil(format!("bloque de medio: {motivo}")))
}

/// Audio del bloque: estéreo intercalado, sin más muestras de las pedidas y
/// sin pasar del pico pactado.
fn validar_audio(audio_pcm: &[i16], cfg: &ConfiguracionWorker) -> Result<(), String> {
    if !audio_pcm.len().is_multiple_of(usize::from(CANALES_SALIDA)) {
        return Err("audio que no es estéreo intercalado".to_string());
    }
    let maximo_muestras = u64::from(FRECUENCIA_SALIDA_HZ)
        * u64::from(CANALES_SALIDA)
        * u64::from(cfg.duracion_bloque_ms)
        * HOLGURA_DURACION_BLOQUE
        / MILISEGUNDOS_POR_SEGUNDO;
    if audio_pcm.len() as u64 > maximo_muestras {
        return Err(format!(
            "{} muestras de audio, más de lo pedido",
            audio_pcm.len()
        ));
    }
    let pico = i32::from(cfg.pico_maximo_muestra());
    if audio_pcm.iter().any(|&m| i32::from(m).abs() > pico) {
        return Err("audio por encima del pico máximo".to_string());
    }
    Ok(())
}

/// Fotogramas del bloque: píxeles que cuadran con sus dimensiones, dentro del
/// límite de resolución y con marcas de tiempo que no retroceden.
fn validar_fotogramas(
    fotogramas: &[FotogramaVideo],
    cfg: &ConfiguracionWorker,
) -> Result<(), String> {
    let mut anterior = None;
    for f in fotogramas {
        if !rgba_cuadra(f.ancho, f.alto, f.rgba.len()) || !cfg.admite_resolucion(f.ancho, f.alto) {
            return Err(format!("fotograma de {}×{} incoherente", f.ancho, f.alto));
        }
        if anterior.is_some_and(|a| f.marca_ms < a) {
            return Err("marcas de tiempo que retroceden".to_string());
        }
        anterior = Some(f.marca_ms);
    }
    Ok(())
}
