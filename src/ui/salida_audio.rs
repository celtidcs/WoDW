//! Salida de audio por el dispositivo predeterminado (`cpal`).
//!
//! Solo lee muestras ya validadas del estado compartido de la reproducción; no
//! interpreta nada que venga de la red. Vive en el hilo de la interfaz porque
//! el flujo de algunos sistemas no puede cambiar de hilo.

use crate::maestro::medios::EstadoReproduccion;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::Arc;

/// Flujo de salida abierto; el sonido se detiene al soltarlo.
pub struct SalidaAudio {
    _flujo: cpal::Stream,
}

impl SalidaAudio {
    /// Abre el dispositivo predeterminado en su formato por defecto y lo
    /// alimenta desde `estado`.
    ///
    /// # Errors
    /// Descripción si no hay dispositivo de salida o no se puede abrir; la
    /// reproducción sigue sin sonido (el reloj avanza con el tiempo real).
    pub fn abrir(estado: Arc<EstadoReproduccion>) -> Result<Self, String> {
        let dispositivo = cpal::default_host()
            .default_output_device()
            .ok_or("no hay dispositivo de salida de audio")?;
        let formato = dispositivo
            .default_output_config()
            .map_err(|e| format!("formato del dispositivo: {e}"))?;
        if formato.sample_format() != cpal::SampleFormat::F32 {
            return Err(format!(
                "formato de muestra {} no admitido (se requiere f32)",
                formato.sample_format()
            ));
        }
        let configuracion: cpal::StreamConfig = formato.into();
        let canales = usize::from(configuracion.channels);
        let frecuencia = configuracion.sample_rate;
        let flujo = dispositivo
            .build_output_stream(
                configuracion,
                move |salida: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    estado.llenar_salida(salida, canales, frecuencia);
                },
                |e| tracing::warn!(error = %e, "salida de audio"),
                None,
            )
            .map_err(|e| format!("no se pudo abrir la salida de audio: {e}"))?;
        flujo
            .play()
            .map_err(|e| format!("no se pudo iniciar la salida de audio: {e}"))?;
        Ok(Self { _flujo: flujo })
    }
}
