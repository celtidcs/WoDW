//! Catálogo de motores de búsqueda y resolución de la barra de direcciones.
//!
//! Los motores vienen de la configuración (`[motores]` en `wodw.toml`): añadir
//! uno no requiere tocar el código (OCP).

use crate::configuracion::{ConfiguracionMotores, MotorConfigurado};

/// Marcador de la plantilla sustituido por la consulta codificada.
const MARCADOR_CONSULTA: &str = "{consulta}";

/// Catálogo de motores con uno seleccionado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogoMotores {
    motores: Vec<MotorConfigurado>,
    seleccionado: usize,
}

impl CatalogoMotores {
    /// Construye el catálogo seleccionando el predeterminado (o el primero).
    pub fn desde_configuracion(cfg: &ConfiguracionMotores) -> Self {
        let seleccionado = cfg
            .lista
            .iter()
            .position(|m| m.nombre == cfg.predeterminado)
            .unwrap_or(0);
        Self {
            motores: cfg.lista.clone(),
            seleccionado,
        }
    }

    /// Motores disponibles.
    pub fn motores(&self) -> &[MotorConfigurado] {
        &self.motores
    }

    /// Índice del motor seleccionado.
    pub fn seleccionado(&self) -> usize {
        self.seleccionado
    }

    /// Nombre del motor seleccionado.
    pub fn nombre_seleccionado(&self) -> &str {
        self.motores
            .get(self.seleccionado)
            .map_or("", |m| m.nombre.as_str())
    }

    /// Selecciona el motor `indice` si existe.
    pub fn seleccionar(&mut self, indice: usize) {
        if indice < self.motores.len() {
            self.seleccionado = indice;
        }
    }

    /// Convierte la entrada de la barra en una URL absoluta:
    /// - `http://…` o `https://…` se respetan;
    /// - un host `.onion` sin esquema recibe `http://`;
    /// - cualquier otra cosa se busca con el motor seleccionado.
    ///
    /// Devuelve `None` si la entrada está vacía.
    pub fn resolver_entrada(&self, entrada: &str) -> Option<String> {
        let limpia = entrada.trim();
        if limpia.is_empty() {
            return None;
        }
        if limpia.starts_with("http://") || limpia.starts_with("https://") {
            return Some(limpia.to_string());
        }
        let host = limpia.split(['/', '?', '#']).next().unwrap_or_default();
        if host.ends_with(".onion") && !limpia.contains(char::is_whitespace) {
            return Some(format!("http://{limpia}"));
        }
        let motor = self.motores.get(self.seleccionado)?;
        Some(
            motor
                .plantilla
                .replace(MARCADOR_CONSULTA, &codificar_componente(limpia)),
        )
    }
}

/// Codifica una consulta para un parámetro de URL (`application/x-www-form-urlencoded`).
fn codificar_componente(texto: &str) -> String {
    let mut resultado = String::with_capacity(texto.len());
    for byte in texto.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                resultado.push(char::from(byte));
            }
            b' ' => resultado.push('+'),
            otro => resultado.push_str(&format!("%{otro:02X}")),
        }
    }
    resultado
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogo() -> CatalogoMotores {
        CatalogoMotores::desde_configuracion(&ConfiguracionMotores::default())
    }

    #[test]
    fn busqueda_usa_la_plantilla_del_motor_seleccionado() {
        let mut c = catalogo();
        assert_eq!(c.nombre_seleccionado(), "Ahmia");
        let url = c.resolver_entrada("seguridad anonimato").unwrap();
        assert!(url.ends_with("/search/?q=seguridad+anonimato"), "{url}");
        c.seleccionar(1);
        assert!(c
            .resolver_entrada("c++ & rust")
            .unwrap()
            .ends_with("/html/?q=c%2B%2B+%26+rust"));
    }

    #[test]
    fn direcciones_directas_se_respetan() {
        let c = catalogo();
        assert_eq!(c.resolver_entrada("  "), None);
        assert_eq!(
            c.resolver_entrada("https://x.onion/a").unwrap(),
            "https://x.onion/a"
        );
        assert_eq!(
            c.resolver_entrada("x.onion/ruta").unwrap(),
            "http://x.onion/ruta"
        );
        assert!(c
            .resolver_entrada("evil.com/?.onion/")
            .unwrap()
            .contains("/search/?q="));
    }
}
