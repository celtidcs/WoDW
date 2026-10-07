//! Barreras de los formatos de imagen ampliados (CA-I4) y de la identificación
//! por contenido (CA-C1).

use std::io::Cursor;
use wodw::configuracion::ConfiguracionWorker;
use wodw::ipc::mensajes::{FormatoImagen, OrdenWorker, RespuestaWorker};

/// AVIF de 16×8: mitad izquierda roja, mitad derecha azul (ver `tests/datos/LEEME.md`).
const AVIF: &[u8] = include_bytes!("datos/imagen.avif");

fn procesar(cfg: &ConfiguracionWorker, formato: FormatoImagen, datos: Vec<u8>) -> RespuestaWorker {
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

/// Píxeles de una respuesta, o pánico con la respuesta recibida.
fn pixeles(r: RespuestaWorker) -> (u32, u32, Vec<u8>) {
    match r {
        RespuestaWorker::ImagenProcesada {
            ancho,
            alto,
            datos_rgba,
            ..
        } => (ancho, alto, datos_rgba),
        otra => panic!("se esperaba ImagenProcesada: {otra:?}"),
    }
}

fn codificar(img: &image::RgbaImage, formato: image::ImageFormat) -> Vec<u8> {
    let mut c = Cursor::new(Vec::new());
    img.write_to(&mut c, formato).unwrap();
    c.into_inner()
}

/// CA-I4: TIFF, ICO y QOI se decodifican a sus píxeles exactos.
#[test]
fn tiff_ico_y_qoi_se_reconstruyen() {
    let img = image::RgbaImage::from_fn(5, 3, |x, y| {
        image::Rgba([x as u8 * 50, y as u8 * 80, 7, 255])
    });
    for (formato, de_image) in [
        (FormatoImagen::Tiff, image::ImageFormat::Tiff),
        (FormatoImagen::Ico, image::ImageFormat::Ico),
        (FormatoImagen::Qoi, image::ImageFormat::Qoi),
    ] {
        let (ancho, alto, rgba) = pixeles(procesar(
            &ConfiguracionWorker::default(),
            formato,
            codificar(&img, de_image),
        ));
        assert_eq!((ancho, alto), (5, 3), "{formato:?}");
        assert_eq!(rgba, img.as_raw().as_slice(), "{formato:?}");
    }
}

/// CA-I4: AVIF se decodifica con sus colores (con margen por la compresión).
#[test]
fn avif_se_reconstruye_con_sus_colores() {
    let (ancho, alto, rgba) = pixeles(procesar(
        &ConfiguracionWorker::default(),
        FormatoImagen::Avif,
        AVIF.to_vec(),
    ));
    assert_eq!((ancho, alto), (16, 8));
    let pixel = |x: usize, y: usize| &rgba[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4];
    let (izquierda, derecha) = (pixel(2, 4), pixel(13, 4));
    assert!(
        izquierda[0] > 200 && izquierda[2] < 60,
        "rojo: {izquierda:?}"
    );
    assert!(derecha[2] > 200 && derecha[0] < 60, "azul: {derecha:?}");
    assert_eq!((izquierda[3], derecha[3]), (255, 255));
}

/// CA-I2 también en AVIF: el límite se mira en la cabecera.
#[test]
fn avif_por_encima_del_limite_es_alerta() {
    let cfg = ConfiguracionWorker {
        lado_largo_maximo_px: 15,
        lado_corto_maximo_px: 8,
        ..Default::default()
    };
    let r = procesar(&cfg, FormatoImagen::Avif, AVIF.to_vec());
    assert!(
        matches!(r, RespuestaWorker::AlertaSeguridad { .. }),
        "{r:?}"
    );
}

/// CA-C1: el contenido real manda; un archivo disfrazado se rechaza.
#[test]
fn archivo_disfrazado_se_rechaza() {
    let png = codificar(
        &image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255])),
        image::ImageFormat::Png,
    );
    for formato in [
        FormatoImagen::Tiff,
        FormatoImagen::Ico,
        FormatoImagen::Qoi,
        FormatoImagen::Avif,
        FormatoImagen::Jpeg,
    ] {
        let r = procesar(&ConfiguracionWorker::default(), formato, png.clone());
        assert!(
            matches!(r, RespuestaWorker::ErrorTarea { .. }),
            "PNG declarado como {formato:?}: {r:?}"
        );
    }
    let r = procesar(
        &ConfiguracionWorker::default(),
        FormatoImagen::Png,
        AVIF.to_vec(),
    );
    assert!(matches!(r, RespuestaWorker::ErrorTarea { .. }), "{r:?}");
}

/// CA-I4: los tipos MIME nuevos se reconocen.
#[test]
fn tipos_mime_nuevos() {
    for (mime, formato) in [
        ("image/tiff", FormatoImagen::Tiff),
        ("image/x-icon", FormatoImagen::Ico),
        ("image/vnd.microsoft.icon", FormatoImagen::Ico),
        ("image/qoi", FormatoImagen::Qoi),
        ("image/avif", FormatoImagen::Avif),
    ] {
        assert_eq!(FormatoImagen::desde_mime(mime), Some(formato), "{mime}");
    }
}

/// CA-C1 sin depender del decodificador: la firma se comprueba antes.
#[test]
fn firma_debe_coincidir_con_el_formato_declarado() {
    use wodw::worker::imagen::firma_coincide;
    let png = codificar(
        &image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255])),
        image::ImageFormat::Png,
    );
    assert!(firma_coincide(FormatoImagen::Png, &png));
    assert!(firma_coincide(FormatoImagen::Avif, AVIF));
    for formato in [FormatoImagen::Tiff, FormatoImagen::Gif, FormatoImagen::Avif] {
        assert!(!firma_coincide(formato, &png), "{formato:?}");
    }
    assert!(!firma_coincide(FormatoImagen::Png, &[]));
}
