//! Tipos de los eventos defensivos del IDS.
//!
//! Cada vector corresponde a una detección que el código realmente produce;
//! no hay vectores declarados sin un emisor.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Número de niveles de [`NivelSeveridad`].
pub const NUMERO_NIVELES_SEVERIDAD: usize = 4;

/// Nivel de criticidad de un evento defensivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub enum NivelSeveridad {
    /// Información operativa.
    #[default]
    Informativo,
    /// Anomalía leve.
    Medio,
    /// Ataque contenido: se bloquea el origen y se rota el aislamiento.
    Alto,
    /// Compromiso posible: se ejecuta el pánico automático.
    Critico,
}

/// Vector de amenaza detectado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorAmenaza {
    /// Un servidor violó límites o protocolo (posible ataque de denegación o *smuggling*).
    RespuestaHostil {
        /// Host de origen.
        host: String,
        /// Motivo técnico.
        motivo: String,
    },
    /// El Worker detectó contenido hostil (por ejemplo, una bomba de descompresión).
    ContenidoHostil {
        /// Host de origen.
        host: String,
        /// Motivo técnico.
        motivo: String,
    },
    /// Un sub-Worker terminó de forma anómala procesando contenido de `host`.
    CaidaSandbox {
        /// Host de origen del contenido.
        host: String,
        /// Detalle de la terminación.
        detalle: String,
    },
    /// El filtro seccomp mató al sub-Worker: el contenido logró ejecutar una
    /// llamada al sistema prohibida.
    ViolacionLlamadaSistema {
        /// Host de origen del contenido.
        host: String,
    },
    /// Un archivo canario fue modificado, borrado o dejó de ser legible.
    CanarioAlterado(String),
    /// La trampa de memoria fue alterada.
    MemoriaTrampaAlterada,
}

/// Evento defensivo con marca temporal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventoDefensivo {
    /// Momento de la detección.
    pub marca_tiempo: SystemTime,
    /// Severidad evaluada.
    pub severidad: NivelSeveridad,
    /// Vector detectado.
    pub vector: VectorAmenaza,
}

impl EventoDefensivo {
    /// Crea un evento con la hora actual.
    pub fn nuevo(severidad: NivelSeveridad, vector: VectorAmenaza) -> Self {
        Self {
            marca_tiempo: SystemTime::now(),
            severidad,
            vector,
        }
    }
}
