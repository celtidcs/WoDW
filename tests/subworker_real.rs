//! El binario real de WoDW en modo Worker, confinado, atiende órdenes a través
//! de la frontera de proceso. Sin red.

use std::path::PathBuf;
use wodw::configuracion::ConfiguracionWorker;
use wodw::error::ErrorApp;
use wodw::ipc::mensajes::{OrdenWorker, RespuestaWorker};
use wodw::maestro::ProcesadorSubworker;

fn procesador(cfg: ConfiguracionWorker) -> ProcesadorSubworker {
    ProcesadorSubworker::nuevo(PathBuf::from(env!("CARGO_BIN_EXE_wodw-worker")), cfg).unwrap()
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
            medios: vec![],
            recortado: false,
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

/// El audio se decodifica de verdad dentro del Worker confinado (en Windows,
/// en el AppContainer sin red) y llega validado al estado de reproducción.
#[tokio::test]
async fn subworker_confinado_decodifica_audio() {
    use wodw::configuracion::ConfiguracionReproduccion;
    use wodw::ipc::mensajes::FamiliaMedio;
    use wodw::maestro::medios::productor::producir;
    use wodw::maestro::medios::EstadoReproduccion;
    let cfg = ConfiguracionWorker::default();
    let mut canal = procesador(cfg.clone()).canal_medio().unwrap();
    let estado = EstadoReproduccion::nuevo(100);
    let cfg_rep = ConfiguracionReproduccion {
        bufer_audio_ms: 10_000,
        ..Default::default()
    };
    producir(
        &mut canal,
        FamiliaMedio::Audio,
        include_bytes!("datos/tono.mp3").to_vec(),
        &estado,
        &cfg,
        &cfg_rep,
    )
    .await
    .unwrap();
    assert!(
        estado.audio_pendiente_ms() > 900,
        "{}",
        estado.audio_pendiente_ms()
    );
}

/// El vídeo (H.264 con OpenH264 y AV1 con rav1d) se decodifica dentro del
/// Worker confinado y llegan fotogramas y sonido validados.
#[tokio::test]
async fn subworker_confinado_decodifica_video() {
    use wodw::configuracion::ConfiguracionReproduccion;
    use wodw::ipc::mensajes::FamiliaMedio;
    use wodw::maestro::medios::productor::producir;
    use wodw::maestro::medios::EstadoReproduccion;
    let cfg = ConfiguracionWorker::default();
    let cfg_rep = ConfiguracionReproduccion {
        bufer_audio_ms: 10_000,
        max_fotogramas_en_bufer: 100,
        ..Default::default()
    };
    for datos in [
        &include_bytes!("datos/video.mp4")[..],
        &include_bytes!("datos/video.webm")[..],
    ] {
        let mut canal = procesador(cfg.clone()).canal_medio().unwrap();
        let estado = EstadoReproduccion::nuevo(100);
        producir(
            &mut canal,
            FamiliaMedio::Video,
            datos.to_vec(),
            &estado,
            &cfg,
            &cfg_rep,
        )
        .await
        .unwrap();
        assert!(
            estado.fotogramas_pendientes() >= 9,
            "{}",
            estado.fotogramas_pendientes()
        );
        assert!(
            estado.audio_pendiente_ms() > 900,
            "{}",
            estado.audio_pendiente_ms()
        );
    }
}

/// CA-C3: un Worker que necesita más memoria de la permitida muere sin
/// afectar al Maestro; con el límite por defecto, la misma orden se atiende.
#[tokio::test]
async fn worker_sin_memoria_suficiente_muere_sin_afectar_al_maestro() {
    use wodw::ipc::mensajes::FamiliaMedio;
    let orden = OrdenWorker::AbrirMedio {
        id_tarea: 1,
        familia: FamiliaMedio::Audio,
        datos_crudos: vec![0x5A; 200 * 1024 * 1024],
    };
    let r = procesador(ConfiguracionWorker::default())
        .procesar(&orden)
        .await;
    assert!(
        matches!(r, Ok(RespuestaWorker::ErrorTarea { .. })),
        "control: {r:?}"
    );
    let escaso = ConfiguracionWorker {
        memoria_maxima_worker_bytes: 128 * 1024 * 1024,
        ..Default::default()
    };
    let r = procesador(escaso).procesar(&orden).await;
    assert!(
        matches!(
            r,
            Err(ErrorApp::WorkerTerminado { .. }) | Err(ErrorApp::Ipc(_))
        ),
        "{r:?}"
    );
}
