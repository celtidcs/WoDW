//! Clasificación de fallos en incidentes de seguridad y respuesta automática.
//!
//! Sin intervención del usuario: un incidente bloquea el host durante la sesión
//! (si está configurado), se notifica al IDS —que rota el aislamiento ante
//! severidad alta o dispara el pánico ante severidad crítica— y se indica si la
//! pestaña debe purgarse.

use crate::error::ErrorApp;
use crate::ids::eventos::{EventoDefensivo, NivelSeveridad, VectorAmenaza};
use crate::ids::motor::EmisorIds;
use std::collections::HashSet;
use std::sync::{Mutex, PoisonError};

/// Operaciones de E/S HTTP cuyo plazo agotado se considera hostil: el servidor
/// aceptó la conexión y luego la retuvo deliberadamente.
const OPERACIONES_HTTP_HOSTILES: &[&str] = &["lectura HTTP", "escritura HTTP"];
/// Operación del Worker cuyo plazo agotado delata contenido hostil.
const OPERACION_WORKER: &str = "procesamiento en el Worker";

/// Incidente derivado de un error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incidente {
    /// Severidad para el IDS.
    pub severidad: NivelSeveridad,
    /// Vector para el IDS.
    pub vector: VectorAmenaza,
    /// Si el contenido de la pestaña debe purgarse.
    pub purgar_pestana: bool,
}

/// Clasifica `error` producido al navegar a `host`. `None` = fallo ordinario.
pub fn clasificar(host: &str, error: &ErrorApp) -> Option<Incidente> {
    let host = host.to_string();
    let (severidad, vector, purgar_pestana) = match error {
        ErrorApp::LimiteExcedido { .. } | ErrorApp::ProtocoloHttp(_) => (
            NivelSeveridad::Alto,
            VectorAmenaza::RespuestaHostil {
                host,
                motivo: error.to_string(),
            },
            false,
        ),
        ErrorApp::TiempoAgotado { operacion, .. }
            if OPERACIONES_HTTP_HOSTILES.contains(operacion) =>
        {
            (
                NivelSeveridad::Alto,
                VectorAmenaza::RespuestaHostil {
                    host,
                    motivo: error.to_string(),
                },
                false,
            )
        }
        ErrorApp::TiempoAgotado { operacion, .. } if *operacion == OPERACION_WORKER => (
            NivelSeveridad::Alto,
            VectorAmenaza::ContenidoHostil {
                host,
                motivo: error.to_string(),
            },
            true,
        ),
        ErrorApp::ContenidoHostil(motivo) => (
            NivelSeveridad::Alto,
            VectorAmenaza::ContenidoHostil {
                host,
                motivo: motivo.clone(),
            },
            true,
        ),
        ErrorApp::WorkerTerminado {
            violacion_llamada_sistema: true,
            ..
        } => (
            NivelSeveridad::Critico,
            VectorAmenaza::ViolacionLlamadaSistema { host },
            true,
        ),
        ErrorApp::WorkerTerminado { detalle, .. } => (
            NivelSeveridad::Alto,
            VectorAmenaza::CaidaSandbox {
                host,
                detalle: detalle.clone(),
            },
            true,
        ),
        _ => return None,
    };
    Some(Incidente {
        severidad,
        vector,
        purgar_pestana,
    })
}

/// Respuesta automática: bloqueo de hosts y notificación al IDS.
#[derive(Debug, Default)]
pub struct RespuestaAutomatica {
    bloquear: bool,
    bloqueados: Mutex<HashSet<String>>,
    emisor: Option<EmisorIds>,
}

impl RespuestaAutomatica {
    /// Crea la respuesta automática.
    pub fn nueva(bloquear: bool, emisor: Option<EmisorIds>) -> Self {
        Self {
            bloquear,
            bloqueados: Mutex::new(HashSet::new()),
            emisor,
        }
    }

    /// `true` si `host` está bloqueado en esta sesión.
    pub fn esta_bloqueado(&self, host: &str) -> bool {
        self.bloqueados
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(host)
    }

    /// Aplica la respuesta a un incidente en `host`.
    pub fn responder(&self, host: &str, incidente: &Incidente) {
        if self.bloquear {
            self.bloqueados
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(host.to_string());
        }
        tracing::warn!(host, ?incidente.vector, "incidente de seguridad: respuesta automática");
        if let Some(emisor) = &self.emisor {
            let evento = EventoDefensivo::nuevo(incidente.severidad, incidente.vector.clone());
            if let Err(e) = emisor.emitir_sincrono(evento) {
                tracing::error!(error = %e, "no se pudo notificar el incidente al IDS");
            }
        }
    }

    /// Olvida todos los bloqueos (purga de sesión).
    pub fn olvidar_bloqueos(&self) {
        self.bloqueados
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clasifica_errores_hostiles_y_ordinarios() {
        let limite = ErrorApp::LimiteExcedido {
            recurso: "cuerpo",
            limite: 1,
        };
        assert_eq!(
            clasificar("h", &limite).unwrap().severidad,
            NivelSeveridad::Alto
        );
        let seccomp = ErrorApp::WorkerTerminado {
            violacion_llamada_sistema: true,
            detalle: String::new(),
        };
        let incidente = clasificar("h", &seccomp).unwrap();
        assert_eq!(incidente.severidad, NivelSeveridad::Critico);
        assert!(incidente.purgar_pestana);
        let conexion = ErrorApp::TiempoAgotado {
            operacion: "conexión Tor",
            milisegundos: 1,
        };
        assert!(clasificar("h", &conexion).is_none());
        assert!(clasificar("h", &ErrorApp::RedArti("x".into())).is_none());
    }

    #[test]
    fn bloquea_el_host_y_olvida_al_purgar() {
        let respuesta = RespuestaAutomatica::nueva(true, None);
        let incidente = clasificar("malo.onion", &ErrorApp::ProtocoloHttp("x".into())).unwrap();
        respuesta.responder("malo.onion", &incidente);
        assert!(respuesta.esta_bloqueado("malo.onion"));
        assert!(!respuesta.esta_bloqueado("bueno.onion"));
        respuesta.olvidar_bloqueos();
        assert!(!respuesta.esta_bloqueado("malo.onion"));
    }
}
