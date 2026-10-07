//! Cadena fija del audio (CA-A2): todo audio sale a 48 kHz, estéreo y 16 bits,
//! sin ultrasonidos y sin picos por encima del máximo configurado.
//!
//! Pasos, con estado entre bloques para que no haya cortes:
//! 1. Mezcla a estéreo (mono se duplica; con más de dos canales, el central
//!    se reparte a −3 dB y el resto se descarta).
//! 2. Paso bajo antes de remuestrear (antisolapamiento) en la frecuencia de
//!    origen, con corte en el menor de: el configurado o el 45 % de la
//!    frecuencia de origen.
//! 3. Remuestreo a 48 kHz por interpolación cúbica (Catmull-Rom).
//! 4. Paso bajo de nuevo a 48 kHz: elimina los restos del remuestreo y
//!    garantiza el corte configurado en la salida.
//! 5. Limitador de pico con ataque instantáneo y recuperación suave, y recorte
//!    final como garantía.

use crate::configuracion::ConfiguracionWorker;
use crate::ipc::mensajes::FRECUENCIA_SALIDA_HZ;
use std::f64::consts::PI;

/// Orden del filtro Butterworth (par: una sección biquad por cada dos polos).
const ORDEN_BUTTERWORTH: usize = 8;
/// Fracción de la frecuencia de origen usada como corte antisolapamiento.
const FRACCION_NYQUIST_SEGURA: f64 = 0.45;
/// Ganancia del canal central al repartirlo en dos (−3 dB).
const GANANCIA_CENTRAL: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// Tiempo de recuperación del limitador (segundos).
const RECUPERACION_LIMITADOR_S: f64 = 0.05;
/// Frecuencias de origen admitidas (Hz).
const FRECUENCIAS_ADMITIDAS: std::ops::RangeInclusive<u32> = 1_000..=384_000;
/// Canales de origen admitidos.
const CANALES_ADMITIDOS: std::ops::RangeInclusive<usize> = 1..=8;
/// Escala de 16 bits.
const ESCALA_I16: f32 = 32_767.0;

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
    /// Paso bajo del *Audio EQ Cookbook* de R. Bristow-Johnson.
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

/// Paso bajo Butterworth de orden [`ORDEN_BUTTERWORTH`] para dos canales.
#[derive(Debug, Clone)]
struct FiltroPasoBajo {
    canales: [Vec<Biquad>; 2],
}

impl FiltroPasoBajo {
    fn nuevo(frecuencia_muestreo: f64, corte: f64) -> Self {
        let cascada = || -> Vec<Biquad> {
            (1..=ORDEN_BUTTERWORTH / 2)
                .map(|k| {
                    let angulo = (2 * k - 1) as f64 * PI / (2 * ORDEN_BUTTERWORTH) as f64;
                    Biquad::paso_bajo(frecuencia_muestreo, corte, 1.0 / (2.0 * angulo.cos()))
                })
                .collect()
        };
        Self {
            canales: [cascada(), cascada()],
        }
    }

    fn procesar(&mut self, muestra: [f32; 2]) -> [f32; 2] {
        let mut salida = [0.0; 2];
        for (canal, (secciones, x)) in self.canales.iter_mut().zip(muestra).enumerate() {
            salida[canal] = secciones
                .iter_mut()
                .fold(f64::from(x), |v, s| s.procesar(v)) as f32;
        }
        salida
    }
}

/// Remuestreador cúbico con estado entre bloques.
#[derive(Debug, Clone)]
struct Remuestreador {
    /// Muestras de origen por muestra de salida.
    paso: f64,
    /// Posición de la próxima salida, en muestras de `pendientes`.
    posicion: f64,
    /// Muestras de origen aún necesarias (la primera es la anterior a `posicion`).
    pendientes: Vec<[f32; 2]>,
}

impl Remuestreador {
    fn nuevo(frecuencia_origen: u32) -> Self {
        Self {
            paso: f64::from(frecuencia_origen) / f64::from(FRECUENCIA_SALIDA_HZ),
            posicion: 1.0,
            pendientes: vec![[0.0; 2]],
        }
    }

    /// Añade muestras de origen y devuelve las de salida que ya se pueden calcular.
    fn procesar(&mut self, entrada: impl IntoIterator<Item = [f32; 2]>) -> Vec<[f32; 2]> {
        self.pendientes.extend(entrada);
        let mut salida = Vec::new();
        while self.posicion + 2.0 < self.pendientes.len() as f64 {
            let i = self.posicion.floor() as usize;
            let t = (self.posicion - i as f64) as f32;
            let p = &self.pendientes;
            let mut muestra = [0.0; 2];
            for (c, m) in muestra.iter_mut().enumerate() {
                *m = catmull_rom(p[i - 1][c], p[i][c], p[i + 1][c], p[i + 2][c], t);
            }
            salida.push(muestra);
            self.posicion += self.paso;
        }
        let consumidas = (self.posicion.floor() as usize)
            .saturating_sub(1)
            .min(self.pendientes.len());
        self.pendientes.drain(..consumidas);
        self.posicion -= consumidas as f64;
        salida
    }
}

/// Interpolación de Catmull-Rom entre `p1` y `p2` (spline cúbica uniforme con
/// tensión 0,5; los coeficientes son los de su forma matricial estándar).
fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t2 * t)
}

/// Limitador de pico.
#[derive(Debug, Clone)]
struct Limitador {
    pico: f32,
    ganancia: f32,
    recuperacion: f32,
}

impl Limitador {
    fn nuevo(pico_por_mil: u16) -> Self {
        Self {
            pico: f32::from(pico_por_mil) / f32::from(crate::unidades::POR_MIL),
            ganancia: 1.0,
            recuperacion: (1.0 / (RECUPERACION_LIMITADOR_S * f64::from(FRECUENCIA_SALIDA_HZ)))
                as f32,
        }
    }

    fn procesar(&mut self, muestra: [f32; 2]) -> [f32; 2] {
        let maximo = muestra[0].abs().max(muestra[1].abs());
        if maximo * self.ganancia > self.pico {
            self.ganancia = self.pico / maximo;
        }
        let salida = muestra.map(|x| (x * self.ganancia).clamp(-self.pico, self.pico));
        self.ganancia += (1.0 - self.ganancia) * self.recuperacion;
        salida
    }
}

/// Cadena completa de un flujo de audio.
#[derive(Debug, Clone)]
pub struct CadenaAudio {
    canales_origen: usize,
    prefiltro: FiltroPasoBajo,
    remuestreador: Remuestreador,
    posfiltro: FiltroPasoBajo,
    limitador: Limitador,
    /// Pico en unidades de 16 bits, el mismo cálculo entero que usa el Maestro
    /// al revalidar (evita que el redondeo lo supere en una unidad).
    pico_i16: i16,
}

impl CadenaAudio {
    /// Prepara la cadena para un origen de `frecuencia_origen` Hz y `canales_origen` canales.
    ///
    /// # Errors
    /// Descripción si la frecuencia o el número de canales están fuera de lo admitido.
    pub fn nueva(
        frecuencia_origen: u32,
        canales_origen: usize,
        cfg: &ConfiguracionWorker,
    ) -> Result<Self, String> {
        if !FRECUENCIAS_ADMITIDAS.contains(&frecuencia_origen)
            || !CANALES_ADMITIDOS.contains(&canales_origen)
        {
            return Err(format!(
                "audio de {frecuencia_origen} Hz y {canales_origen} canales fuera de lo admitido"
            ));
        }
        let corte = f64::from(cfg.frecuencia_corte_audio_hz);
        let origen = f64::from(frecuencia_origen);
        Ok(Self {
            canales_origen,
            prefiltro: FiltroPasoBajo::nuevo(origen, corte.min(origen * FRACCION_NYQUIST_SEGURA)),
            remuestreador: Remuestreador::nuevo(frecuencia_origen),
            posfiltro: FiltroPasoBajo::nuevo(
                f64::from(FRECUENCIA_SALIDA_HZ),
                corte.min(f64::from(FRECUENCIA_SALIDA_HZ) * FRACCION_NYQUIST_SEGURA),
            ),
            limitador: Limitador::nuevo(cfg.pico_maximo_audio_por_mil),
            pico_i16: cfg.pico_maximo_muestra(),
        })
    }

    /// Procesa muestras de origen intercaladas y devuelve PCM estéreo de 16 bits
    /// a 48 kHz. Un resto que no completa una muestra de todos los canales se ignora.
    pub fn procesar(&mut self, entrada: &[f32]) -> Vec<i16> {
        let estereo: Vec<[f32; 2]> = entrada
            .chunks_exact(self.canales_origen)
            .map(|m| self.prefiltro.procesar(mezclar(m)))
            .collect();
        self.remuestreador
            .procesar(estereo)
            .into_iter()
            .flat_map(|m| self.limitador.procesar(self.posfiltro.procesar(m)))
            .map(|x| ((x * ESCALA_I16).round() as i16).clamp(-self.pico_i16, self.pico_i16))
            .collect()
    }
}

/// Mezcla una muestra multicanal a estéreo. Un valor no finito cuenta como silencio.
fn mezclar(muestra: &[f32]) -> [f32; 2] {
    let canal = |i: usize| {
        muestra
            .get(i)
            .copied()
            .filter(|x| x.is_finite())
            .unwrap_or(0.0)
    };
    match muestra.len() {
        1 => [canal(0); 2],
        2 => [canal(0), canal(1)],
        _ => [
            canal(0) + GANANCIA_CENTRAL * canal(2),
            canal(1) + GANANCIA_CENTRAL * canal(2),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ganancia en dB de un tono puro mono a `frecuencia` Hz.
    fn ganancia_db(frecuencia: u32, tono: f64) -> f64 {
        let mut cadena =
            CadenaAudio::nueva(frecuencia, 1, &ConfiguracionWorker::default()).unwrap();
        let entrada: Vec<f32> = (0..frecuencia / 2)
            .map(|i| 0.3 * (2.0 * PI * tono * f64::from(i) / f64::from(frecuencia)).sin() as f32)
            .collect();
        let salida = cadena.procesar(&entrada);
        let tramo = &salida[salida.len() / 5..];
        let rms =
            (tramo.iter().map(|&x| f64::from(x).powi(2)).sum::<f64>() / tramo.len() as f64).sqrt();
        20.0 * (rms / (0.3 * f64::from(ESCALA_I16) / 2f64.sqrt())).log10()
    }

    #[test]
    fn audible_pasa_y_ultrasonido_se_atenua() {
        assert!(ganancia_db(44_100, 1_000.0) > -1.0);
        assert!(ganancia_db(44_100, 5_000.0) > -1.0);
        assert!(ganancia_db(48_000, 22_000.0) < -30.0);
        assert!(ganancia_db(96_000, 30_000.0) < -30.0);
    }

    #[test]
    fn duracion_se_conserva_al_remuestrear() {
        let mut cadena = CadenaAudio::nueva(44_100, 2, &ConfiguracionWorker::default()).unwrap();
        let salida: usize = (0..10)
            .map(|_| cadena.procesar(&[0.0; 4410 * 2]).len())
            .sum();
        let esperado = 48_000 * 2;
        assert!(salida.abs_diff(esperado) <= 8, "{salida}");
    }

    #[test]
    fn origen_imposible_se_rechaza() {
        let cfg = ConfiguracionWorker::default();
        assert!(CadenaAudio::nueva(0, 2, &cfg).is_err());
        assert!(CadenaAudio::nueva(48_000, 0, &cfg).is_err());
        assert!(CadenaAudio::nueva(48_000, 9, &cfg).is_err());
    }

    #[test]
    fn valores_no_finitos_son_silencio() {
        assert_eq!(mezclar(&[f32::NAN, f32::INFINITY]), [0.0, 0.0]);
    }
}
