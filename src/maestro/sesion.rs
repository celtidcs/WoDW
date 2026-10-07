//! Sesión del Maestro en segundo plano: une Tor, IDS, navegación y vigilancia.
//!
//! La interfaz envía [`OrdenSesion`] y recibe [`EventoSesion`] sin bloquearse:
//! todo el trabajo de red y de procesos ocurre en un runtime de Tokio propio en
//! otro hilo. Al soltar la [`ConexionSesion`], el runtime se detiene y
//! los sub-Workers en curso se destruyen (`kill_on_drop`).

use crate::configuracion::{ConfiguracionActualizaciones, ConfiguracionWodw};
use crate::error::Resultado;
use crate::ids::eventos::EventoDefensivo;
use crate::ids::motor::{ControlIds, EmisorIds, MotorIds};
use crate::ipc::mensajes::FamiliaMedio;
use crate::maestro::medios::productor::producir;
use crate::maestro::medios::{FaseReproduccion, ManejadorReproduccion};
use crate::maestro::navegacion::incidentes::RespuestaAutomatica;
use crate::maestro::navegacion::{FalloNavegacion, ResultadoNavegacion, ServicioNavegacion};
use crate::maestro::proceso_worker::ProcesadorSubworker;
use crate::maestro::red::ClienteTor;
use crate::maestro::versiones::{interpretar_ultima_version, VersionNueva};
use crate::seguridad::{GestorCanarios, HoneypotRam, TrampaMemoria};
use std::path::PathBuf;
use std::sync::{mpsc as canal_std, Arc};
use std::time::Duration;
use tokio::sync::mpsc;

/// Intervalo de sondeo del progreso de arranque de Tor mostrado en la interfaz.
const INTERVALO_SONDEO_ARRANQUE: Duration = Duration::from_millis(500);
/// Etiqueta de la trampa de memoria.
const ETIQUETA_TRAMPA: &str = "clave_privada_sesion";
/// Cebo de la trampa de memoria (verosímil, sin valor real).
const CEBO_TRAMPA: &[u8] = b"-----BEGIN OPENSSH PRIVATE KEY-----senuelo";
/// Subdirectorio temporal por defecto de los canarios.
const SUBDIRECTORIO_CANARIOS: &str = "wodw-canarios";

/// Orden de la interfaz a la sesión.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrdenSesion {
    /// Navegar desde una pestaña; `solicitud` permite descartar respuestas antiguas.
    Navegar {
        /// Pestaña de origen.
        id_pestana: u64,
        /// Número de solicitud de la pestaña.
        solicitud: u64,
        /// URL absoluta.
        direccion: String,
    },
    /// Una pestaña se cerró: olvidar su aislamiento.
    CerrarPestana(u64),
    /// Rotar el aislamiento de todas las pestañas.
    RotarAislamiento,
    /// Purga de sesión: rota el aislamiento y olvida los hosts bloqueados.
    Purgar,
    /// Reproducir un medio ya descargado en un sub-Worker sin red dedicado.
    Reproducir {
        /// Pestaña que lo muestra.
        id_pestana: u64,
        /// Host de origen (para la respuesta automática si el medio es hostil).
        host: String,
        /// Familia declarada.
        familia: FamiliaMedio,
        /// Bytes del archivo.
        datos: Vec<u8>,
        /// Estado compartido con la interfaz.
        reproduccion: ManejadorReproduccion,
    },
}

/// Estado del arranque de Tor.
#[derive(Debug, Clone, PartialEq)]
pub enum EstadoTor {
    /// Conectando; fracción de progreso entre 0 y 1.
    Conectando(f32),
    /// Listo para navegar.
    Listo,
    /// Fallo de arranque.
    Error(String),
}

/// Evento de la sesión hacia la interfaz.
#[derive(Debug, Clone, PartialEq)]
pub enum EventoSesion {
    /// Cambio de estado de Tor.
    Tor(EstadoTor),
    /// Resultado de una navegación.
    Navegacion {
        /// Pestaña de origen.
        id_pestana: u64,
        /// Número de solicitud.
        solicitud: u64,
        /// Resultado.
        resultado: Result<ResultadoNavegacion, FalloNavegacion>,
    },
    /// Aislamiento rotado automáticamente o a petición.
    AislamientoRotado,
    /// El IDS ordenó el pánico automático.
    PanicoAutomatico,
    /// Hay una versión de WoDW más reciente que la instalada.
    VersionNueva(VersionNueva),
    /// Un medio resultó hostil: respuesta automática ya aplicada.
    IncidenteReproduccion {
        /// Pestaña afectada.
        id_pestana: u64,
        /// Fallo tratado.
        fallo: FalloNavegacion,
    },
}

/// Emisor de eventos hacia la interfaz que además solicita repintado.
#[derive(Clone)]
struct AvisoInterfaz {
    tx: canal_std::Sender<EventoSesion>,
    repintar: Arc<dyn Fn() + Send + Sync>,
}

impl AvisoInterfaz {
    fn enviar(&self, evento: EventoSesion) {
        if self.tx.send(evento).is_ok() {
            (self.repintar)();
        }
    }
}

/// Servicio de navegación de producción.
type ServicioReal = ServicioNavegacion<Arc<ClienteTor>, ProcesadorSubworker>;

/// Extremo de la sesión que posee la interfaz.
pub struct ConexionSesion {
    ordenes: mpsc::UnboundedSender<OrdenSesion>,
    eventos: canal_std::Receiver<EventoSesion>,
    control_ids: ControlIds,
    _hilo: std::thread::JoinHandle<()>,
}

impl ConexionSesion {
    /// Envía una orden a la sesión (no bloquea).
    pub fn ordenar(&self, orden: OrdenSesion) {
        if self.ordenes.send(orden).is_err() {
            tracing::error!("la sesión del Maestro ya no está activa");
        }
    }

    /// Siguiente evento pendiente, sin bloquear.
    pub fn siguiente_evento(&self) -> Option<EventoSesion> {
        self.eventos.try_recv().ok()
    }

    /// Control del IDS para la telemetría.
    pub fn control_ids(&self) -> &ControlIds {
        &self.control_ids
    }
}

/// Arranca la sesión en un hilo propio.
///
/// `ejecutable` es el binario de WoDW usado para los sub-Workers y `repintar`
/// se invoca tras cada evento para que la interfaz se actualice.
///
/// # Errors
/// Errores al crear el runtime, el cliente Tor o la configuración del Worker.
pub fn iniciar_sesion(
    cfg: ConfiguracionWodw,
    ejecutable: PathBuf,
    repintar: impl Fn() + Send + Sync + 'static,
) -> Resultado<ConexionSesion> {
    let runtime = tokio::runtime::Runtime::new()?;
    let (tx_eventos, eventos) = canal_std::channel();
    let aviso = AvisoInterfaz {
        tx: tx_eventos,
        repintar: Arc::new(repintar),
    };
    let (ordenes, rx_ordenes) = mpsc::unbounded_channel();
    let (control_ids, nucleo) =
        runtime.block_on(async { construir_nucleo(&cfg, ejecutable, aviso.clone()) })?;
    let hilo = std::thread::Builder::new()
        .name("wodw-sesion".to_string())
        .spawn(move || runtime.block_on(ejecutar(nucleo, cfg, aviso, rx_ordenes)))?;
    Ok(ConexionSesion {
        ordenes,
        eventos,
        control_ids,
        _hilo: hilo,
    })
}

/// Componentes vivos de la sesión.
struct Nucleo {
    cliente: Arc<ClienteTor>,
    servicio: Arc<ServicioReal>,
    emisor: EmisorIds,
    trampa: TrampaMemoria,
    _honeypot: Option<HoneypotRam>,
    canarios: Option<GestorCanarios>,
}

/// Crea cliente Tor, IDS con contramedidas, servicio y señuelos (dentro del runtime).
fn construir_nucleo(
    cfg: &ConfiguracionWodw,
    ejecutable: PathBuf,
    aviso: AvisoInterfaz,
) -> Resultado<(ControlIds, Nucleo)> {
    let cliente = Arc::new(ClienteTor::nuevo_sin_arranque(&cfg.tor, cfg.red.clone())?);
    let para_rotar = cliente.clone();
    let aviso_rotacion = aviso.clone();
    let (control_ids, _tarea) = MotorIds::nuevo(cfg.ids.clone())
        .con_rotacion(move || {
            para_rotar.rotar_aislamiento();
            aviso_rotacion.enviar(EventoSesion::AislamientoRotado);
        })
        .con_panico(move || aviso.enviar(EventoSesion::PanicoAutomatico))
        .iniciar();
    let emisor = control_ids.emisor();
    let respuesta = RespuestaAutomatica::nueva(
        cfg.automatizacion.bloquear_hosts_hostiles,
        Some(emisor.clone()),
    );
    let procesador = ProcesadorSubworker::nuevo(ejecutable, cfg.worker.clone())?;
    let servicio = Arc::new(
        ServicioNavegacion::nuevo(
            cliente.clone(),
            procesador,
            respuesta,
            cfg.red.max_redirecciones,
        )
        .con_limites(&cfg.worker),
    );
    let nucleo = Nucleo {
        cliente,
        servicio,
        emisor,
        trampa: TrampaMemoria::nueva(ETIQUETA_TRAMPA, CEBO_TRAMPA),
        _honeypot: HoneypotRam::nueva_pagina_protegida()
            .map_err(|e| tracing::warn!(error = %e, "sin página honeypot"))
            .ok(),
        canarios: sembrar_canarios(cfg)?,
    };
    Ok((control_ids, nucleo))
}

/// Siembra los canarios si están habilitados.
fn sembrar_canarios(cfg: &ConfiguracionWodw) -> Resultado<Option<GestorCanarios>> {
    if !cfg.canarios.habilitados {
        return Ok(None);
    }
    let directorio = cfg
        .canarios
        .directorio
        .clone()
        .unwrap_or_else(|| std::env::temp_dir().join(SUBDIRECTORIO_CANARIOS));
    GestorCanarios::sembrar(&directorio).map(Some)
}

/// Bucle de la sesión: lanza tareas de fondo y atiende órdenes.
async fn ejecutar(
    nucleo: Nucleo,
    cfg: ConfiguracionWodw,
    aviso: AvisoInterfaz,
    mut ordenes: mpsc::UnboundedReceiver<OrdenSesion>,
) {
    tokio::spawn(arrancar_tor(
        nucleo.cliente.clone(),
        aviso.clone(),
        cfg.actualizaciones.clone(),
    ));
    let intervalo = Duration::from_millis(cfg.ids.intervalo_verificacion_ms);
    let vigilancia = tokio::spawn(vigilar_senuelos(
        nucleo.trampa.clone(),
        nucleo.canarios,
        nucleo.emisor.clone(),
        intervalo,
    ));
    let cfg = Arc::new(cfg);
    while let Some(orden) = ordenes.recv().await {
        atender(orden, &nucleo.servicio, &aviso, &cfg);
    }
    vigilancia.abort();
}

/// Atiende una orden de la interfaz.
fn atender(
    orden: OrdenSesion,
    servicio: &Arc<ServicioReal>,
    aviso: &AvisoInterfaz,
    cfg: &Arc<ConfiguracionWodw>,
) {
    match orden {
        OrdenSesion::Navegar {
            id_pestana,
            solicitud,
            direccion,
        } => {
            let servicio = servicio.clone();
            let aviso = aviso.clone();
            tokio::spawn(async move {
                let resultado = servicio.navegar(id_pestana, &direccion).await;
                aviso.enviar(EventoSesion::Navegacion {
                    id_pestana,
                    solicitud,
                    resultado,
                });
            });
        }
        OrdenSesion::CerrarPestana(id) => servicio.fuente().olvidar_pestana(id),
        OrdenSesion::RotarAislamiento => {
            servicio.fuente().rotar_aislamiento();
            aviso.enviar(EventoSesion::AislamientoRotado);
        }
        OrdenSesion::Purgar => {
            servicio.fuente().rotar_aislamiento();
            servicio.respuesta_automatica().olvidar_bloqueos();
        }
        OrdenSesion::Reproducir {
            id_pestana,
            host,
            familia,
            datos,
            reproduccion,
        } => {
            let peticion = PeticionReproduccion {
                id_pestana,
                host,
                familia,
                datos,
                reproduccion,
            };
            tokio::spawn(reproducir(
                servicio.clone(),
                aviso.clone(),
                cfg.clone(),
                peticion,
            ));
        }
    }
}

/// Un medio que la interfaz pide reproducir, con la pestaña y el sitio de los
/// que procede (para atribuirle un incidente si resulta hostil).
struct PeticionReproduccion {
    id_pestana: u64,
    host: String,
    familia: FamiliaMedio,
    datos: Vec<u8>,
    reproduccion: ManejadorReproduccion,
}

/// Reproduce un medio en su propio sub-Worker y trata como incidente lo hostil.
async fn reproducir(
    servicio: Arc<ServicioReal>,
    aviso: AvisoInterfaz,
    cfg: Arc<ConfiguracionWodw>,
    peticion: PeticionReproduccion,
) {
    let PeticionReproduccion {
        id_pestana,
        host,
        familia,
        datos,
        reproduccion,
    } = peticion;
    let estado = &reproduccion.0;
    let resultado = match servicio.procesador().canal_medio() {
        Ok(mut canal) => {
            producir(
                &mut canal,
                familia,
                datos,
                estado,
                &cfg.worker,
                &cfg.reproduccion,
            )
            .await
        }
        Err(e) => {
            estado.fijar_fase(FaseReproduccion::Error(e.to_string()));
            Err(e)
        }
    };
    if let Err(error) = resultado {
        let fallo = servicio.responder_a_fallo(&host, error);
        if fallo.purgar_pestana {
            aviso.enviar(EventoSesion::IncidenteReproduccion { id_pestana, fallo });
        }
    }
}

/// Arranca Tor e informa del progreso; conectado, consulta si hay versión nueva.
async fn arrancar_tor(
    cliente: Arc<ClienteTor>,
    aviso: AvisoInterfaz,
    actualizaciones: ConfiguracionActualizaciones,
) {
    let sondeo = {
        let cliente = cliente.clone();
        let aviso = aviso.clone();
        tokio::spawn(async move {
            loop {
                let estado = cliente.estado_arranque();
                if estado.listo {
                    return;
                }
                aviso.enviar(EventoSesion::Tor(EstadoTor::Conectando(estado.fraccion)));
                tokio::time::sleep(INTERVALO_SONDEO_ARRANQUE).await;
            }
        })
    };
    let resultado = cliente.arrancar().await;
    sondeo.abort();
    let conectado = resultado.is_ok();
    aviso.enviar(EventoSesion::Tor(match resultado {
        Ok(()) => EstadoTor::Listo,
        Err(e) => EstadoTor::Error(e.to_string()),
    }));
    if conectado && actualizaciones.comprobar_al_iniciar {
        comprobar_version(&cliente, &aviso, &actualizaciones).await;
    }
}

/// Aislamiento propio de la consulta de versiones: no comparte circuito con
/// ninguna pestaña (los identificadores de pestaña empiezan en 1).
const AISLAMIENTO_VERSIONES: u64 = u64::MAX;

/// Consulta por Tor la última versión publicada y avisa si es posterior.
/// Cualquier fallo solo se anota en el registro de depuración: no es un
/// incidente ni molesta al usuario.
async fn comprobar_version(
    cliente: &ClienteTor,
    aviso: &AvisoInterfaz,
    cfg: &ConfiguracionActualizaciones,
) {
    let resultado = async {
        let url = url::Url::parse(&cfg.url_api).map_err(|e| e.to_string())?;
        let respuesta = cliente
            .obtener(AISLAMIENTO_VERSIONES, &url)
            .await
            .map_err(|e| e.to_string())?;
        interpretar_ultima_version(respuesta.cuerpo(), env!("CARGO_PKG_VERSION"), cfg)
    }
    .await;
    match resultado {
        Ok(Some(nueva)) => aviso.enviar(EventoSesion::VersionNueva(nueva)),
        Ok(None) => {}
        Err(e) => tracing::info!(error = %e, "no se pudo comprobar la versión"),
    }
}

/// Verifica periódicamente la trampa de memoria y los canarios.
async fn vigilar_senuelos(
    trampa: TrampaMemoria,
    canarios: Option<GestorCanarios>,
    emisor: EmisorIds,
    intervalo: Duration,
) {
    let mut reloj = tokio::time::interval(intervalo);
    loop {
        reloj.tick().await;
        let alteraciones: Vec<EventoDefensivo> = trampa
            .verificar_integridad()
            .into_iter()
            .chain(canarios.as_ref().and_then(GestorCanarios::verificar))
            .collect();
        for evento in alteraciones {
            if let Err(e) = emisor.emitir(evento).await {
                tracing::error!(error = %e, "vigilancia de señuelos sin IDS");
                return;
            }
        }
    }
}
