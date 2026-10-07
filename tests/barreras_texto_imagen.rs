//! Barreras de la fase 1 de medios: texto (CA-T1–T4) e imagen (CA-I2–I3).
//!
//! Comprueban el Worker (`procesar_orden`) y la segunda validación del Maestro
//! con un Worker mentiroso. Sin red ni subprocesos.

use std::io::Cursor;
use std::sync::Arc;
use url::Url;
use wodw::configuracion::{ConfiguracionIds, ConfiguracionWorker};
use wodw::error::Resultado;
use wodw::ids::MotorIds;
use wodw::ipc::mensajes::{Enlace, FormatoImagen, OrdenWorker, RespuestaWorker};
use wodw::maestro::navegacion::incidentes::RespuestaAutomatica;
use wodw::maestro::navegacion::{FuenteHttp, FuturoCaja, ProcesadorContenido};
use wodw::maestro::red::RespuestaHttp;
use wodw::maestro::{ContenidoPagina, ServicioNavegacion};
use wodw::seguridad::enlaces::{evaluar_enlace, AvisoEnlace};
use wodw::seguridad::texto::limpiar_texto;

/// Caracteres invisibles que CA-T1 obliga a eliminar.
const INVISIBLES: &[char] = &[
    '\u{200B}',
    '\u{200C}',
    '\u{200D}',
    '\u{2060}',
    '\u{2064}',
    '\u{FEFF}',
    '\u{180E}',
    '\u{00AD}',
    '\u{E0041}',
    '\u{E007F}',
];

fn html(cfg: &ConfiguracionWorker, cuerpo: &str) -> RespuestaWorker {
    wodw::worker::procesar_orden(
        OrdenWorker::ProcesarHtml {
            id_tarea: 1,
            url_origen: "http://a.onion/".to_string(),
            contenido_html: cuerpo.as_bytes().to_vec(),
        },
        cfg,
    )
    .expect("respuesta")
}

/// CA-T1: los invisibles desaparecen del texto, del título y de los enlaces.
#[test]
fn invisibles_se_eliminan_en_el_worker() {
    let sucio: String = INVISIBLES.iter().fold(String::from("ho"), |mut s, c| {
        s.push(*c);
        s
    }) + "la";
    let r = html(
        &ConfiguracionWorker::default(),
        &format!("<title>{sucio}</title><p>{sucio}</p><a href=\"/x\">{sucio}</a>"),
    );
    let RespuestaWorker::HtmlProcesado {
        titulo,
        texto_limpio,
        enlaces,
        ..
    } = r
    else {
        panic!("se esperaba HtmlProcesado: {r:?}");
    };
    assert_eq!(titulo, "hola");
    assert!(texto_limpio.contains("hola"), "{texto_limpio:?}");
    assert_eq!(enlaces[0].texto, "hola");
    for c in INVISIBLES {
        assert!(!texto_limpio.contains(*c), "queda U+{:04X}", u32::from(*c));
    }
}

/// CA-T2: forma normal C.
#[test]
fn texto_se_normaliza_a_nfc() {
    assert_eq!(limpiar_texto("e\u{301}"), "\u{e9}");
}

/// CA-T3: enlaces engañosos y punycode.
#[test]
fn enlaces_enganosos_se_marcan() {
    assert_eq!(
        evaluar_enlace("http://banco.onion/entrar", "http://malo.onion/"),
        Some(AvisoEnlace::DestinoDistinto {
            host_real: "malo.onion".to_string()
        })
    );
    assert_eq!(
        evaluar_enlace("www.banco.com", "http://malo.onion/"),
        Some(AvisoEnlace::DestinoDistinto {
            host_real: "malo.onion".to_string()
        })
    );
    assert_eq!(
        evaluar_enlace("Pulsa aquí", "http://xn--bnco-0qa.com/"),
        Some(AvisoEnlace::Punycode {
            host_real: "xn--bnco-0qa.com".to_string()
        })
    );
    assert_eq!(evaluar_enlace("Pulsa aquí", "http://a.onion/x"), None);
    assert_eq!(
        evaluar_enlace("http://a.onion/otra", "http://a.onion/x"),
        None
    );
    assert_eq!(evaluar_enlace("archivo.txt", "http://a.onion/x"), None);
}

/// CA-T4: tope de caracteres en el Worker.
#[test]
fn texto_largo_se_recorta_y_se_avisa() {
    let cfg = ConfiguracionWorker {
        max_caracteres_texto: 10,
        ..Default::default()
    };
    let r = html(&cfg, &format!("<p>{}</p>", "á".repeat(50)));
    let RespuestaWorker::HtmlProcesado {
        texto_limpio,
        recortado,
        ..
    } = r
    else {
        panic!("se esperaba HtmlProcesado");
    };
    assert!(recortado);
    assert_eq!(texto_limpio.chars().count(), 10);
}

fn png(ancho: u32, alto: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(ancho, alto, image::Rgba([10, 20, 30, 255]));
    let mut c = Cursor::new(Vec::new());
    img.write_to(&mut c, image::ImageFormat::Png).unwrap();
    c.into_inner()
}

fn imagen(cfg: &ConfiguracionWorker, formato: FormatoImagen, datos: Vec<u8>) -> RespuestaWorker {
    wodw::worker::procesar_orden(
        OrdenWorker::ProcesarImagen {
            id_tarea: 1,
            formato,
            datos_crudos: datos,
        },
        cfg,
    )
    .expect("respuesta")
}

/// CA-I2: límite 4K en cualquier orientación, sin reducir lo que cabe.
#[test]
fn imagen_por_encima_de_4k_se_rechaza_y_la_que_cabe_no_se_reduce() {
    let cfg = ConfiguracionWorker::default();
    for (ancho, alto) in [(3840, 1), (1, 2160), (2160, 3840)] {
        let r = imagen(&cfg, FormatoImagen::Png, png(ancho, alto));
        let RespuestaWorker::ImagenProcesada {
            ancho: a, alto: h, ..
        } = r
        else {
            panic!("{ancho}×{alto} cabe en 4K: {r:?}");
        };
        assert_eq!((a, h), (ancho, alto), "no se reduce");
    }
    for (ancho, alto) in [(3841, 1), (2161, 2161), (1, 3841)] {
        let r = imagen(&cfg, FormatoImagen::Png, png(ancho, alto));
        assert!(
            matches!(r, RespuestaWorker::AlertaSeguridad { .. }),
            "{ancho}×{alto} supera 4K: {r:?}"
        );
    }
}

/// CA-I3: de un GIF animado solo sale el primer fotograma.
#[test]
fn gif_animado_entrega_el_primer_fotograma() {
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Frame, Rgba, RgbaImage};
    let mut gif = Vec::new();
    {
        let mut codificador = GifEncoder::new(&mut gif);
        for color in [[255, 0, 0, 255], [0, 0, 255, 255]] {
            codificador
                .encode_frame(Frame::from_parts(
                    RgbaImage::from_pixel(2, 2, Rgba(color)),
                    0,
                    0,
                    Delay::from_numer_denom_ms(100, 1),
                ))
                .unwrap();
        }
    }
    let RespuestaWorker::ImagenProcesada { datos_rgba, .. } =
        imagen(&ConfiguracionWorker::default(), FormatoImagen::Gif, gif)
    else {
        panic!("se esperaba ImagenProcesada");
    };
    assert_eq!(&datos_rgba[..4], &[255, 0, 0, 255]);
}

/// Fuente que devuelve siempre el mismo tipo MIME.
struct Fuente(&'static str);

impl FuenteHttp for Fuente {
    fn obtener<'a>(&'a self, _id: u64, _url: &'a Url) -> FuturoCaja<'a, Resultado<RespuestaHttp>> {
        let r = Ok(RespuestaHttp::nueva(
            200,
            &[("Content-Type", self.0)],
            vec![1],
        ));
        Box::pin(async move { r })
    }
}

/// Worker comprometido que devuelve una respuesta fija.
struct WorkerMentiroso(RespuestaWorker);

impl ProcesadorContenido for WorkerMentiroso {
    fn procesar<'a>(
        &'a self,
        _orden: &'a OrdenWorker,
    ) -> FuturoCaja<'a, Resultado<RespuestaWorker>> {
        let r = Ok(self.0.clone());
        Box::pin(async move { r })
    }
}

fn servicio(
    tipo: &'static str,
    respuesta: RespuestaWorker,
    cfg: &ConfiguracionWorker,
) -> Arc<ServicioNavegacion<Fuente, WorkerMentiroso>> {
    let (control, _) = MotorIds::nuevo(ConfiguracionIds::default()).iniciar();
    Arc::new(
        ServicioNavegacion::nuevo(
            Fuente(tipo),
            WorkerMentiroso(respuesta),
            RespuestaAutomatica::nueva(true, Some(control.emisor())),
            5,
        )
        .con_limites(cfg),
    )
}

/// CA-T1–T4 en el Maestro: un Worker comprometido no cuela invisibles, Bidi ni
/// texto sin tope, y los enlaces llegan revisados.
#[tokio::test]
async fn maestro_vuelve_a_limpiar_el_texto_del_worker() {
    let cfg = ConfiguracionWorker {
        max_caracteres_texto: 8,
        ..Default::default()
    };
    let s = servicio(
        "text/html",
        RespuestaWorker::HtmlProcesado {
            id_tarea: 1,
            titulo: "ti\u{200B}tulo\u{202E}".to_string(),
            texto_limpio: "a\u{FEFF}b".repeat(20),
            enlaces: vec![Enlace {
                texto: "http://banco.onion/".to_string(),
                url: "http://malo.onion/".to_string(),
            }],
            medios: vec![],
            recortado: false,
        },
        &cfg,
    );
    let r = s.navegar(1, "http://a.onion/").await.unwrap();
    let ContenidoPagina::Documento {
        titulo,
        texto,
        enlaces,
        recortado,
        ..
    } = r.contenido
    else {
        panic!("se esperaba documento");
    };
    assert_eq!(titulo, "titulo");
    assert_eq!(texto, "abababab");
    assert!(recortado);
    assert_eq!(
        enlaces[0].aviso,
        Some(AvisoEnlace::DestinoDistinto {
            host_real: "malo.onion".to_string()
        })
    );
}

/// CA-I2 en el Maestro: una imagen por encima de 4K es un incidente.
#[tokio::test]
async fn maestro_rechaza_imagen_por_encima_de_4k() {
    let s = servicio(
        "image/png",
        RespuestaWorker::ImagenProcesada {
            id_tarea: 1,
            ancho: 2161,
            alto: 2161,
            datos_rgba: vec![0; 2161 * 2161 * 4],
        },
        &ConfiguracionWorker::default(),
    );
    let fallo = s.navegar(1, "http://a.onion/i.png").await.unwrap_err();
    assert!(fallo.purgar_pestana, "{}", fallo.mensaje);
}
