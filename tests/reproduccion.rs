//! Barreras de la reproducción en el Maestro: nada suena solo y todo se
//! detiene al navegar, purgar, recibir un incidente o pulsar el pánico
//! (CA-A4); la tarea productora valida cada bloque y trata lo hostil como
//! incidente (CA-A5). Sin red, sin procesos y sin tarjeta de sonido.

use wodw::configuracion::{ConfiguracionReproduccion, ConfiguracionWodw, ConfiguracionWorker};
use wodw::error::{ErrorApp, Resultado};
use wodw::ipc::mensajes::{FamiliaMedio, OrdenWorker, RespuestaWorker};
use wodw::maestro::medios::productor::{producir, CanalMedio};
use wodw::maestro::medios::{EstadoReproduccion, FaseReproduccion};
use wodw::maestro::navegacion::FuturoCaja;
use wodw::maestro::{ContenidoPagina, EventoSesion, FalloNavegacion, ResultadoNavegacion};
use wodw::ui::VentanaPrincipal;
use wodw::worker::EstadoWorker;

const MP3: &[u8] = include_bytes!("datos/tono.mp3");

/// Ventana sin sesión que muestra un audio en la pestaña activa.
fn ventana_con_audio() -> VentanaPrincipal {
    let mut cfg = ConfiguracionWodw::default();
    cfg.panico.abortar_proceso = false;
    let mut v = VentanaPrincipal::nueva(cfg, None, Box::new(|| {}));
    v.ir_a("http://a.onion/tono.mp3");
    let id = v.estado().pestana_activa().id();
    let solicitud = v.estado().pestana_activa().solicitud();
    v.atender_evento(EventoSesion::Navegacion {
        id_pestana: id,
        solicitud,
        resultado: Ok(ResultadoNavegacion {
            url: "http://a.onion/tono.mp3".to_string(),
            codigo_estado: 200,
            contenido: ContenidoPagina::Medio {
                familia: FamiliaMedio::Audio,
                tipo_mime: "audio/mpeg".to_string(),
                datos: MP3.to_vec(),
            },
        }),
    });
    v
}

#[test]
fn nada_suena_hasta_pulsar_reproducir() {
    let mut v = ventana_con_audio();
    v.vigilar_reproduccion();
    assert!(v.reproduccion_activa().is_none());
    v.reproducir();
    v.vigilar_reproduccion();
    let r = v.reproduccion_activa().expect("reproduce tras pulsar");
    assert!(!r.0.parado());
}

/// Comprueba que `accion` detiene una reproducción en curso.
fn se_detiene_con(accion: impl FnOnce(&mut VentanaPrincipal)) {
    let mut v = ventana_con_audio();
    v.reproducir();
    let manejador = v.reproduccion_activa().cloned().expect("reproduciendo");
    accion(&mut v);
    // Sin esperar al siguiente fotograma: la parada es inmediata.
    assert!(manejador.0.parado(), "la reproducción siguió");
    assert!(v.reproduccion_activa().is_none());
}

#[test]
fn se_detiene_al_navegar_a_otra_cosa() {
    se_detiene_con(|v| v.ir_a("http://b.onion/"));
}

#[test]
fn se_detiene_con_el_panico() {
    se_detiene_con(VentanaPrincipal::activar_panico);
}

#[test]
fn se_detiene_con_la_purga_por_inactividad() {
    se_detiene_con(|v| {
        v.comprobar_inactividad(std::time::Instant::now() + std::time::Duration::from_secs(31 * 60))
    });
}

#[test]
fn se_detiene_con_un_incidente_del_medio() {
    se_detiene_con(|v| {
        let id = v.estado().pestana_activa().id();
        v.atender_evento(EventoSesion::IncidenteReproduccion {
            id_pestana: id,
            fallo: FalloNavegacion {
                mensaje: "hostil".to_string(),
                purgar_pestana: true,
            },
        });
    });
}

/// Canal con el código real del Worker, en el mismo proceso.
struct CanalEnProceso(EstadoWorker);

impl CanalMedio for CanalEnProceso {
    fn pedir<'a>(
        &'a mut self,
        orden: &'a OrdenWorker,
    ) -> FuturoCaja<'a, Resultado<RespuestaWorker>> {
        let r = self
            .0
            .atender(orden.clone())
            .ok_or_else(|| ErrorApp::Proceso("sin respuesta".to_string()));
        Box::pin(async move { r })
    }
}

/// Worker comprometido: abre bien y luego entrega un bloque con picos.
struct CanalMentiroso;

impl CanalMedio for CanalMentiroso {
    fn pedir<'a>(
        &'a mut self,
        orden: &'a OrdenWorker,
    ) -> FuturoCaja<'a, Resultado<RespuestaWorker>> {
        let r = Ok(match orden {
            OrdenWorker::AbrirMedio { id_tarea, .. } => RespuestaWorker::MedioAbierto {
                id_tarea: *id_tarea,
                audio: true,
                video: None,
                duracion_ms: None,
            },
            _ => RespuestaWorker::BloqueMedio {
                id_tarea: 1,
                audio_pcm: vec![i16::MAX; 4],
                fotogramas: vec![],
                fin: false,
            },
        });
        Box::pin(async move { r })
    }
}

#[tokio::test]
async fn el_productor_llena_el_bufer_y_termina() {
    let estado = EstadoReproduccion::nuevo(100);
    let cfg_rep = ConfiguracionReproduccion {
        bufer_audio_ms: 10_000,
        ..Default::default()
    };
    let mut canal = CanalEnProceso(EstadoWorker::nuevo(ConfiguracionWorker::default()));
    producir(
        &mut canal,
        FamiliaMedio::Audio,
        MP3.to_vec(),
        &estado,
        &ConfiguracionWorker::default(),
        &cfg_rep,
    )
    .await
    .unwrap();
    assert_eq!(estado.fase(), FaseReproduccion::Reproduciendo);
    assert!(
        (950..1100).contains(&estado.audio_pendiente_ms()),
        "{}",
        estado.audio_pendiente_ms()
    );
    estado.avanzar_sin_dispositivo(2_000);
    assert!(estado.agotada());
}

#[tokio::test]
async fn bloque_hostil_detiene_y_es_incidente() {
    let estado = EstadoReproduccion::nuevo(100);
    let r = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        producir(
            &mut CanalMentiroso,
            FamiliaMedio::Audio,
            vec![],
            &estado,
            &ConfiguracionWorker::default(),
            &ConfiguracionReproduccion::default(),
        ),
    )
    .await
    .expect("el productor debe detenerse ante el primer bloque hostil");
    assert!(matches!(r, Err(ErrorApp::ContenidoHostil(_))), "{r:?}");
    assert!(estado.parado());
    assert!(matches!(estado.fase(), FaseReproduccion::Error(_)));
    assert_eq!(
        estado.audio_pendiente_ms(),
        0,
        "nada del bloque hostil llega al búfer"
    );
}

#[tokio::test]
async fn parada_previa_no_pide_bloques() {
    let estado = EstadoReproduccion::nuevo(100);
    estado.parar();
    let mut canal = CanalEnProceso(EstadoWorker::nuevo(ConfiguracionWorker::default()));
    producir(
        &mut canal,
        FamiliaMedio::Audio,
        MP3.to_vec(),
        &estado,
        &ConfiguracionWorker::default(),
        &ConfiguracionReproduccion::default(),
    )
    .await
    .unwrap();
    assert_eq!(estado.audio_pendiente_ms(), 0);
}
