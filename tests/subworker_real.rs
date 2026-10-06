//! El binario real de WoDW en modo Worker, confinado, atiende órdenes a través
//! de la frontera de proceso. Sin red.

use std::path::PathBuf;
use wodw::configuracion::ConfiguracionWorker;
use wodw::error::ErrorApp;
use wodw::ipc::mensajes::{OrdenWorker, RespuestaWorker};
use wodw::maestro::ProcesadorSubworker;

fn procesador(cfg: ConfiguracionWorker) -> ProcesadorSubworker {
    ProcesadorSubworker::nuevo(PathBuf::from(env!("CARGO_BIN_EXE_wodw")), cfg).unwrap()
}

#[tokio::test]
async fn subworker_confinado_procesa_html_y_muere() {
    let p = procesador(ConfiguracionWorker::default());
    let orden = OrdenWorker::ProcesarHtml {
        id_tarea: 7,
        url_origen: "http://a.onion/".to_string(),
        contenido_html: b"<title>T</title><p>Hola</p><script>x()</script>".to_vec(),
    };
    let r = p.procesar(&orden).await.unwrap();
    assert_eq!(
        r,
        RespuestaWorker::HtmlProcesado {
            id_tarea: 7,
            titulo: "T".to_string(),
            texto_limpio: "Hola".to_string(),
            enlaces: vec![],
        }
    );
    assert_eq!(
        p.procesar(&OrdenWorker::Ping { marca_tiempo: 3 })
            .await
            .unwrap(),
        RespuestaWorker::Pong { marca_tiempo: 3 }
    );
}

/// Un plazo de 1 ms es inferior al tiempo de arranque de cualquier proceso
/// (lanzar y confinar el Worker cuesta decenas de milisegundos), así que la
/// respuesta nunca llega a tiempo.
#[tokio::test]
async fn worker_que_no_responde_agota_el_plazo() {
    let cfg = ConfiguracionWorker {
        tiempo_espera_ms: 1,
        ..Default::default()
    };
    let r = procesador(cfg)
        .procesar(&OrdenWorker::Ping { marca_tiempo: 1 })
        .await;
    assert!(matches!(r, Err(ErrorApp::TiempoAgotado { .. })), "{r:?}");
}
