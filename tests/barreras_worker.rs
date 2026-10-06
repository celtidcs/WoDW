//! Pruebas del procesamiento de imágenes, audio y enlaces en el Worker.

use std::f64::consts::PI;
use std::io::Cursor;
use wodw::configuracion::ConfiguracionWorker;
use wodw::ipc::mensajes::{FormatoImagen, OrdenWorker, RespuestaWorker};
use wodw::worker::procesar_orden;

fn procesar(orden: OrdenWorker) -> RespuestaWorker {
    procesar_orden(orden, &ConfiguracionWorker::default()).expect("respuesta")
}

/// La imagen se decodifica de verdad y depende de la entrada.
#[test]
fn imagen_real_conserva_dimensiones_y_pixeles() {
    let imagen = image::RgbaImage::from_fn(3, 2, |x, y| {
        image::Rgba([x as u8 * 80, y as u8 * 120, 9, 255])
    });
    let mut png = Cursor::new(Vec::new());
    imagen.write_to(&mut png, image::ImageFormat::Png).unwrap();
    let r = procesar(OrdenWorker::ProcesarImagen {
        id_tarea: 1,
        formato: FormatoImagen::Png,
        datos_crudos: png.into_inner(),
    });
    let RespuestaWorker::ImagenProcesada {
        ancho,
        alto,
        datos_rgba,
        ..
    } = r
    else {
        panic!("se esperaba ImagenProcesada: {r:?}");
    };
    assert_eq!((ancho, alto), (3, 2));
    assert_eq!(datos_rgba, imagen.into_raw());
    let basura = procesar(OrdenWorker::ProcesarImagen {
        id_tarea: 2,
        formato: FormatoImagen::Png,
        datos_crudos: vec![1, 2, 3],
    });
    assert!(matches!(basura, RespuestaWorker::ErrorTarea { .. }));
}

/// Construye un WAV PCM16 mono.
fn wav(frecuencia_muestreo: u32, tono: f64) -> Vec<u8> {
    let n = frecuencia_muestreo as usize / 2;
    let datos: Vec<u8> = (0..n)
        .map(|i| {
            (10_000.0 * (2.0 * PI * tono * i as f64 / f64::from(frecuencia_muestreo)).sin()) as i16
        })
        .flat_map(i16::to_le_bytes)
        .collect();
    let mut w = Vec::new();
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + datos.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&frecuencia_muestreo.to_le_bytes());
    w.extend_from_slice(&(frecuencia_muestreo * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(datos.len() as u32).to_le_bytes());
    w.extend_from_slice(&datos);
    w
}

/// Ganancia en dB tras el Worker, descartando el transitorio inicial.
fn ganancia_db(frecuencia_muestreo: u32, tono: f64) -> f64 {
    let bytes = wav(frecuencia_muestreo, tono);
    let entrada: Vec<i16> = bytes[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&b| i16::from_le_bytes(b))
        .collect();
    let r = procesar(OrdenWorker::ProcesarAudio {
        id_tarea: 1,
        datos_crudos: bytes,
    });
    let RespuestaWorker::AudioProcesado {
        muestras_pcm,
        frecuencia_muestreo: fs,
        ..
    } = r
    else {
        panic!("se esperaba AudioProcesado: {r:?}");
    };
    assert_eq!(fs, frecuencia_muestreo);
    let descarte = entrada.len() / 5;
    let rms = |v: &[i16]| {
        (v[descarte..]
            .iter()
            .map(|&x| f64::from(x).powi(2))
            .sum::<f64>()
            / (v.len() - descarte) as f64)
            .sqrt()
    };
    20.0 * (rms(&muestras_pcm) / rms(&entrada)).log10()
}

/// El audible pasa intacto y el ultrasonido se atenúa.
#[test]
fn filtro_de_audio_respeta_audible_y_corta_ultrasonidos() {
    assert!(ganancia_db(44_100, 1_000.0) > -1.0);
    assert!(ganancia_db(44_100, 5_000.0) > -1.0);
    assert!(ganancia_db(96_000, 30_000.0) < -30.0);
}

/// Solo enlaces http/https absolutos; `data-href` no cuenta como `href`.
#[test]
fn enlaces_peligrosos_se_descartan() {
    let html = br#"<a href="javascript:alert(1)">a</a><a href='file:///C:/x'>b</a><a href="data:text/html,x">c</a><a data-href="http://trampa.onion" href="/real">d</a>"#;
    let r = procesar(OrdenWorker::ProcesarHtml {
        id_tarea: 1,
        url_origen: "http://origen.onion/dir/".to_string(),
        contenido_html: html.to_vec(),
    });
    let RespuestaWorker::HtmlProcesado { enlaces, .. } = r else {
        panic!("se esperaba HtmlProcesado");
    };
    let urls: Vec<&str> = enlaces.iter().map(|e| e.url.as_str()).collect();
    assert_eq!(urls, ["http://origen.onion/real"]);
}
