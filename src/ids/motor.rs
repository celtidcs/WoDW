//! Motor del Sistema de Detección de Intrusiones (IDS) en tiempo real.
//!
//! Recibe eventos defensivos por un canal acotado, los difunde a la interfaz,
//! lleva las métricas de la sesión y ejecuta contramedidas automáticas
//! graduadas: rotación de aislamiento ante [`NivelSeveridad::Alto`] y pánico
//! automático ante [`NivelSeveridad::Critico`].

use crate::configuracion::ConfiguracionIds;
use crate::error::ErrorApp;
use crate::ids::eventos::{EventoDefensivo, NivelSeveridad};
use std::collections::VecDeque;
use std::sync::{Arc, PoisonError, RwLock};
use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;

/// Contramedida invocada por el motor.
pub type Contramedida = Arc<dyn Fn() + Send + Sync>;

/// Resumen de la telemetría de la sesión.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResumenTelemetria {
    /// Total de eventos recibidos.
    pub total_eventos: u64,
    /// Eventos por nivel, indexados por [`NivelSeveridad`] en orden ascendente.
    pub eventos_por_nivel: [u64; crate::ids::eventos::NUMERO_NIVELES_SEVERIDAD],
    /// Nivel más alto registrado.
    pub alerta_maxima: NivelSeveridad,
    /// Rotaciones automáticas ejecutadas.
    pub rotaciones_automaticas: u64,
    /// `true` si se disparó el pánico automático.
    pub panico_automatico: bool,
    /// Eventos más recientes (acotados).
    pub historial_reciente: Vec<EventoDefensivo>,
}

impl ResumenTelemetria {
    /// Número de eventos de un nivel.
    pub fn eventos_de(&self, nivel: NivelSeveridad) -> u64 {
        self.eventos_por_nivel[nivel as usize]
    }
}

/// Estado interno protegido por `RwLock`.
#[derive(Debug)]
struct EstadoIds {
    resumen: ResumenTelemetria,
    historial: VecDeque<EventoDefensivo>,
    limite_historial: usize,
}

impl EstadoIds {
    fn registrar(&mut self, evento: &EventoDefensivo) {
        let r = &mut self.resumen;
        r.total_eventos += 1;
        r.eventos_por_nivel[evento.severidad as usize] += 1;
        r.alerta_maxima = r.alerta_maxima.max(evento.severidad);
        if self.historial.len() >= self.limite_historial {
            self.historial.pop_front();
        }
        self.historial.push_back(evento.clone());
    }

    fn instantanea(&self) -> ResumenTelemetria {
        ResumenTelemetria {
            historial_reciente: self.historial.iter().cloned().collect(),
            ..self.resumen.clone()
        }
    }
}

/// Emisor clonable de eventos hacia el motor.
#[derive(Clone, Debug)]
pub struct EmisorIds {
    tx: mpsc::Sender<EventoDefensivo>,
}

impl EmisorIds {
    /// Emite un evento esperando hueco en el canal.
    ///
    /// # Errors
    /// [`ErrorApp::IdsViolacion`] si el motor ya no está activo.
    pub async fn emitir(&self, evento: EventoDefensivo) -> Result<(), ErrorApp> {
        self.tx
            .send(evento)
            .await
            .map_err(|_| ErrorApp::IdsViolacion("motor IDS detenido".to_string()))
    }

    /// Emite un evento sin esperar (contextos síncronos).
    ///
    /// # Errors
    /// [`ErrorApp::IdsViolacion`] si el canal está lleno o cerrado.
    pub fn emitir_sincrono(&self, evento: EventoDefensivo) -> Result<(), ErrorApp> {
        self.tx.try_send(evento).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => {
                ErrorApp::IdsViolacion("canal IDS saturado".to_string())
            }
            mpsc::error::TrySendError::Closed(_) => {
                ErrorApp::IdsViolacion("motor IDS detenido".to_string())
            }
        })
    }
}

/// Control y consulta del motor en marcha.
#[derive(Clone)]
pub struct ControlIds {
    emisor: EmisorIds,
    difusion: broadcast::Sender<EventoDefensivo>,
    estado: Arc<RwLock<EstadoIds>>,
    parada: watch::Sender<bool>,
}

impl ControlIds {
    /// Emisor de eventos.
    pub fn emisor(&self) -> EmisorIds {
        self.emisor.clone()
    }

    /// Nueva suscripción a la difusión de eventos.
    pub fn suscribir_telemetria(&self) -> broadcast::Receiver<EventoDefensivo> {
        self.difusion.subscribe()
    }

    /// Instantánea de la telemetría.
    pub fn obtener_resumen(&self) -> ResumenTelemetria {
        self.estado
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .instantanea()
    }

    /// Detiene el bucle del motor.
    pub fn detener(&self) {
        let _ = self.parada.send(true);
    }
}

/// Contramedidas automáticas del motor.
#[derive(Clone, Default)]
struct Contramedidas {
    activas: bool,
    rotacion: Option<Contramedida>,
    panico: Option<Contramedida>,
}

impl Contramedidas {
    /// Aplica la contramedida del nivel y la anota en el estado.
    fn aplicar(&self, evento: &EventoDefensivo, estado: &RwLock<EstadoIds>) {
        if !self.activas {
            return;
        }
        let (contramedida, anotar): (_, fn(&mut ResumenTelemetria)) = match evento.severidad {
            NivelSeveridad::Alto => (&self.rotacion, |r| r.rotaciones_automaticas += 1),
            NivelSeveridad::Critico => (&self.panico, |r| r.panico_automatico = true),
            NivelSeveridad::Informativo | NivelSeveridad::Medio => return,
        };
        tracing::warn!(?evento.vector, ?evento.severidad, "IDS: contramedida automática");
        anotar(
            &mut estado
                .write()
                .unwrap_or_else(PoisonError::into_inner)
                .resumen,
        );
        if let Some(accion) = contramedida {
            accion();
        }
    }
}

/// Constructor del motor IDS.
pub struct MotorIds {
    configuracion: ConfiguracionIds,
    contramedidas: Contramedidas,
}

impl MotorIds {
    /// Motor con la configuración indicada.
    pub fn nuevo(configuracion: ConfiguracionIds) -> Self {
        let contramedidas = Contramedidas {
            activas: configuracion.mitigacion_automatica,
            ..Contramedidas::default()
        };
        Self {
            configuracion,
            contramedidas,
        }
    }

    /// Contramedida ante eventos [`NivelSeveridad::Alto`].
    pub fn con_rotacion<F: Fn() + Send + Sync + 'static>(mut self, accion: F) -> Self {
        self.contramedidas.rotacion = Some(Arc::new(accion));
        self
    }

    /// Contramedida ante eventos [`NivelSeveridad::Critico`].
    pub fn con_panico<F: Fn() + Send + Sync + 'static>(mut self, accion: F) -> Self {
        self.contramedidas.panico = Some(Arc::new(accion));
        self
    }

    /// Arranca el bucle del motor en una tarea de Tokio.
    pub fn iniciar(self) -> (ControlIds, JoinHandle<()>) {
        let (tx, rx) = mpsc::channel(self.configuracion.capacidad_canal);
        let (difusion, _) = broadcast::channel(self.configuracion.capacidad_difusion);
        let (parada, parada_rx) = watch::channel(false);
        let estado = Arc::new(RwLock::new(EstadoIds {
            resumen: ResumenTelemetria::default(),
            historial: VecDeque::with_capacity(self.configuracion.limite_historial),
            limite_historial: self.configuracion.limite_historial,
        }));
        let control = ControlIds {
            emisor: EmisorIds { tx },
            difusion: difusion.clone(),
            estado: estado.clone(),
            parada,
        };
        let tarea = tokio::spawn(bucle(rx, parada_rx, difusion, estado, self.contramedidas));
        (control, tarea)
    }
}

/// Bucle principal: registra, difunde y responde a cada evento.
async fn bucle(
    mut rx: mpsc::Receiver<EventoDefensivo>,
    mut parada: watch::Receiver<bool>,
    difusion: broadcast::Sender<EventoDefensivo>,
    estado: Arc<RwLock<EstadoIds>>,
    contramedidas: Contramedidas,
) {
    loop {
        tokio::select! {
            cambio = parada.changed() => {
                if cambio.is_err() || *parada.borrow() {
                    return;
                }
            }
            mensaje = rx.recv() => {
                let Some(evento) = mensaje else { return };
                estado.write().unwrap_or_else(PoisonError::into_inner).registrar(&evento);
                contramedidas.aplicar(&evento, &estado);
                let _ = difusion.send(evento);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::eventos::VectorAmenaza;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    /// Plazo máximo de espera en pruebas; se sale antes en cuanto se cumple la condición.
    const PLAZO_PRUEBA: Duration = Duration::from_secs(5);

    fn evento(severidad: NivelSeveridad) -> EventoDefensivo {
        EventoDefensivo::nuevo(severidad, VectorAmenaza::MemoriaTrampaAlterada)
    }

    /// Espera a que el motor haya procesado `n` eventos.
    async fn esperar_eventos(control: &ControlIds, n: u64) -> ResumenTelemetria {
        tokio::time::timeout(PLAZO_PRUEBA, async {
            loop {
                let resumen = control.obtener_resumen();
                if resumen.total_eventos >= n {
                    return resumen;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("el motor no procesó los eventos a tiempo")
    }

    #[tokio::test]
    async fn acumula_metricas_y_rota_ante_alto() {
        let rotado = Arc::new(AtomicBool::new(false));
        let bandera = rotado.clone();
        let (control, _t) = MotorIds::nuevo(ConfiguracionIds::default())
            .con_rotacion(move || bandera.store(true, Ordering::SeqCst))
            .iniciar();
        let emisor = control.emisor();
        emisor
            .emitir(evento(NivelSeveridad::Informativo))
            .await
            .unwrap();
        emisor.emitir(evento(NivelSeveridad::Medio)).await.unwrap();
        emisor
            .emitir_sincrono(evento(NivelSeveridad::Alto))
            .unwrap();
        let r = esperar_eventos(&control, 3).await;
        assert_eq!(r.eventos_de(NivelSeveridad::Alto), 1);
        assert_eq!(r.alerta_maxima, NivelSeveridad::Alto);
        assert_eq!(r.rotaciones_automaticas, 1);
        assert!(!r.panico_automatico);
        assert!(rotado.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn critico_dispara_panico_automatico() {
        let panico = Arc::new(AtomicBool::new(false));
        let bandera = panico.clone();
        let (control, _t) = MotorIds::nuevo(ConfiguracionIds::default())
            .con_panico(move || bandera.store(true, Ordering::SeqCst))
            .iniciar();
        control
            .emisor()
            .emitir(evento(NivelSeveridad::Critico))
            .await
            .unwrap();
        let r = esperar_eventos(&control, 1).await;
        assert!(r.panico_automatico);
        assert!(panico.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn difunde_eventos_e_historial_acotado() {
        let cfg = ConfiguracionIds {
            limite_historial: 5,
            ..Default::default()
        };
        let (control, _t) = MotorIds::nuevo(cfg).iniciar();
        let mut receptor = control.suscribir_telemetria();
        for _ in 0..10 {
            control
                .emisor()
                .emitir(evento(NivelSeveridad::Informativo))
                .await
                .unwrap();
        }
        let recibido = tokio::time::timeout(PLAZO_PRUEBA, receptor.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(recibido.severidad, NivelSeveridad::Informativo);
        assert_eq!(
            esperar_eventos(&control, 10).await.historial_reciente.len(),
            5
        );
    }

    #[tokio::test]
    async fn detener_concluye_la_tarea() {
        let (control, tarea) = MotorIds::nuevo(ConfiguracionIds::default()).iniciar();
        control.detener();
        assert!(tokio::time::timeout(PLAZO_PRUEBA, tarea).await.is_ok());
    }
}
