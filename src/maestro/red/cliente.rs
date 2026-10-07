//! Cliente Tor embebido con aislamiento de flujos por pestaña.
//!
//! Cada pestaña obtiene su propio cliente aislado (`isolated_client`), de modo
//! que sus flujos nunca comparten circuito con los de otra pestaña. «Rotar el
//! aislamiento» descarta esos clientes: las peticiones siguientes de cada
//! pestaña viajan por circuitos nuevos. Los circuitos antiguos no se cierran de
//! inmediato (Arti no expone esa operación en su API estable), pero ya no
//! transportan tráfico de WoDW.

use super::configuracion_tor::construir_configuracion_tor;
use super::http::{ejecutar_peticion_en_flujo, PeticionHttp, RespuestaHttp};
use super::transporte::conectar_tls;
use crate::configuracion::{ConfiguracionRedHttp, ConfiguracionTor};
use crate::error::{ErrorApp, Resultado};
use arti_client::TorClient;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tor_rtcompat::PreferredRuntime;
use url::Url;

/// Puerto por omisión de HTTPS.
const PUERTO_HTTPS: u16 = 443;
/// Puerto por omisión de HTTP.
const PUERTO_HTTP: u16 = 80;

/// Cliente Tor concreto sobre el runtime de Tokio.
type ClienteArti = TorClient<PreferredRuntime>;

/// Estado del arranque (bootstrap) de Tor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EstadoArranque {
    /// Progreso entre 0 y 1.
    pub fraccion: f32,
    /// `true` cuando Tor está listo para tráfico de usuario.
    pub listo: bool,
}

/// Cliente Tor del Proceso Maestro.
pub struct ClienteTor {
    base: Arc<ClienteArti>,
    aislados: Mutex<HashMap<u64, Arc<ClienteArti>>>,
    red: ConfiguracionRedHttp,
}

impl std::fmt::Debug for ClienteTor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClienteTor").finish_non_exhaustive()
    }
}

impl ClienteTor {
    /// Crea el cliente sin conectarse todavía a la red Tor.
    ///
    /// # Errors
    /// [`ErrorApp::CircuitoTor`] si la configuración Tor es inválida y
    /// [`ErrorApp::RedArti`] si no hay runtime Tokio o Arti no puede crearse.
    pub fn nuevo_sin_arranque(
        tor: &ConfiguracionTor,
        red: ConfiguracionRedHttp,
    ) -> Resultado<Self> {
        let runtime = PreferredRuntime::current()
            .map_err(|e| ErrorApp::RedArti(format!("runtime no disponible: {e}")))?;
        let base = TorClient::with_runtime(runtime)
            .config(construir_configuracion_tor(tor)?)
            .create_unbootstrapped()
            .map_err(|e| ErrorApp::RedArti(format!("no se pudo crear el cliente Tor: {e}")))?;
        Ok(Self {
            base,
            aislados: Mutex::new(HashMap::new()),
            red,
        })
    }

    /// Conecta con la red Tor y descarga el consenso.
    ///
    /// # Errors
    /// [`ErrorApp::RedArti`] si el arranque falla.
    pub async fn arrancar(&self) -> Resultado<()> {
        self.base
            .bootstrap()
            .await
            .map_err(|e| ErrorApp::RedArti(format!("fallo en el arranque de Tor: {e}")))
    }

    /// Progreso actual del arranque.
    pub fn estado_arranque(&self) -> EstadoArranque {
        let estado = self.base.bootstrap_status();
        EstadoArranque {
            fraccion: estado.as_frac(),
            listo: estado.ready_for_traffic(),
        }
    }

    /// Descarta los clientes aislados de todas las pestañas.
    ///
    /// Devuelve cuántas pestañas tenían un aislamiento activo.
    pub fn rotar_aislamiento(&self) -> usize {
        let mut aislados = self.aislados.lock().unwrap_or_else(PoisonError::into_inner);
        let renovados = aislados.len();
        aislados.clear();
        renovados
    }

    /// Olvida el aislamiento de una pestaña cerrada.
    pub fn olvidar_pestana(&self, id_pestana: u64) {
        self.aislados
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id_pestana);
    }

    /// Cliente aislado de una pestaña; lo crea si no existe.
    pub(crate) fn cliente_de_pestana(&self, id_pestana: u64) -> Arc<ClienteArti> {
        self.aislados
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(id_pestana)
            .or_insert_with(|| self.base.isolated_client())
            .clone()
    }

    /// Descarga `url` por el circuito aislado de la pestaña.
    ///
    /// # Errors
    /// [`ErrorApp::EntradaInvalida`] si la URL no es http/https o viola
    /// `solo_onion`, [`ErrorApp::TiempoAgotado`] si la conexión no se abre a
    /// tiempo, y los errores de [`ejecutar_peticion_en_flujo`].
    pub async fn obtener(&self, id_pestana: u64, url: &Url) -> Resultado<RespuestaHttp> {
        let destino = Destino::desde_url(url, self.red.solo_onion)?;
        let peticion = PeticionHttp::get(&destino.host, destino.puerto, &destino.ruta)?;
        let cliente = self.cliente_de_pestana(id_pestana);
        let plazo = Duration::from_millis(self.red.tiempo_espera_conexion_ms);
        let flujo = tokio::time::timeout(
            plazo,
            cliente.connect((destino.host.as_str(), destino.puerto)),
        )
        .await
        .map_err(|_| ErrorApp::TiempoAgotado {
            operacion: "conexión Tor",
            milisegundos: self.red.tiempo_espera_conexion_ms,
        })?
        .map_err(|e| ErrorApp::RedArti(mensaje_conexion_fallida(&destino.host, &e.to_string())))?;
        if destino.cifrado {
            let mut tls = conectar_tls(flujo, &destino.host, plazo).await?;
            ejecutar_peticion_en_flujo(&mut tls, &peticion, &self.red).await
        } else {
            let mut flujo = flujo;
            ejecutar_peticion_en_flujo(&mut flujo, &peticion, &self.red).await
        }
    }
}

/// Explicaciones en español de los errores de conexión de Arti más comunes:
/// (fragmento del mensaje original, explicación).
const ERRORES_ARTI: &[(&str, &str)] = &[
    (
        "Invalid onion address",
        "la dirección .onion no es válida (revisa que esté completa)",
    ),
    (
        "Onion Service not found",
        "el servicio .onion no está publicado ahora mismo",
    ),
    (
        "descriptor",
        "no se encontró la descripción del servicio .onion: puede estar apagado",
    ),
    ("timed out", "se agotó el tiempo de espera"),
    ("Connection refused", "el servidor rechazó la conexión"),
    (
        "Could not connect",
        "no se pudo abrir el circuito con el servicio",
    ),
    (
        "not bootstrapped",
        "Tor todavía no ha terminado de conectar",
    ),
];

/// Mensaje de conexión fallida: la explicación en español si se conoce y,
/// entre paréntesis, el mensaje original de Arti para quien quiera el detalle.
pub fn mensaje_conexion_fallida(host: &str, original: &str) -> String {
    let explicacion = ERRORES_ARTI
        .iter()
        .find(|(fragmento, _)| original.contains(fragmento))
        .map_or("error de la red Tor", |(_, explicacion)| explicacion);
    format!("no se pudo conectar con {host}: {explicacion} (detalle técnico: {original})")
}

/// Destino de red extraído y validado de una URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destino {
    /// Host (nombre DNS o dirección onion).
    pub host: String,
    /// Puerto TCP.
    pub puerto: u16,
    /// Ruta y consulta ya codificadas.
    pub ruta: String,
    /// `true` para https.
    pub cifrado: bool,
}

impl Destino {
    /// Valida y descompone una URL.
    ///
    /// # Errors
    /// [`ErrorApp::EntradaInvalida`] si el esquema no es http/https, falta el
    /// host o el destino no es `.onion` con `solo_onion` activo.
    pub fn desde_url(url: &Url, solo_onion: bool) -> Resultado<Self> {
        let cifrado = match url.scheme() {
            "http" => false,
            "https" => true,
            otro => {
                return Err(ErrorApp::EntradaInvalida {
                    campo: "url",
                    motivo: format!("esquema «{otro}» no permitido"),
                })
            }
        };
        let host = url.host_str().ok_or_else(|| ErrorApp::EntradaInvalida {
            campo: "url",
            motivo: "falta el host".to_string(),
        })?;
        if solo_onion && !host.ends_with(".onion") {
            return Err(ErrorApp::EntradaInvalida {
                campo: "url",
                motivo: "la configuración solo permite destinos .onion".to_string(),
            });
        }
        let puerto =
            url.port_or_known_default()
                .unwrap_or(if cifrado { PUERTO_HTTPS } else { PUERTO_HTTP });
        let ruta = match url.query() {
            Some(consulta) => format!("{}?{consulta}", url.path()),
            None => url.path().to_string(),
        };
        Ok(Self {
            host: host.to_string(),
            puerto,
            ruta,
            cifrado,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rotar_renueva_el_aislamiento_de_cada_pestana() {
        // Directorios temporales propios: la prueba no toca los de Arti del usuario.
        let base = std::env::temp_dir().join(format!("wodw_arti_{}", std::process::id()));
        let tor = ConfiguracionTor {
            ruta_estado: Some(base.join("estado")),
            ruta_cache: Some(base.join("cache")),
            ..Default::default()
        };
        let cliente =
            ClienteTor::nuevo_sin_arranque(&tor, ConfiguracionRedHttp::default()).unwrap();
        let antes = cliente.cliente_de_pestana(1);
        assert!(Arc::ptr_eq(&antes, &cliente.cliente_de_pestana(1)));
        assert!(!Arc::ptr_eq(&antes, &cliente.cliente_de_pestana(2)));
        assert_eq!(cliente.rotar_aislamiento(), 2);
        assert!(!Arc::ptr_eq(&antes, &cliente.cliente_de_pestana(1)));
        drop(cliente);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn destino_rechaza_esquemas_y_respeta_solo_onion() {
        let ftp = Url::parse("ftp://a.onion/").unwrap();
        assert!(Destino::desde_url(&ftp, false).is_err());
        let clara = Url::parse("http://example.com/").unwrap();
        assert!(Destino::desde_url(&clara, true).is_err());
        let d = Destino::desde_url(&Url::parse("https://x.onion/a?b=c d").unwrap(), true).unwrap();
        assert_eq!(
            (d.puerto, d.ruta.as_str(), d.cifrado),
            (443, "/a?b=c%20d", true)
        );
    }
}
