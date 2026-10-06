//! Navegación y respuesta automática a incidentes con dobles de red y de Worker
//! Sin red ni subprocesos.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use url::Url;
use wodw::configuracion::{ConfiguracionIds, ConfiguracionWorker};
use wodw::error::{ErrorApp, Resultado};
use wodw::ids::{ControlIds, MotorIds, NivelSeveridad, ResumenTelemetria};
use wodw::ipc::mensajes::{OrdenWorker, RespuestaWorker};
use wodw::maestro::navegacion::incidentes::RespuestaAutomatica;
use wodw::maestro::navegacion::{FuenteHttp, FuturoCaja, ProcesadorContenido};
use wodw::maestro::red::RespuestaHttp;
use wodw::maestro::{ContenidoPagina, ServicioNavegacion};

/// Respuesta programada de la fuente falsa.
type Programada = Box<dyn Fn(&Url) -> Resultado<RespuestaHttp> + Send + Sync>;

/// Fuente HTTP falsa que cuenta las peticiones.
struct FuenteFalsa {
    responder: Programada,
    llamadas: AtomicUsize,
}

impl FuenteHttp for FuenteFalsa {
    fn obtener<'a>(&'a self, _id: u64, url: &'a Url) -> FuturoCaja<'a, Resultado<RespuestaHttp>> {
        self.llamadas.fetch_add(1, Ordering::SeqCst);
        let r = (self.responder)(url);
        Box::pin(async move { r })
    }
}

/// Procesador en proceso (el código real del Worker, sin confinamiento) o fallo fijo.
enum Procesador {
    EnProceso(AtomicUsize),
    Fallo(fn() -> ErrorApp),
}

impl ProcesadorContenido for Procesador {
    fn procesar<'a>(
        &'a self,
        orden: &'a OrdenWorker,
    ) -> FuturoCaja<'a, Resultado<RespuestaWorker>> {
        let r = match self {
            Procesador::EnProceso(usos) => {
                usos.fetch_add(1, Ordering::SeqCst);
                wodw::worker::procesar_orden(orden.clone(), &ConfiguracionWorker::default())
                    .ok_or_else(|| ErrorApp::Proceso("sin respuesta".to_string()))
            }
            Procesador::Fallo(crear) => Err(crear()),
        };
        Box::pin(async move { r })
    }
}

fn html(cuerpo: &str) -> Resultado<RespuestaHttp> {
    Ok(RespuestaHttp::nueva(
        200,
        &[("Content-Type", "text/html")],
        cuerpo.as_bytes().to_vec(),
    ))
}

/// Motor IDS con banderas de rotación y pánico.
struct Ids {
    control: ControlIds,
    rotado: Arc<AtomicBool>,
    panico: Arc<AtomicBool>,
}

fn ids() -> Ids {
    let rotado = Arc::new(AtomicBool::new(false));
    let panico = Arc::new(AtomicBool::new(false));
    let (r, p) = (rotado.clone(), panico.clone());
    let (control, _) = MotorIds::nuevo(ConfiguracionIds::default())
        .con_rotacion(move || r.store(true, Ordering::SeqCst))
        .con_panico(move || p.store(true, Ordering::SeqCst))
        .iniciar();
    Ids {
        control,
        rotado,
        panico,
    }
}

async fn esperar(control: &ControlIds, n: u64) -> ResumenTelemetria {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let r = control.obtener_resumen();
            if r.total_eventos >= n {
                return r;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("el IDS no recibió el evento")
}

fn servicio(
    responder: Programada,
    procesador: Procesador,
    ids: &Ids,
) -> ServicioNavegacion<FuenteFalsa, Procesador> {
    ServicioNavegacion::nuevo(
        FuenteFalsa {
            responder,
            llamadas: AtomicUsize::new(0),
        },
        procesador,
        RespuestaAutomatica::nueva(true, Some(ids.control.emisor())),
        5,
    )
}

#[tokio::test]
async fn navegacion_sigue_redirecciones_y_procesa_el_documento() {
    let ids = ids();
    let s = servicio(
        Box::new(|url: &Url| match url.path() {
            "/" => Ok(RespuestaHttp::nueva(302, &[("Location", "/final")], vec![])),
            _ => html("<title>Fin</title><p>Hola</p><a href=\"otra\">x</a>"),
        }),
        Procesador::EnProceso(AtomicUsize::new(0)),
        &ids,
    );
    let r = s.navegar(1, "http://a.onion/").await.unwrap();
    assert_eq!(r.url, "http://a.onion/final");
    let ContenidoPagina::Documento {
        titulo,
        texto,
        enlaces,
    } = r.contenido
    else {
        panic!("se esperaba documento");
    };
    assert_eq!(
        (titulo.as_str(), texto.as_str()),
        (
            "Fin", "Hola
x"
        )
    );
    assert_eq!(enlaces[0].url, "http://a.onion/otra");
}

#[tokio::test]
async fn respuesta_hostil_bloquea_el_host_y_rota_sin_intervencion() {
    let ids = ids();
    let s = servicio(
        Box::new(|_: &Url| {
            Err(ErrorApp::LimiteExcedido {
                recurso: "cuerpo HTTP",
                limite: 1,
            })
        }),
        Procesador::EnProceso(AtomicUsize::new(0)),
        &ids,
    );
    let fallo = s.navegar(1, "http://malo.onion/").await.unwrap_err();
    assert!(!fallo.purgar_pestana);
    let resumen = esperar(&ids.control, 1).await;
    assert_eq!(resumen.alerta_maxima, NivelSeveridad::Alto);
    assert!(
        ids.rotado.load(Ordering::SeqCst),
        "no se rotó el aislamiento"
    );
    let segundo = s.navegar(2, "http://malo.onion/otra").await.unwrap_err();
    assert!(segundo.mensaje.contains("bloqueado"), "{}", segundo.mensaje);
    assert_eq!(
        s.fuente().llamadas.load(Ordering::SeqCst),
        1,
        "el host bloqueado no debe contactarse"
    );
}

#[tokio::test]
async fn violacion_de_seccomp_dispara_panico_automatico() {
    let ids = ids();
    let s = servicio(
        Box::new(|_: &Url| html("<p>x</p>")),
        Procesador::Fallo(|| ErrorApp::WorkerTerminado {
            violacion_llamada_sistema: true,
            detalle: "signal: 31 (SIGSYS)".to_string(),
        }),
        &ids,
    );
    let fallo = s.navegar(1, "http://exploit.onion/").await.unwrap_err();
    assert!(fallo.purgar_pestana);
    let resumen = esperar(&ids.control, 1).await;
    assert_eq!(resumen.alerta_maxima, NivelSeveridad::Critico);
    assert!(ids.panico.load(Ordering::SeqCst), "no se disparó el pánico");
}

#[tokio::test]
async fn bomba_de_descompresion_purga_la_pestana() {
    let ids = ids();
    let s = servicio(
        Box::new(|_: &Url| html("x")),
        Procesador::Fallo(|| ErrorApp::ContenidoHostil("bomba de descompresión".to_string())),
        &ids,
    );
    let fallo = s.navegar(1, "http://bomba.onion/").await.unwrap_err();
    assert!(fallo.purgar_pestana);
    assert_eq!(
        esperar(&ids.control, 1).await.alerta_maxima,
        NivelSeveridad::Alto
    );
}

#[tokio::test]
async fn tipo_no_soportado_no_llega_al_worker() {
    let ids = ids();
    let procesador = Procesador::EnProceso(AtomicUsize::new(0));
    let s = servicio(
        Box::new(|_: &Url| {
            Ok(RespuestaHttp::nueva(
                200,
                &[("Content-Type", "application/x-msdownload")],
                b"MZ".to_vec(),
            ))
        }),
        procesador,
        &ids,
    );
    let r = s.navegar(1, "http://a.onion/virus.exe").await.unwrap();
    assert!(matches!(r.contenido, ContenidoPagina::NoSoportado { .. }));
    let Procesador::EnProceso(usos) = s.procesador() else {
        unreachable!()
    };
    assert_eq!(usos.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn fallo_ordinario_no_bloquea_ni_alerta() {
    let ids = ids();
    let s = servicio(
        Box::new(|_: &Url| {
            Err(ErrorApp::TiempoAgotado {
                operacion: "conexión Tor",
                milisegundos: 1,
            })
        }),
        Procesador::EnProceso(AtomicUsize::new(0)),
        &ids,
    );
    assert!(s.navegar(1, "http://lento.onion/").await.is_err());
    assert!(s.navegar(1, "http://lento.onion/").await.is_err());
    assert_eq!(s.fuente().llamadas.load(Ordering::SeqCst), 2);
    assert_eq!(ids.control.obtener_resumen().total_eventos, 0);
}

/// Un Worker comprometido que declara dimensiones falsas no llega a la interfaz.
#[tokio::test]
async fn imagen_incoherente_del_worker_se_trata_como_hostil() {
    struct WorkerMentiroso;
    impl ProcesadorContenido for WorkerMentiroso {
        fn procesar<'a>(
            &'a self,
            _orden: &'a OrdenWorker,
        ) -> FuturoCaja<'a, Resultado<RespuestaWorker>> {
            Box::pin(async {
                Ok(RespuestaWorker::ImagenProcesada {
                    id_tarea: 1,
                    ancho: 1000,
                    alto: 1000,
                    datos_rgba: vec![0; 4],
                })
            })
        }
    }
    let ids = ids();
    let s = ServicioNavegacion::nuevo(
        FuenteFalsa {
            responder: Box::new(|_: &Url| {
                Ok(RespuestaHttp::nueva(
                    200,
                    &[("Content-Type", "image/png")],
                    vec![1],
                ))
            }),
            llamadas: AtomicUsize::new(0),
        },
        WorkerMentiroso,
        RespuestaAutomatica::nueva(true, Some(ids.control.emisor())),
        5,
    );
    let fallo = s.navegar(1, "http://img.onion/a.png").await.unwrap_err();
    assert!(fallo.purgar_pestana);
}
