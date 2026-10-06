//! Archivos canario: señuelos en disco cuya alteración delata a un intruso.
//!
//! Detecta **modificación, sustitución, borrado o pérdida de acceso**. No
//! detecta lecturas: ningún sistema de archivos ofrece de forma fiable y sin
//! privilegios un registro de accesos de lectura (los tiempos de acceso suelen
//! estar desactivados). Si un canario existe pero no puede leerse, se trata
//! como alteración (falla cerrado).

use crate::error::Resultado;
use crate::ids::eventos::{EventoDefensivo, NivelSeveridad, VectorAmenaza};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Señuelos sembrados: nombre de archivo y contenido verosímil pero falso.
const SENUELOS: &[(&str, &[u8])] = &[
    (
        "perfil_sesion.db",
        b"SQLite format 3\0\x10\0\x01\x01\0\x40\x20\x20-- tabla auth (usuario, token)\n",
    ),
    (
        "secring_backup.asc",
        b"-----BEGIN PGP PRIVATE KEY BLOCK-----\n\nlQOYBF9senuelo\n=falso\n-----END PGP PRIVATE KEY BLOCK-----\n",
    ),
    (
        "onion_auth.json",
        b"{\"auth_cookies\": {\"mercado.onion\": \"x-token-senuelo\"}}",
    ),
];

/// Estado de referencia de un canario.
#[derive(Debug, Clone)]
struct Canario {
    nombre: &'static str,
    ruta: PathBuf,
    contenido: &'static [u8],
    modificado: Option<SystemTime>,
}

/// Gestor de los canarios sembrados. Al destruirse, los borra.
#[derive(Debug)]
pub struct GestorCanarios {
    directorio: PathBuf,
    canarios: Vec<Canario>,
}

impl GestorCanarios {
    /// Siembra los señuelos en `directorio` (lo crea si no existe).
    ///
    /// # Errors
    /// [`crate::error::ErrorApp::Io`] si no se puede crear el directorio o escribir un señuelo.
    pub fn sembrar(directorio: &Path) -> Resultado<Self> {
        fs::create_dir_all(directorio)?;
        let canarios = SENUELOS
            .iter()
            .map(|&(nombre, contenido)| {
                let ruta = directorio.join(nombre);
                fs::write(&ruta, contenido)?;
                let modificado = fs::metadata(&ruta)?.modified().ok();
                Ok(Canario {
                    nombre,
                    ruta,
                    contenido,
                    modificado,
                })
            })
            .collect::<Resultado<Vec<_>>>()?;
        Ok(Self {
            directorio: directorio.to_path_buf(),
            canarios,
        })
    }

    /// Directorio de siembra.
    pub fn directorio(&self) -> &Path {
        &self.directorio
    }

    /// Comprueba los canarios; devuelve un evento crítico ante la primera alteración.
    pub fn verificar(&self) -> Option<EventoDefensivo> {
        self.canarios.iter().find_map(|canario| {
            alteracion(canario).map(|motivo| {
                EventoDefensivo::nuevo(
                    NivelSeveridad::Critico,
                    VectorAmenaza::CanarioAlterado(format!("{}: {motivo}", canario.nombre)),
                )
            })
        })
    }
}

/// Describe la alteración de un canario, si la hay.
fn alteracion(canario: &Canario) -> Option<String> {
    let contenido = match fs::read(&canario.ruta) {
        Ok(contenido) => contenido,
        Err(e) => return Some(format!("ilegible o eliminado ({e})")),
    };
    if contenido != canario.contenido {
        return Some("contenido modificado".to_string());
    }
    let modificado = fs::metadata(&canario.ruta).ok()?.modified().ok();
    (modificado != canario.modificado).then(|| "marca de modificación alterada".to_string())
}

impl Drop for GestorCanarios {
    fn drop(&mut self) {
        for canario in &self.canarios {
            if let Err(e) = fs::remove_file(&canario.ruta) {
                tracing::warn!(ruta = %canario.ruta.display(), error = %e, "no se pudo borrar un canario");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directorio_prueba(sufijo: &str) -> PathBuf {
        std::env::temp_dir().join(format!("wodw_canarios_{sufijo}_{}", std::process::id()))
    }

    #[test]
    fn siembra_limpia_no_genera_eventos_y_se_borra_al_soltar() {
        let dir = directorio_prueba("limpio");
        let gestor = GestorCanarios::sembrar(&dir).unwrap();
        assert!(gestor.verificar().is_none());
        drop(gestor);
        assert!(!dir.join("perfil_sesion.db").exists());
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn modificacion_y_borrado_generan_evento_critico() {
        let dir = directorio_prueba("alterado");
        let gestor = GestorCanarios::sembrar(&dir).unwrap();
        fs::write(dir.join("perfil_sesion.db"), b"alterado").unwrap();
        let evento = gestor.verificar().unwrap();
        assert_eq!(evento.severidad, NivelSeveridad::Critico);
        fs::write(dir.join("perfil_sesion.db"), SENUELOS[0].1).unwrap();
        fs::remove_file(dir.join("secring_backup.asc")).unwrap();
        assert!(gestor.verificar().is_some());
        drop(gestor);
        let _ = fs::remove_dir(&dir);
    }
}
