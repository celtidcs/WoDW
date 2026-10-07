//! Ventana principal: une el estado del navegador, la sesión del Maestro y los
//! paneles de `egui`.
//!
//! La ventana reacciona sola, sin que el usuario tenga que pulsar nada: aplica
//! los resultados de la sesión, ejecuta el pánico automático que ordene el IDS,
//! purga por inactividad y purga al cerrarse.
//!
//! Piezas: este módulo lleva el estado de la ventana y su reacción a los
//! eventos; `anotaciones` pasa al registro lo que ocurre y `dibujo` pinta
//! cada fotograma y recoge lo que pulsa el usuario.

mod anotaciones;
mod dibujo;

use crate::configuracion::ConfiguracionWodw;
use crate::maestro::medios::ManejadorReproduccion;
use crate::maestro::navegacion::ContenidoPagina;
use crate::maestro::sesion::{ConexionSesion, EventoSesion, OrdenSesion};
use crate::maestro::versiones::VersionNueva;
use crate::registro::{CategoriaRegistro, RegistroSesion};
use crate::ui::contenido::CacheImagen;
use crate::ui::estado::EstadoContenido;
use crate::ui::estado::{EstadoNavegador, ParametrosEstado, SolicitudNavegacion};
use crate::ui::motores::CatalogoMotores;
use crate::ui::panel_accesos::PanelAccesos;
use crate::ui::panel_registro::PanelRegistro;
use crate::ui::reproductor::Reproductor;
use crate::ui::textos;
use std::time::{Duration, Instant};

/// Acción ejecutada tras la purga del pánico cuando `panico.abortar_proceso` es `true`.
pub type AccionSalida = Box<dyn Fn()>;

/// Ventana principal de WoDW.
pub struct VentanaPrincipal {
    estado: EstadoNavegador,
    catalogo: CatalogoMotores,
    sesion: Option<ConexionSesion>,
    cfg: ConfiguracionWodw,
    mensaje_estado: String,
    mostrar_telemetria: bool,
    cache_imagen: CacheImagen,
    reproductor: Reproductor,
    version_nueva: Option<VersionNueva>,
    registro: RegistroSesion,
    panel_registro: PanelRegistro,
    panel_accesos: PanelAccesos,
    /// Eventos del IDS ya pasados al registro.
    eventos_ids_registrados: u64,
    accion_salida: AccionSalida,
}

/// Nombre del archivo de registro por defecto, junto al ejecutable.
const ARCHIVO_REGISTRO: &str = "wodw-registro.txt";

/// Ruta de guardado del registro: la configurada o junto al ejecutable.
fn ruta_registro(cfg: &ConfiguracionWodw) -> std::path::PathBuf {
    cfg.registro.ruta_automatica.clone().unwrap_or_else(|| {
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|d| d.join(ARCHIVO_REGISTRO)))
            .unwrap_or_else(|| std::path::PathBuf::from(ARCHIVO_REGISTRO))
    })
}

impl VentanaPrincipal {
    /// Crea la ventana. Sin sesión, la interfaz funciona pero no navega.
    pub fn nueva(
        cfg: ConfiguracionWodw,
        sesion: Option<ConexionSesion>,
        accion_salida: AccionSalida,
    ) -> Self {
        let minutos = cfg.automatizacion.minutos_inactividad_purga;
        let parametros = ParametrosEstado {
            pulsaciones_panico: cfg.panico.pulsaciones,
            ventana_panico: Duration::from_millis(cfg.panico.ventana_ms),
            inactividad_purga: (minutos > 0)
                .then(|| Duration::from_secs(minutos * crate::unidades::SEGUNDOS_POR_MINUTO)),
        };
        let mensaje_estado = if sesion.is_some() {
            String::new()
        } else {
            textos::SIN_SESION.to_string()
        };
        let registro = RegistroSesion::nuevo(&cfg.registro);
        let panel_registro = PanelRegistro::nuevo(ruta_registro(&cfg).display().to_string());
        Self {
            estado: EstadoNavegador::nuevo(parametros, Instant::now()),
            catalogo: CatalogoMotores::desde_configuracion(&cfg.motores),
            sesion,
            cfg,
            mensaje_estado,
            mostrar_telemetria: false,
            cache_imagen: CacheImagen::default(),
            reproductor: Reproductor::default(),
            version_nueva: None,
            registro,
            panel_registro,
            panel_accesos: PanelAccesos::default(),
            eventos_ids_registrados: 0,
            accion_salida,
        }
    }

    /// Estado del navegador (solo lectura).
    pub fn estado(&self) -> &EstadoNavegador {
        &self.estado
    }

    /// Mensaje de la barra de estado.
    pub fn mensaje_estado(&self) -> &str {
        &self.mensaje_estado
    }

    /// Navega a lo escrito en la barra (URL o búsqueda).
    pub fn ir_a(&mut self, entrada: &str) {
        if let Some(direccion) = self.catalogo.resolver_entrada(entrada) {
            let solicitud = self.estado.navegar(direccion);
            self.enviar(solicitud);
            self.vigilar_reproduccion();
        }
    }

    fn enviar(&self, solicitud: SolicitudNavegacion) {
        if let Some(sesion) = &self.sesion {
            sesion.ordenar(OrdenSesion::Navegar {
                id_pestana: solicitud.id_pestana,
                solicitud: solicitud.solicitud,
                direccion: solicitud.direccion,
            });
        }
    }

    /// Pánico: purga todo, renueva circuitos, olvida bloqueos y sale si está configurado.
    pub fn activar_panico(&mut self) {
        // El pánico nunca guarda el registro: lo borra.
        self.registro.vaciar();
        self.purgar_sesion();
        if self.cfg.panico.abortar_proceso {
            (self.accion_salida)();
        }
    }

    fn purgar_sesion(&mut self) {
        self.reproductor.parar();
        self.estado.purgar_todo();
        self.cache_imagen.vaciar();
        if let Some(sesion) = &self.sesion {
            sesion.ordenar(OrdenSesion::Purgar);
        }
    }

    /// Aplica los eventos pendientes de la sesión.
    pub fn atender_eventos_sesion(&mut self) {
        let eventos: Vec<EventoSesion> = self
            .sesion
            .as_ref()
            .map(|s| std::iter::from_fn(|| s.siguiente_evento()).collect())
            .unwrap_or_default();
        for evento in eventos {
            self.atender_evento(evento);
        }
    }

    /// Aplica un evento de la sesión.
    pub fn atender_evento(&mut self, evento: EventoSesion) {
        match evento {
            EventoSesion::Tor(estado) => {
                if !matches!(estado, crate::maestro::sesion::EstadoTor::Conectando(_)) {
                    self.registro
                        .anotar(CategoriaRegistro::Tor, textos::estado_tor(&estado));
                }
                self.mensaje_estado = textos::estado_tor(&estado);
            }
            EventoSesion::Navegacion {
                id_pestana,
                solicitud,
                resultado,
            } => {
                self.anotar_navegacion(&resultado);
                self.estado
                    .aplicar_resultado(id_pestana, solicitud, resultado);
            }
            EventoSesion::AislamientoRotado => {
                self.registro
                    .anotar(CategoriaRegistro::Tor, textos::AISLAMIENTO_ROTADO);
                self.mensaje_estado = textos::AISLAMIENTO_ROTADO.to_string()
            }
            EventoSesion::PanicoAutomatico => self.activar_panico(),
            EventoSesion::VersionNueva(nueva) => {
                self.registro.anotar(
                    CategoriaRegistro::Version,
                    textos::aviso_version(&nueva.version),
                );
                self.version_nueva = Some(nueva);
            }
            EventoSesion::IncidenteReproduccion { id_pestana, fallo } => {
                self.registro
                    .anotar(CategoriaRegistro::Medio, fallo.mensaje.clone());
                self.reproductor.parar();
                self.estado.purgar_pestana_por_incidente(id_pestana, fallo);
            }
        }
    }

    /// Reproduce el audio o vídeo de la pestaña activa (lo que hace el botón
    /// «Reproducir»). Sin medio a la vista no hace nada.
    pub fn reproducir(&mut self) {
        let pestana = self.estado.pestana_activa();
        let EstadoContenido::Pagina(ContenidoPagina::Medio { familia, datos, .. }) =
            pestana.contenido()
        else {
            return;
        };
        let (familia, datos) = (*familia, datos.clone());
        let host = url::Url::parse(pestana.url())
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_default();
        let (id_pestana, solicitud) = (pestana.id(), pestana.solicitud());
        let anotacion = textos::anotacion_reproduccion(pestana.url());
        self.registro
            .anotar(CategoriaRegistro::Reproduccion, anotacion);
        let manejador = self.reproductor.iniciar(
            id_pestana,
            solicitud,
            self.cfg.reproduccion.volumen_inicial_por_ciento,
            self.sesion.is_some(),
        );
        if let Some(sesion) = &self.sesion {
            sesion.ordenar(OrdenSesion::Reproducir {
                id_pestana,
                host,
                familia,
                datos,
                reproduccion: manejador,
            });
        }
    }

    /// Versión nueva anunciada por la sesión, si la hay.
    pub fn version_nueva(&self) -> Option<&VersionNueva> {
        self.version_nueva.as_ref()
    }

    /// Reproducción en curso, si la hay.
    pub fn reproduccion_activa(&self) -> Option<&ManejadorReproduccion> {
        self.reproductor.activa()
    }

    /// Detiene la reproducción si ya no corresponde a lo que muestra la pestaña
    /// activa (otra pestaña, otra navegación o pestaña cerrada). Se ejecuta en
    /// cada fotograma.
    pub fn vigilar_reproduccion(&mut self) {
        let pestana = self.estado.pestana_activa();
        let muestra_medio = matches!(
            pestana.contenido(),
            EstadoContenido::Pagina(ContenidoPagina::Medio { .. })
        );
        self.reproductor
            .vigilar(pestana.id(), pestana.solicitud(), muestra_medio);
    }

    /// Abre o cierra el panel de accesos directos (lo que hace el botón «Accesos»).
    pub fn alternar_accesos(&mut self) {
        self.panel_accesos.abierto = !self.panel_accesos.abierto;
    }

    /// `true` si el panel de accesos directos está abierto.
    pub fn accesos_abiertos(&self) -> bool {
        self.panel_accesos.abierto
    }

    /// Abre el acceso directo `indice` en la pestaña activa (lo que hace pulsar
    /// uno en el panel). Devuelve `false` si no existe.
    pub fn abrir_acceso(&mut self, indice: usize) -> bool {
        let Some(url) = self.cfg.accesos.lista.get(indice).map(|a| a.url.clone()) else {
            return false;
        };
        self.panel_accesos.abierto = false;
        self.ir_a(&url);
        true
    }

    /// Registro de la sesión (solo lectura).
    pub fn registro(&self) -> &RegistroSesion {
        &self.registro
    }

    /// Cierre normal de la aplicación: guarda el registro si el guardado es
    /// Automático y después purga la sesión.
    pub fn al_cerrar(&mut self) {
        let ruta = std::path::PathBuf::from(self.panel_registro.ruta());
        if let Err(e) = self.registro.al_cerrar(&ruta) {
            tracing::error!(error = %e, "no se pudo guardar el registro al cerrar");
        }
        self.registro.vaciar();
        self.purgar_sesion();
    }

    /// Purga por inactividad si ha vencido el plazo.
    pub fn comprobar_inactividad(&mut self, ahora: Instant) {
        if self.estado.inactividad_vencida(ahora) {
            self.registro.vaciar();
            self.registro
                .anotar(CategoriaRegistro::Purga, textos::PURGA_INACTIVIDAD);
            self.purgar_sesion();
            self.mensaje_estado = textos::PURGA_INACTIVIDAD.to_string();
        }
    }
}
