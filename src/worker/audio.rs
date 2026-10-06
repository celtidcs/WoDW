//! Decodificación WAV PCM de 16 bits y filtro paso bajo anti-ultrasonidos.
//!
//! El filtro es un Butterworth de orden 8 (cuatro secciones biquad del
//! *Audio EQ Cookbook* de R. Bristow-Johnson): plano en la banda audible y con
//! una caída de unos 48 dB por octava por encima del corte, suficiente para
//! eliminar balizas ultrasónicas de rastreo entre dispositivos cuando la
//! frecuencia de muestreo permite representarlas.

use std::f64::consts::PI;

/// Orden del filtro Butterworth (debe ser par: una biquad por cada dos polos).
const ORDEN_BUTTERWORTH: usize = 8;
/// Código de formato PCM entero en la cabecera `fmt `.
const FORMATO_PCM: u16 = 1;
/// Código de formato extensible; el subformato debe ser PCM.
const FORMATO_EXTENSIBLE: u16 = 0xFFFE;
/// Bits por muestra soportados.
const BITS_POR_MUESTRA: u16 = 16;
/// Bytes por muestra de 16 bits.
const BYTES_POR_MUESTRA: usize = 2;
/// Tamaño de una cabecera de fragmento RIFF (identificador + longitud).
const CABECERA_FRAGMENTO: usize = 8;
/// Bytes de `RIFF<tamaño>WAVE`.
const CABECERA_RIFF: usize = 12;
/// Tamaño mínimo del fragmento `fmt `.
const TAMANO_MINIMO_FMT: usize = 16;

/// Audio PCM decodificado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioPcm {
    /// Frecuencia de muestreo en Hz.
    pub frecuencia_muestreo: u32,
    /// Número de canales.
    pub canales: u16,
    /// Muestras intercaladas por canal.
    pub muestras: Vec<i16>,
}

/// Decodifica un WAV PCM de 16 bits.
///
/// # Errors
/// Devuelve una descripción si el contenedor no es WAV, el formato no es PCM
/// de 16 bits o algún fragmento declara un tamaño imposible.
pub fn decodificar_wav(datos: &[u8]) -> Result<AudioPcm, String> {
    if datos.len() < CABECERA_RIFF || &datos[..4] != b"RIFF" || &datos[8..12] != b"WAVE" {
        return Err("no es un contenedor RIFF/WAVE".to_string());
    }
    let mut formato = None;
    let mut posicion = CABECERA_RIFF;
    while let Some(cabecera) = datos.get(posicion..posicion + CABECERA_FRAGMENTO) {
        let tamano =
            u32::from_le_bytes([cabecera[4], cabecera[5], cabecera[6], cabecera[7]]) as usize;
        let inicio = posicion + CABECERA_FRAGMENTO;
        let cuerpo = datos
            .get(inicio..inicio.saturating_add(tamano))
            .ok_or("fragmento WAV truncado")?;
        match &cabecera[..4] {
            b"fmt " => formato = Some(interpretar_fmt(cuerpo)?),
            b"data" => {
                let (frecuencia_muestreo, canales) =
                    formato.ok_or("fragmento data antes que fmt")?;
                return Ok(AudioPcm {
                    frecuencia_muestreo,
                    canales,
                    muestras: cuerpo
                        .as_chunks::<BYTES_POR_MUESTRA>()
                        .0
                        .iter()
                        .map(|&b| i16::from_le_bytes(b))
                        .collect(),
                });
            }
            _ => {}
        }
        posicion = inicio + tamano + tamano % 2;
    }
    Err("el WAV no contiene fragmento data".to_string())
}

/// Interpreta el fragmento `fmt ` y devuelve (frecuencia, canales).
fn interpretar_fmt(cuerpo: &[u8]) -> Result<(u32, u16), String> {
    if cuerpo.len() < TAMANO_MINIMO_FMT {
        return Err("fragmento fmt demasiado corto".to_string());
    }
    let leer_u16 = |i: usize| u16::from_le_bytes([cuerpo[i], cuerpo[i + 1]]);
    let codigo = leer_u16(0);
    let canales = leer_u16(2);
    let frecuencia = u32::from_le_bytes([cuerpo[4], cuerpo[5], cuerpo[6], cuerpo[7]]);
    let bits = leer_u16(14);
    if !(codigo == FORMATO_PCM || codigo == FORMATO_EXTENSIBLE) || bits != BITS_POR_MUESTRA {
        return Err(format!(
            "formato no soportado (código {codigo}, {bits} bits)"
        ));
    }
    if canales == 0 || frecuencia == 0 {
        return Err("canales o frecuencia nulos".to_string());
    }
    Ok((frecuencia, canales))
}

/// Sección biquad en forma directa I.
#[derive(Debug, Clone, Copy, Default)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    /// Paso bajo del *Audio EQ Cookbook*.
    fn paso_bajo(frecuencia_muestreo: f64, corte: f64, q: f64) -> Self {
        let w0 = 2.0 * PI * corte / frecuencia_muestreo;
        let alpha = w0.sin() / (2.0 * q);
        let coseno = w0.cos();
        let a0 = 1.0 + alpha;
        Self {
            b0: (1.0 - coseno) / 2.0 / a0,
            b1: (1.0 - coseno) / a0,
            b2: (1.0 - coseno) / 2.0 / a0,
            a1: -2.0 * coseno / a0,
            a2: (1.0 - alpha) / a0,
            ..Self::default()
        }
    }

    fn procesar(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Factores Q de las secciones de un Butterworth de orden [`ORDEN_BUTTERWORTH`].
fn factores_q() -> impl Iterator<Item = f64> {
    (1..=ORDEN_BUTTERWORTH / 2).map(|k| {
        let angulo = (2 * k - 1) as f64 * PI / (2 * ORDEN_BUTTERWORTH) as f64;
        1.0 / (2.0 * angulo.cos())
    })
}

/// Aplica el paso bajo a cada canal por separado.
///
/// Si el corte está en o por encima de la frecuencia de Nyquist, la señal no
/// puede contener nada por encima del corte y se devuelve sin cambios.
pub fn filtrar_paso_bajo(audio: &AudioPcm, corte_hz: u32) -> Vec<i16> {
    let frecuencia = f64::from(audio.frecuencia_muestreo);
    let corte = f64::from(corte_hz);
    if corte >= frecuencia / 2.0 {
        return audio.muestras.clone();
    }
    let canales = usize::from(audio.canales);
    let mut cascadas: Vec<Vec<Biquad>> = (0..canales)
        .map(|_| {
            factores_q()
                .map(|q| Biquad::paso_bajo(frecuencia, corte, q))
                .collect()
        })
        .collect();
    audio
        .muestras
        .iter()
        .enumerate()
        .map(|(indice, &muestra)| {
            let salida = cascadas[indice % canales]
                .iter_mut()
                .fold(f64::from(muestra), |x, seccion| seccion.procesar(x));
            salida
                .round()
                .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construye un WAV PCM16 mono con una sinusoide.
    fn wav_sinusoide(frecuencia_muestreo: u32, tono: f64, segundos: f64) -> Vec<u8> {
        let n = (f64::from(frecuencia_muestreo) * segundos) as usize;
        let datos: Vec<u8> = (0..n)
            .map(|i| {
                (10_000.0 * (2.0 * PI * tono * i as f64 / f64::from(frecuencia_muestreo)).sin())
                    as i16
            })
            .flat_map(i16::to_le_bytes)
            .collect();
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + datos.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&frecuencia_muestreo.to_le_bytes());
        wav.extend_from_slice(&(frecuencia_muestreo * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(datos.len() as u32).to_le_bytes());
        wav.extend_from_slice(&datos);
        wav
    }

    /// Ganancia en dB del filtro para un tono, ignorando el transitorio inicial.
    fn ganancia_db(frecuencia_muestreo: u32, tono: f64) -> f64 {
        let audio = decodificar_wav(&wav_sinusoide(frecuencia_muestreo, tono, 0.5)).unwrap();
        let salida = filtrar_paso_bajo(&audio, 18_000);
        let descarte = audio.muestras.len() / 5;
        let rms = |v: &[i16]| {
            (v[descarte..]
                .iter()
                .map(|&x| f64::from(x).powi(2))
                .sum::<f64>()
                / (v.len() - descarte) as f64)
                .sqrt()
        };
        20.0 * (rms(&salida) / rms(&audio.muestras)).log10()
    }

    #[test]
    fn banda_audible_pasa_y_ultrasonidos_se_atenuan() {
        assert!(ganancia_db(44_100, 1_000.0) > -1.0);
        assert!(ganancia_db(44_100, 5_000.0) > -1.0);
        assert!(ganancia_db(96_000, 30_000.0) < -30.0);
    }

    #[test]
    fn cabecera_wav_aporta_frecuencia_y_canales() {
        let audio = decodificar_wav(&wav_sinusoide(22_050, 440.0, 0.01)).unwrap();
        assert_eq!((audio.frecuencia_muestreo, audio.canales), (22_050, 1));
    }

    #[test]
    fn datos_no_wav_o_truncados_se_rechazan() {
        assert!(decodificar_wav(b"no es wav").is_err());
        let mut truncado = wav_sinusoide(8_000, 440.0, 0.01);
        truncado.truncate(50);
        assert!(decodificar_wav(&truncado).is_err());
    }
}
