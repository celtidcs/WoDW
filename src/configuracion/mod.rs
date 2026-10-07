//! Configuración global de WoDW cargada desde TOML.
//!
//! Ningún valor ajustable vive incrustado en el código: todos se declaran en
//! [`secciones`] con su valor por defecto documentado. El archivo `wodw.toml`
//! es opcional; si falta, rigen los valores por defecto. Si existe pero es
//! inválido, el arranque falla (fail fast) en vez de continuar con un valor
//! sorprendente.

mod onion;
pub mod secciones;
mod validacion;

pub use onion::es_direccion_onion_v3;
pub use secciones::{
    AccesoDirecto, ConfiguracionAccesos, ConfiguracionActualizaciones, ConfiguracionAutomatizacion,
    ConfiguracionCanarios, ConfiguracionIds, ConfiguracionInterfaz, ConfiguracionMotores,
    ConfiguracionPanico, ConfiguracionRedHttp, ConfiguracionRegistro, ConfiguracionReproduccion,
    ConfiguracionTor, ConfiguracionWorker, ContenidoRegistro, Fiabilidad, GuardadoRegistro,
    ModoVanguardias, MotorConfigurado, TransporteEnchufable, MOTIVO_SIN_COMPROBAR,
};

use crate::error::{ErrorApp, Resultado};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Nombre del archivo de configuración buscado junto al ejecutable.
pub const NOMBRE_ARCHIVO_CONFIGURACION: &str = "wodw.toml";

/// Raíz de la configuración de WoDW.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfiguracionWodw {
    /// Pila HTTP del Maestro.
    pub red: ConfiguracionRedHttp,
    /// Cliente Tor embebido.
    pub tor: ConfiguracionTor,
    /// Motores de búsqueda.
    pub motores: ConfiguracionMotores,
    /// Accesos directos.
    pub accesos: ConfiguracionAccesos,
    /// Procesamiento confinado en el Worker.
    pub worker: ConfiguracionWorker,
    /// Reproducción de audio y vídeo.
    pub reproduccion: ConfiguracionReproduccion,
    /// Aviso de versión nueva.
    pub actualizaciones: ConfiguracionActualizaciones,
    /// Registro de la sesión.
    pub registro: ConfiguracionRegistro,
    /// Motor IDS.
    pub ids: ConfiguracionIds,
    /// Botón del pánico.
    pub panico: ConfiguracionPanico,
    /// Archivos canario.
    pub canarios: ConfiguracionCanarios,
    /// Interfaz gráfica.
    pub interfaz: ConfiguracionInterfaz,
    /// Respuestas automáticas.
    pub automatizacion: ConfiguracionAutomatizacion,
}

impl ConfiguracionWodw {
    /// Interpreta y valida un texto TOML.
    ///
    /// # Errors
    /// [`ErrorApp::Configuracion`] si el TOML no se puede interpretar, contiene
    /// claves desconocidas o algún valor viola sus invariantes.
    pub fn desde_texto(texto: &str) -> Resultado<Self> {
        let configuracion: Self = toml::from_str(texto).map_err(|e| ErrorApp::Configuracion {
            campo: "wodw.toml".to_string(),
            motivo: e.to_string(),
        })?;
        validacion::validar(&configuracion)?;
        Ok(configuracion)
    }

    /// Lee y valida un archivo TOML.
    ///
    /// # Errors
    /// [`ErrorApp::Io`] si no se puede leer y [`ErrorApp::Configuracion`] si es inválido.
    pub fn desde_archivo(ruta: &Path) -> Resultado<Self> {
        let texto = std::fs::read_to_string(ruta)?;
        Self::desde_texto(&texto)
    }

    /// Carga la configuración efectiva del ejecutable.
    ///
    /// Orden: la ruta de `--config <ruta>` si se indicó; si no, `wodw.toml`
    /// junto al ejecutable si existe; si no, los valores por defecto.
    ///
    /// # Errors
    /// Los de [`Self::desde_archivo`]; un `--config` sin ruta es [`ErrorApp::Configuracion`].
    pub fn cargar(argumentos: &[String]) -> Resultado<Self> {
        match ruta_configuracion(argumentos)? {
            Some(ruta) => Self::desde_archivo(&ruta),
            None => Ok(Self::default()),
        }
    }

    /// Serializa la sección del Worker para pasarla al subproceso.
    ///
    /// # Errors
    /// [`ErrorApp::Configuracion`] si la serialización falla.
    pub fn worker_como_argumento(&self) -> Resultado<String> {
        toml::to_string(&self.worker).map_err(|e| ErrorApp::Configuracion {
            campo: "worker".to_string(),
            motivo: e.to_string(),
        })
    }

    /// Reconstruye y valida la sección del Worker recibida por línea de órdenes.
    ///
    /// # Errors
    /// [`ErrorApp::Configuracion`] si el texto es inválido.
    pub fn worker_desde_argumento(texto: &str) -> Resultado<ConfiguracionWorker> {
        let worker: ConfiguracionWorker =
            toml::from_str(texto).map_err(|e| ErrorApp::Configuracion {
                campo: "worker".to_string(),
                motivo: e.to_string(),
            })?;
        validacion::validar_worker(&worker)?;
        Ok(worker)
    }
}

/// Determina qué archivo de configuración usar, si alguno.
fn ruta_configuracion(argumentos: &[String]) -> Resultado<Option<PathBuf>> {
    if let Some(posicion) = argumentos.iter().position(|a| a == "--config") {
        let ruta = argumentos
            .get(posicion + 1)
            .ok_or_else(|| ErrorApp::Configuracion {
                campo: "--config".to_string(),
                motivo: "falta la ruta del archivo".to_string(),
            })?;
        return Ok(Some(PathBuf::from(ruta)));
    }
    let junto_al_ejecutable = std::env::current_exe()?
        .parent()
        .map(|dir| dir.join(NOMBRE_ARCHIVO_CONFIGURACION));
    Ok(junto_al_ejecutable.filter(|ruta| ruta.is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texto_vacio_produce_valores_por_defecto() {
        let cfg = ConfiguracionWodw::desde_texto("").unwrap();
        assert_eq!(cfg, ConfiguracionWodw::default());
    }

    #[test]
    fn clave_desconocida_se_rechaza() {
        let err = ConfiguracionWodw::desde_texto("[red]\nlimite_cuerpo = 5\n").unwrap_err();
        assert!(matches!(err, ErrorApp::Configuracion { .. }), "{err}");
    }

    #[test]
    fn seccion_worker_viaja_ida_y_vuelta_por_argumento() {
        let cfg = ConfiguracionWodw::default();
        let texto = cfg.worker_como_argumento().unwrap();
        let worker = ConfiguracionWodw::worker_desde_argumento(&texto).unwrap();
        assert_eq!(worker, cfg.worker);
    }

    #[test]
    fn config_sin_ruta_es_error() {
        let args = vec!["wodw".to_string(), "--config".to_string()];
        assert!(ConfiguracionWodw::cargar(&args).is_err());
    }
}
