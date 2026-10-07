//! Barreras de CA-G1: las imágenes, audios y vídeos incrustados en una página
//! se listan, pero **no se descargan** hasta que el usuario los pulsa.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use url::Url;
use wodw::configuracion::{ConfiguracionIds, ConfiguracionWorker};
use wodw::error::Resultado;
use wodw::ids::MotorIds;
use wodw::ipc::mensajes::{MedioEnlazado, OrdenWorker, RespuestaWorker, TipoMedioEnlazado};
use wodw::maestro::navegacion::incidentes::RespuestaAutomatica;
use wodw::maestro::navegacion::{FuenteHttp, FuturoCaja, ProcesadorContenido};
use wodw::maestro::red::RespuestaHttp;
use wodw::maestro::{ContenidoPagina, ServicioNavegacion};

const PAGINA: &str = r#"<title>Galería</title>
<p>Texto</p>
<img src="/a.png" alt="Foto">
<img src="javascript:alert(1)">
<img src="b.jpg">
<audio src="cancion.mp3"></audio>
<video controls><source src="peli.webm" type="video/webm"><source src="data:video/mp4,x"></video>
<img src="/a.png">"#;

fn html(cfg: &ConfiguracionWorker, cuerpo: &str) -> RespuestaWorker {
    wodw::worker::procesar_orden(
        OrdenWorker::ProcesarHtml {
            id_tarea: 1,
            url_origen: "http://a.onion/dir/".to_string(),
            contenido_html: cuerpo.as_bytes().to_vec(),
        },
        cfg,
    )
    .expect("respuesta")
}

/// El Worker lista los medios con su tipo y URL absoluta, sin esquemas
/// peligrosos ni repetidos.
#[test]
fn el_worker_lista_los_medios_de_la_pagina() {
    let RespuestaWorker::HtmlProcesado { medios, .. } =
        html(&ConfiguracionWorker::default(), PAGINA)
    else {
        panic!("se esperaba HtmlProcesado");
    };
    let vistos: Vec<(TipoMedioEnlazado, &str, &str)> = medios
        .iter()
        .map(|m| (m.tipo, m.url.as_str(), m.texto.as_str()))
        .collect();
    assert_eq!(
        vistos,
        [
            (TipoMedioEnlazado::Imagen, "http://a.onion/a.png", "Foto"),
            (TipoMedioEnlazado::Imagen, "http://a.onion/dir/b.jpg", ""),
            (
                TipoMedioEnlazado::Audio,
                "http://a.onion/dir/cancion.mp3",
                ""
            ),
            (TipoMedioEnlazado::Video, "http://a.onion/dir/peli.webm", ""),
        ]
    );
}

/// El número de medios listados tiene tope.
#[test]
fn medios_con_tope() {
    let cfg = ConfiguracionWorker {
        max_medios_por_pagina: 2,
        ..Default::default()
    };
    let RespuestaWorker::HtmlProcesado { medios, .. } = html(&cfg, PAGINA) else {
        panic!("se esperaba HtmlProcesado");
    };
    assert_eq!(medios.len(), 2);
}

/// Fuente que cuenta las descargas.
struct Fuente(Arc<AtomicUsize>);

impl FuenteHttp for Fuente {
    fn obtener<'a>(&'a self, _id: u64, _url: &'a Url) -> FuturoCaja<'a, Resultado<RespuestaHttp>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        let r = Ok(RespuestaHttp::nueva(
            200,
            &[("Content-Type", "text/html")],
            PAGINA.as_bytes().to_vec(),
        ));
        Box::pin(async move { r })
    }
}

/// Procesador: el Worker real en proceso o uno comprometido con respuesta fija.
enum Procesador {
    Real,
    Mentiroso(RespuestaWorker),
}

impl ProcesadorContenido for Procesador {
    fn procesar<'a>(
        &'a self,
        orden: &'a OrdenWorker,
    ) -> FuturoCaja<'a, Resultado<RespuestaWorker>> {
        let r = match self {
            Procesador::Real => Ok(wodw::worker::procesar_orden(
                orden.clone(),
                &ConfiguracionWorker::default(),
            )
            .expect("respuesta")),
            Procesador::Mentiroso(r) => Ok(r.clone()),
        };
        Box::pin(async move { r })
    }
}

fn servicio(p: Procesador, descargas: Arc<AtomicUsize>) -> ServicioNavegacion<Fuente, Procesador> {
    let (control, _) = MotorIds::nuevo(ConfiguracionIds::default()).iniciar();
    ServicioNavegacion::nuevo(
        Fuente(descargas),
        p,
        RespuestaAutomatica::nueva(true, Some(control.emisor())),
        5,
    )
}

/// Abrir la página descarga solo la página: ningún medio se pide solo.
#[tokio::test]
async fn abrir_la_pagina_no_descarga_sus_medios() {
    let descargas = Arc::new(AtomicUsize::new(0));
    let s = servicio(Procesador::Real, descargas.clone());
    let r = s.navegar(1, "http://a.onion/dir/").await.unwrap();
    let ContenidoPagina::Documento { medios, .. } = r.contenido else {
        panic!("se esperaba documento");
    };
    assert_eq!(medios.len(), 4);
    assert_eq!(descargas.load(Ordering::SeqCst), 1, "solo la página");
}

/// El Maestro vuelve a filtrar los medios de un Worker comprometido.
#[tokio::test]
async fn el_maestro_refiltra_los_medios() {
    let medio = |url: &str, texto: &str| MedioEnlazado {
        tipo: TipoMedioEnlazado::Imagen,
        url: url.to_string(),
        texto: texto.to_string(),
    };
    let mentiroso = RespuestaWorker::HtmlProcesado {
        id_tarea: 1,
        titulo: String::new(),
        texto_limpio: String::new(),
        enlaces: vec![],
        medios: vec![
            medio("javascript:alert(1)", "x"),
            medio("file:///C:/secreto", "y"),
            medio("http://b.onion/c.png", "ok\u{202E}\u{200B}"),
        ],
        recortado: false,
    };
    let s = servicio(
        Procesador::Mentiroso(mentiroso),
        Arc::new(AtomicUsize::new(0)),
    );
    let r = s.navegar(1, "http://a.onion/").await.unwrap();
    let ContenidoPagina::Documento { medios, .. } = r.contenido else {
        panic!("se esperaba documento");
    };
    assert_eq!(medios, vec![medio("http://b.onion/c.png", "ok")]);
}
