//! Registro de la sesión (CA-RS1–RS5), independiente de la interfaz.
//!
//! El usuario elige **qué** se registra ([`ContenidoRegistro`]) y **cómo** se
//! guarda ([`GuardadoRegistro`]). El registro vive en memoria con un tope de
//! entradas; solo llega al disco al pulsar «Guardar registro» (Manual) o al
//! cerrar con normalidad (Automático). El pánico lo borra sin guardarlo.
//! Pasar a la opción más reveladora exige confirmación ([`CambioRegistro`]).

use crate::configuracion::{ConfiguracionRegistro, ContenidoRegistro, GuardadoRegistro};
use crate::seguridad::purgar_cadenas;
use crate::unidades::{SEGUNDOS_POR_DIA, SEGUNDOS_POR_HORA, SEGUNDOS_POR_MINUTO};
use std::collections::VecDeque;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Clase de suceso registrado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoriaRegistro {
    /// Estado de la conexión con Tor y renovación de circuitos.
    Tor,
    /// Incidente de seguridad (contenido hostil, IDS, Worker caído).
    Incidente,
    /// Medio hostil al reproducirlo.
    Medio,
    /// Pánico o purga.
    Purga,
    /// Aviso de versión nueva.
    Version,
    /// Página o archivo abierto (solo en el contenido Completo).
    Navegacion,
    /// Reproducción iniciada (solo en el contenido Completo).
    Reproduccion,
}

impl CategoriaRegistro {
    /// `true` si pertenece al contenido Seguridad.
    pub fn es_de_seguridad(self) -> bool {
        !matches!(self, Self::Navegacion | Self::Reproduccion)
    }

    /// Nombre para el texto exportado.
    pub fn nombre(self) -> &'static str {
        match self {
            Self::Tor => "Tor",
            Self::Incidente => "Incidente",
            Self::Medio => "Medio",
            Self::Purga => "Purga",
            Self::Version => "Versión",
            Self::Navegacion => "Navegación",
            Self::Reproduccion => "Reproducción",
        }
    }
}

/// Entrada del registro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntradaRegistro {
    /// Momento del suceso.
    pub instante: SystemTime,
    /// Clase.
    pub categoria: CategoriaRegistro,
    /// Descripción.
    pub mensaje: String,
}

/// Resultado de pedir un cambio de opción.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CambioRegistro {
    /// Aplicado (era igual o más discreto).
    Aplicado,
    /// Más revelador: hay que confirmarlo explícitamente.
    RequiereConfirmacion,
}

/// Registro de la sesión.
#[derive(Debug, Clone)]
pub struct RegistroSesion {
    contenido: ContenidoRegistro,
    guardado: GuardadoRegistro,
    entradas: VecDeque<EntradaRegistro>,
    maximo: usize,
}

impl RegistroSesion {
    /// Registro vacío con las opciones iniciales de la configuración.
    pub fn nuevo(cfg: &ConfiguracionRegistro) -> Self {
        Self {
            contenido: cfg.contenido,
            guardado: cfg.guardado,
            entradas: VecDeque::new(),
            maximo: cfg.max_entradas,
        }
    }

    /// Anota un suceso si corresponde al contenido elegido. Al llegar al tope
    /// se descarta la entrada más antigua.
    pub fn anotar(&mut self, categoria: CategoriaRegistro, mensaje: impl Into<String>) {
        if self.contenido == ContenidoRegistro::Seguridad && !categoria.es_de_seguridad() {
            return;
        }
        if self.entradas.len() >= self.maximo {
            if let Some(mut vieja) = self.entradas.pop_front() {
                purgar_cadenas([&mut vieja.mensaje]);
            }
        }
        self.entradas.push_back(EntradaRegistro {
            instante: SystemTime::now(),
            categoria,
            mensaje: mensaje.into(),
        });
    }

    /// Entradas en orden cronológico.
    pub fn entradas(&self) -> impl Iterator<Item = &EntradaRegistro> {
        self.entradas.iter()
    }

    /// Contenido elegido.
    pub fn contenido(&self) -> ContenidoRegistro {
        self.contenido
    }

    /// Guardado elegido.
    pub fn guardado(&self) -> GuardadoRegistro {
        self.guardado
    }

    /// Pide cambiar el contenido; Completo requiere confirmación. Volver a
    /// Seguridad borra las entradas que ya no le corresponden.
    pub fn solicitar_contenido(&mut self, nuevo: ContenidoRegistro) -> CambioRegistro {
        if nuevo == ContenidoRegistro::Completo && self.contenido != nuevo {
            return CambioRegistro::RequiereConfirmacion;
        }
        self.confirmar_contenido(nuevo);
        CambioRegistro::Aplicado
    }

    /// Aplica el contenido (tras la confirmación del usuario si hacía falta).
    pub fn confirmar_contenido(&mut self, nuevo: ContenidoRegistro) {
        self.contenido = nuevo;
        if nuevo == ContenidoRegistro::Seguridad {
            for entrada in self
                .entradas
                .iter_mut()
                .filter(|e| !e.categoria.es_de_seguridad())
            {
                purgar_cadenas([&mut entrada.mensaje]);
            }
            self.entradas.retain(|e| e.categoria.es_de_seguridad());
        }
    }

    /// Pide cambiar el guardado; Automático requiere confirmación.
    pub fn solicitar_guardado(&mut self, nuevo: GuardadoRegistro) -> CambioRegistro {
        if nuevo == GuardadoRegistro::Automatico && self.guardado != nuevo {
            return CambioRegistro::RequiereConfirmacion;
        }
        self.guardado = nuevo;
        CambioRegistro::Aplicado
    }

    /// Aplica el guardado (tras la confirmación del usuario si hacía falta).
    pub fn confirmar_guardado(&mut self, nuevo: GuardadoRegistro) {
        self.guardado = nuevo;
    }

    /// Texto del registro, una entrada por línea con fecha UTC.
    pub fn texto(&self) -> String {
        self.entradas
            .iter()
            .map(|e| {
                format!(
                    "{} [{}] {}\n",
                    fecha_utc(e.instante),
                    e.categoria.nombre(),
                    e.mensaje
                )
            })
            .collect()
    }

    /// Escribe el registro en `ruta` (lo que hace «Guardar registro»).
    ///
    /// # Errors
    /// El error de E/S si no se puede escribir.
    pub fn guardar(&self, ruta: &Path) -> std::io::Result<()> {
        std::fs::write(ruta, self.texto())
    }

    /// Al cerrar con normalidad: escribe en `ruta` solo si el guardado es
    /// Automático. Devuelve si escribió.
    ///
    /// # Errors
    /// El error de E/S si no se puede escribir.
    pub fn al_cerrar(&self, ruta: &Path) -> std::io::Result<bool> {
        if self.guardado != GuardadoRegistro::Automatico {
            return Ok(false);
        }
        self.guardar(ruta).map(|()| true)
    }

    /// Borra todas las entradas sobrescribiendo sus textos.
    pub fn vaciar(&mut self) {
        purgar_cadenas(self.entradas.iter_mut().map(|e| &mut e.mensaje));
        self.entradas.clear();
    }
}

/// Fecha `AAAA-MM-DD HH:MM:SS UTC` de un instante (algoritmo de días civiles
/// de H. Hinnant, válido para el calendario gregoriano proléptico). Las cifras
/// del cálculo son las del algoritmo publicado (eras de 400 años = 146 097
/// días, desplazamiento 719 468 del 0000-03-01 a 1970-01-01, meses de marzo a
/// febrero): https://howardhinnant.github.io/date_algorithms.html#civil_from_days
fn fecha_utc(instante: SystemTime) -> String {
    let segundos = instante
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (dias, resto) = (segundos / SEGUNDOS_POR_DIA, segundos % SEGUNDOS_POR_DIA);
    let z = dias as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let anio = yoe + era * 400 + i64::from(mes <= 2);
    format!(
        "{anio:04}-{mes:02}-{dia:02} {:02}:{:02}:{:02} UTC",
        resto / SEGUNDOS_POR_HORA,
        resto % SEGUNDOS_POR_HORA / SEGUNDOS_POR_MINUTO,
        resto % SEGUNDOS_POR_MINUTO
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn fechas_conocidas() {
        assert_eq!(fecha_utc(UNIX_EPOCH), "1970-01-01 00:00:00 UTC");
        // 2026-10-06 12:34:56 UTC = 1 791 290 096 s.
        assert_eq!(
            fecha_utc(UNIX_EPOCH + Duration::from_secs(1_791_290_096)),
            "2026-10-06 12:34:56 UTC"
        );
        // 29 de febrero de 2024 (año bisiesto).
        assert_eq!(
            fecha_utc(UNIX_EPOCH + Duration::from_secs(1_709_164_800)),
            "2024-02-29 00:00:00 UTC"
        );
    }

    #[test]
    fn tope_de_entradas() {
        let mut r = RegistroSesion::nuevo(&ConfiguracionRegistro {
            max_entradas: 2,
            ..Default::default()
        });
        for i in 0..5 {
            r.anotar(CategoriaRegistro::Tor, format!("{i}"));
        }
        let mensajes: Vec<&str> = r.entradas().map(|e| e.mensaje.as_str()).collect();
        assert_eq!(mensajes, ["3", "4"]);
    }
}
