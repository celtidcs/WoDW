//! Estado compartido de una reproducción: lo llena la tarea productora (que
//! pide bloques al sub-Worker) y lo vacían la salida de audio y la interfaz.
//!
//! El reloj de la reproducción es el audio ya entregado al dispositivo: los
//! fotogramas se muestran cuando su marca de tiempo llega a ese reloj. Sin
//! dispositivo de audio, la interfaz hace avanzar el reloj con el tiempo real
//! ([`EstadoReproduccion::avanzar_sin_dispositivo`]).

use crate::ipc::mensajes::{FotogramaVideo, CANALES_SALIDA, FRECUENCIA_SALIDA_HZ};
use crate::unidades::{ESCALA_MUESTRA_I16, MILISEGUNDOS_POR_SEGUNDO, POR_CIENTO};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Volumen máximo (100 %): el volumen solo atenúa, nunca amplifica, así que el
/// pico pactado con el Worker se mantiene.
pub const VOLUMEN_MAXIMO_POR_CIENTO: u32 = POR_CIENTO;
/// Escala de la fase fraccionaria del remuestreo, que se guarda en un entero
/// atómico en millonésimas de muestra.
const ESCALA_FASE: f64 = 1e6;
/// Marcos estéreo necesarios para interpolar: el actual y el siguiente.
const MARCOS_INTERPOLACION: usize = 2;

/// Fase de la reproducción.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaseReproduccion {
    /// Abriendo el medio en el sub-Worker.
    Abriendo,
    /// Reproduciendo (o en pausa).
    Reproduciendo,
    /// Terminada con normalidad.
    Terminada,
    /// Detenida por error; el mensaje es para el usuario.
    Error(String),
}

/// Estado compartido de una reproducción.
#[derive(Debug)]
pub struct EstadoReproduccion {
    audio: Mutex<VecDeque<i16>>,
    fotogramas: Mutex<VecDeque<FotogramaVideo>>,
    fase: Mutex<FaseReproduccion>,
    /// Muestras por canal ya entregadas al dispositivo (el reloj).
    reproducidas: AtomicU64,
    /// Resto fraccionario del remuestreo hacia el dispositivo, en millonésimas.
    fase_remuestreo: AtomicU64,
    pausado: AtomicBool,
    parado: AtomicBool,
    produccion_terminada: AtomicBool,
    volumen_por_ciento: AtomicU32,
}

/// Toma un cerrojo aunque otro hilo haya entrado en pánico con él: los datos
/// son colas de muestras, siempre coherentes entre operaciones.
fn bloquear<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl EstadoReproduccion {
    /// Estado nuevo con el volumen inicial (se satura a 100 %).
    pub fn nuevo(volumen_por_ciento: u32) -> Arc<Self> {
        Arc::new(Self {
            audio: Mutex::new(VecDeque::new()),
            fotogramas: Mutex::new(VecDeque::new()),
            fase: Mutex::new(FaseReproduccion::Abriendo),
            reproducidas: AtomicU64::new(0),
            fase_remuestreo: AtomicU64::new(0),
            pausado: AtomicBool::new(false),
            parado: AtomicBool::new(false),
            produccion_terminada: AtomicBool::new(false),
            volumen_por_ciento: AtomicU32::new(volumen_por_ciento.min(VOLUMEN_MAXIMO_POR_CIENTO)),
        })
    }

    /// Añade un bloque ya validado.
    pub fn anadir(&self, audio: Vec<i16>, fotogramas: Vec<FotogramaVideo>) {
        bloquear(&self.audio).extend(audio);
        bloquear(&self.fotogramas).extend(fotogramas);
    }

    /// Milisegundos de audio pendientes de reproducir.
    pub fn audio_pendiente_ms(&self) -> u64 {
        let marcos = bloquear(&self.audio).len() as u64 / u64::from(CANALES_SALIDA);
        marcos_a_ms(marcos)
    }

    /// Fotogramas pendientes de mostrar.
    pub fn fotogramas_pendientes(&self) -> usize {
        bloquear(&self.fotogramas).len()
    }

    /// Reloj de la reproducción en milisegundos.
    pub fn reloj_ms(&self) -> u64 {
        marcos_a_ms(self.reproducidas.load(Ordering::Relaxed))
    }

    /// Saca el fotograma más reciente cuya hora ha llegado, descartando los
    /// anteriores (si la interfaz va con retraso, salta fotogramas).
    pub fn tomar_fotograma_vigente(&self) -> Option<FotogramaVideo> {
        let reloj = self.reloj_ms();
        let mut cola = bloquear(&self.fotogramas);
        let mut vigente = None;
        while cola.front().is_some_and(|f| f.marca_ms <= reloj) {
            vigente = cola.pop_front();
        }
        vigente
    }

    /// Llena un búfer del dispositivo de `canales` canales a `frecuencia` Hz con
    /// muestras `f32`. En pausa, o si no hay audio, entrega silencio sin avanzar
    /// el reloj. El remuestreo hacia el dispositivo es lineal: el audio ya llega
    /// filtrado a 18 kHz, así que no hay nada que pueda solaparse.
    pub fn llenar_salida(&self, salida: &mut [f32], canales: usize, frecuencia: u32) {
        salida.fill(0.0);
        if canales == 0 || frecuencia == 0 || self.pausado.load(Ordering::Relaxed) || self.parado()
        {
            return;
        }
        let volumen = self.volumen_por_ciento.load(Ordering::Relaxed) as f32 / POR_CIENTO as f32;
        let paso = f64::from(FRECUENCIA_SALIDA_HZ) / f64::from(frecuencia);
        let mut posicion = self.fase_remuestreo.load(Ordering::Relaxed) as f64 / ESCALA_FASE;
        let mut cola = bloquear(&self.audio);
        let mut consumidas = 0u64;
        for marco in salida.chunks_mut(canales) {
            let indice = posicion.floor() as usize;
            let (Some(actual), Some(siguiente)) =
                (muestra(&cola, indice), muestra(&cola, indice + 1))
            else {
                break;
            };
            let t = (posicion - indice as f64) as f32;
            for (c, valor) in marco
                .iter_mut()
                .take(usize::from(CANALES_SALIDA))
                .enumerate()
            {
                *valor = (actual[c] + (siguiente[c] - actual[c]) * t) * volumen;
            }
            posicion += paso;
            let enteras = posicion.floor() as usize;
            if enteras > 0 {
                let quitar = (enteras * usize::from(CANALES_SALIDA)).min(cola.len());
                cola.drain(..quitar);
                consumidas += enteras as u64;
                posicion -= enteras as f64;
            }
        }
        self.fase_remuestreo
            .store((posicion * ESCALA_FASE) as u64, Ordering::Relaxed);
        self.reproducidas.fetch_add(consumidas, Ordering::Relaxed);
    }

    /// Sin dispositivo de audio: consume `ms` de audio para que el reloj avance.
    pub fn avanzar_sin_dispositivo(&self, ms: u64) {
        if self.pausado.load(Ordering::Relaxed) || self.parado() {
            return;
        }
        let muestras = ms * u64::from(FRECUENCIA_SALIDA_HZ) / MILISEGUNDOS_POR_SEGUNDO;
        let mut cola = bloquear(&self.audio);
        let disponibles = cola.len() as u64 / u64::from(CANALES_SALIDA);
        let consumidas = muestras.min(disponibles);
        cola.drain(..usize::try_from(consumidas).unwrap_or(0) * usize::from(CANALES_SALIDA));
        self.reproducidas.fetch_add(consumidas, Ordering::Relaxed);
    }

    /// Pausa o reanuda.
    pub fn pausar(&self, pausado: bool) {
        self.pausado.store(pausado, Ordering::Relaxed);
    }

    /// `true` si está en pausa.
    pub fn en_pausa(&self) -> bool {
        self.pausado.load(Ordering::Relaxed)
    }

    /// Detiene la reproducción para siempre y suelta lo almacenado.
    pub fn parar(&self) {
        self.parado.store(true, Ordering::Relaxed);
        bloquear(&self.audio).clear();
        let mut fotogramas = bloquear(&self.fotogramas);
        for f in fotogramas.iter_mut() {
            f.rgba.fill(0);
        }
        fotogramas.clear();
    }

    /// `true` si se ordenó parar.
    pub fn parado(&self) -> bool {
        self.parado.load(Ordering::Relaxed)
    }

    /// Fija el volumen (se satura a 100 %).
    pub fn fijar_volumen(&self, por_ciento: u32) {
        self.volumen_por_ciento
            .store(por_ciento.min(VOLUMEN_MAXIMO_POR_CIENTO), Ordering::Relaxed);
    }

    /// Volumen actual.
    pub fn volumen(&self) -> u32 {
        self.volumen_por_ciento.load(Ordering::Relaxed)
    }

    /// Fase actual.
    pub fn fase(&self) -> FaseReproduccion {
        bloquear(&self.fase).clone()
    }

    /// Cambia la fase.
    pub fn fijar_fase(&self, fase: FaseReproduccion) {
        *bloquear(&self.fase) = fase;
    }

    /// Marca que el Worker ya entregó el último bloque.
    pub fn marcar_produccion_terminada(&self) {
        self.produccion_terminada.store(true, Ordering::Relaxed);
    }

    /// `true` si ya no queda nada por producir ni por reproducir.
    pub fn agotada(&self) -> bool {
        self.produccion_terminada.load(Ordering::Relaxed)
            && bloquear(&self.audio).len() < usize::from(CANALES_SALIDA) * MARCOS_INTERPOLACION
            && self.fotogramas_pendientes() == 0
    }
}

/// Milisegundos que duran `marcos` marcos estéreo a la frecuencia de salida.
fn marcos_a_ms(marcos: u64) -> u64 {
    marcos * MILISEGUNDOS_POR_SEGUNDO / u64::from(FRECUENCIA_SALIDA_HZ)
}

/// Muestra estéreo `indice` de la cola, normalizada a ±1.
fn muestra(cola: &VecDeque<i16>, indice: usize) -> Option<[f32; 2]> {
    let base = indice * usize::from(CANALES_SALIDA);
    Some([
        f32::from(*cola.get(base)?) / ESCALA_MUESTRA_I16,
        f32::from(*cola.get(base + 1)?) / ESCALA_MUESTRA_I16,
    ])
}

/// Manejador compartible de una reproducción. Dos manejadores son iguales si
/// apuntan a la misma reproducción.
#[derive(Debug, Clone)]
pub struct ManejadorReproduccion(pub Arc<EstadoReproduccion>);

impl PartialEq for ManejadorReproduccion {
    fn eq(&self, otro: &Self) -> bool {
        Arc::ptr_eq(&self.0, &otro.0)
    }
}

impl Eq for ManejadorReproduccion {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_reloj_avanza_con_lo_reproducido_y_no_en_pausa() {
        let e = EstadoReproduccion::nuevo(100);
        e.anadir(vec![1000; 48_000 * 2], vec![]);
        let mut salida = vec![0.0; 4800 * 2];
        e.llenar_salida(&mut salida, 2, 48_000);
        assert_eq!(e.reloj_ms(), 100);
        assert!(salida.iter().all(|&x| (x - 1000.0 / 32_768.0).abs() < 1e-6));
        e.pausar(true);
        e.llenar_salida(&mut salida, 2, 48_000);
        assert_eq!(e.reloj_ms(), 100);
        assert!(salida.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn el_volumen_solo_atenua() {
        let e = EstadoReproduccion::nuevo(500);
        assert_eq!(e.volumen(), 100);
        e.fijar_volumen(50);
        e.anadir(vec![16_384; 200], vec![]);
        let mut salida = vec![0.0; 20];
        e.llenar_salida(&mut salida, 2, 48_000);
        assert!((salida[0] - 0.25).abs() < 1e-4);
    }

    #[test]
    fn dispositivo_a_otra_frecuencia_conserva_la_duracion() {
        let e = EstadoReproduccion::nuevo(100);
        e.anadir(vec![0; 48_000 * 2], vec![]);
        let mut salida = vec![0.0; 4410 * 2];
        for _ in 0..5 {
            e.llenar_salida(&mut salida, 2, 44_100);
        }
        assert!(e.reloj_ms().abs_diff(500) <= 1, "{}", e.reloj_ms());
    }

    #[test]
    fn fotograma_vigente_salta_los_atrasados() {
        let e = EstadoReproduccion::nuevo(100);
        let f = |marca_ms| FotogramaVideo {
            marca_ms,
            ancho: 1,
            alto: 1,
            rgba: vec![0; 4],
        };
        e.anadir(vec![0; 48_000 * 2], vec![f(0), f(40), f(80), f(500)]);
        e.avanzar_sin_dispositivo(100);
        assert_eq!(e.tomar_fotograma_vigente().map(|f| f.marca_ms), Some(80));
        assert_eq!(e.tomar_fotograma_vigente(), None);
    }

    #[test]
    fn parar_suelta_todo() {
        let e = EstadoReproduccion::nuevo(100);
        e.anadir(vec![1; 10], vec![]);
        e.parar();
        assert_eq!(e.audio_pendiente_ms(), 0);
        let mut salida = vec![1.0; 4];
        e.llenar_salida(&mut salida, 2, 48_000);
        assert_eq!(salida, vec![0.0; 4]);
    }
}
