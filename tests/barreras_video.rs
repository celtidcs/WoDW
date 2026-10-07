//! Barreras del vídeo (CA-V1–V6): MP4 (H.264) y WebM (AV1) decodificados en el
//! Worker, fotogramas reconstruidos en orden de presentación, límite 4K
//! mirado en la cabecera, sonido por la cadena del audio y, sin pista de
//! audio, silencio que mantiene el reloj. Sin red ni procesos.

use wodw::configuracion::ConfiguracionWorker;
use wodw::ipc::mensajes::{
    FamiliaMedio, FotogramaVideo, OrdenWorker, RespuestaWorker, CANALES_SALIDA,
    FRECUENCIA_SALIDA_HZ,
};
use wodw::maestro::medios::validar_bloque;
use wodw::worker::EstadoWorker;

/// 64×48, 1 s a 10 fps, H.264 Baseline + AAC.
const MP4: &[u8] = include_bytes!("datos/video.mp4");
/// 64×48, 1 s a 10 fps, AV1 + Opus.
const WEBM: &[u8] = include_bytes!("datos/video.webm");
/// 32×32, 10 grises crecientes, H.264 High con fotogramas B, sin audio.
const ORDEN_H264: &[u8] = include_bytes!("datos/orden-h264.mp4");
/// 32×32, 10 grises crecientes, AV1, sin audio.
const ORDEN_AV1: &[u8] = include_bytes!("datos/orden-av1.webm");
/// VP9: fuera de la lista cerrada.
const VP9: &[u8] = include_bytes!("datos/video-vp9.webm");

/// Resultado de reproducir un vídeo entero.
struct Visto {
    dimensiones: Option<(u32, u32)>,
    audio: Vec<i16>,
    fotogramas: Vec<FotogramaVideo>,
}

fn ver(datos: &[u8], cfg: &ConfiguracionWorker) -> Result<Visto, RespuestaWorker> {
    let mut w = EstadoWorker::nuevo(cfg.clone());
    let apertura = w
        .atender(OrdenWorker::AbrirMedio {
            id_tarea: 1,
            familia: FamiliaMedio::Video,
            datos_crudos: datos.to_vec(),
        })
        .expect("respuesta");
    let RespuestaWorker::MedioAbierto { video, .. } = apertura else {
        return Err(apertura);
    };
    let mut visto = Visto {
        dimensiones: video,
        audio: Vec::new(),
        fotogramas: Vec::new(),
    };
    for _ in 0..1000 {
        let r = w
            .atender(OrdenWorker::SiguienteBloque { id_tarea: 1 })
            .expect("respuesta");
        if !matches!(r, RespuestaWorker::BloqueMedio { .. }) {
            return Err(r);
        }
        validar_bloque(&r, cfg).unwrap_or_else(|e| panic!("bloque rechazado por el Maestro: {e}"));
        let RespuestaWorker::BloqueMedio {
            audio_pcm,
            fotogramas,
            fin,
            ..
        } = r
        else {
            unreachable!()
        };
        visto.audio.extend(audio_pcm);
        visto.fotogramas.extend(fotogramas);
        if fin {
            return Ok(visto);
        }
    }
    panic!("el vídeo no termina");
}

fn segundos_de_audio(pcm: &[i16]) -> f64 {
    pcm.len() as f64 / f64::from(FRECUENCIA_SALIDA_HZ) / f64::from(CANALES_SALIDA)
}

/// CA-V1/V2/V5: MP4 y WebM salen con sus fotogramas y su sonido.
#[test]
fn mp4_y_webm_se_reproducen_con_imagen_y_sonido() {
    for (nombre, datos) in [("mp4", MP4), ("webm", WEBM)] {
        let v = ver(datos, &ConfiguracionWorker::default())
            .unwrap_or_else(|e| panic!("{nombre}: {e:?}"));
        assert_eq!(v.dimensiones, Some((64, 48)), "{nombre}");
        assert!(
            (9..=11).contains(&v.fotogramas.len()),
            "{nombre}: {} fotogramas",
            v.fotogramas.len()
        );
        for f in &v.fotogramas {
            assert_eq!(
                (f.ancho, f.alto, f.rgba.len()),
                (64, 48, 64 * 48 * 4),
                "{nombre}"
            );
            assert!(f.marca_ms < 1100, "{nombre}: marca {}", f.marca_ms);
        }
        let s = segundos_de_audio(&v.audio);
        assert!((0.9..1.15).contains(&s), "{nombre}: {s} s de audio");
        assert!(
            v.audio.iter().any(|&m| m.unsigned_abs() > 1000),
            "{nombre}: audio mudo"
        );
    }
}

/// CA-V2: los fotogramas salen en orden de presentación (grises crecientes),
/// también con fotogramas B; sin audio, el bloque lleva silencio para el reloj.
#[test]
fn fotogramas_en_orden_de_presentacion_y_silencio_sin_audio() {
    for (nombre, datos) in [("h264", ORDEN_H264), ("av1", ORDEN_AV1)] {
        let v = ver(datos, &ConfiguracionWorker::default())
            .unwrap_or_else(|e| panic!("{nombre}: {e:?}"));
        assert_eq!(v.fotogramas.len(), 10, "{nombre}");
        let grises: Vec<u8> = v
            .fotogramas
            .iter()
            .map(|f| f.rgba[(16 * 32 + 16) * 4])
            .collect();
        assert!(
            grises.windows(2).all(|p| p[1] > p[0]),
            "{nombre}: grises {grises:?}"
        );
        let marcas: Vec<u64> = v.fotogramas.iter().map(|f| f.marca_ms).collect();
        assert!(
            marcas.windows(2).all(|p| p[1] > p[0]),
            "{nombre}: marcas {marcas:?}"
        );
        let s = segundos_de_audio(&v.audio);
        assert!((0.9..1.15).contains(&s), "{nombre}: {s} s de silencio");
        assert!(
            v.audio.iter().all(|&m| m == 0),
            "{nombre}: debe ser silencio"
        );
    }
}

/// CA-V2: por encima del límite de resolución se rechaza antes de decodificar.
#[test]
fn video_por_encima_del_limite_es_alerta() {
    let cfg = ConfiguracionWorker {
        lado_largo_maximo_px: 63,
        lado_corto_maximo_px: 48,
        ..Default::default()
    };
    for (nombre, datos) in [("mp4", MP4), ("webm", WEBM)] {
        let r = ver(datos, &cfg);
        assert!(
            matches!(r, Err(RespuestaWorker::AlertaSeguridad { .. })),
            "{nombre}: {:?}",
            r.map(|v| v.fotogramas.len())
        );
    }
}

/// CA-V1: un códec fuera de la lista se rechaza sin caída.
#[test]
fn codec_no_admitido_se_rechaza() {
    let r = ver(VP9, &ConfiguracionWorker::default());
    match r {
        Err(RespuestaWorker::ErrorTarea { mensaje, .. }) => {
            assert!(mensaje.contains("no admitido"), "{mensaje}")
        }
        otro => panic!(
            "se esperaba rechazo: {:?}",
            otro.map(|v| v.fotogramas.len())
        ),
    }
}

/// CA-V4: el Maestro rechaza fotogramas incoherentes.
#[test]
fn maestro_rechaza_fotogramas_incoherentes() {
    let cfg = ConfiguracionWorker::default();
    let f = |marca_ms, ancho: u32, alto: u32, bytes| FotogramaVideo {
        marca_ms,
        ancho,
        alto,
        rgba: vec![0; bytes],
    };
    let bloque = |fotogramas| RespuestaWorker::BloqueMedio {
        id_tarea: 1,
        audio_pcm: vec![],
        fotogramas,
        fin: false,
    };
    assert!(validar_bloque(&bloque(vec![f(0, 2, 2, 16), f(40, 2, 2, 16)]), &cfg).is_ok());
    assert!(
        validar_bloque(&bloque(vec![f(0, 2, 2, 15)]), &cfg).is_err(),
        "búfer que no cuadra"
    );
    assert!(
        validar_bloque(&bloque(vec![f(40, 2, 2, 16), f(0, 2, 2, 16)]), &cfg).is_err(),
        "marcas que retroceden"
    );
    assert!(
        validar_bloque(&bloque(vec![f(0, 3841, 1, 3841 * 4)]), &cfg).is_err(),
        "más de 4K"
    );
}

/// CA-V3/G2: ningún bloque supera el tope de bytes de una respuesta; los
/// fotogramas que no caben pasan al bloque siguiente sin perderse.
#[test]
fn bloques_respetan_el_tope_de_bytes() {
    let fotograma = 64 * 48 * 4;
    let cfg = ConfiguracionWorker {
        limite_mensaje_ipc_bytes: 1024 * 1024 + 2 * fotograma,
        ..Default::default()
    };
    let mut w = EstadoWorker::nuevo(cfg.clone());
    w.atender(OrdenWorker::AbrirMedio {
        id_tarea: 1,
        familia: FamiliaMedio::Video,
        datos_crudos: MP4.to_vec(),
    });
    let mut total = 0;
    for _ in 0..100 {
        let Some(RespuestaWorker::BloqueMedio {
            fotogramas, fin, ..
        }) = w.atender(OrdenWorker::SiguienteBloque { id_tarea: 1 })
        else {
            panic!("se esperaba un bloque");
        };
        assert!(
            fotogramas.len() <= 2,
            "{} fotogramas en un bloque",
            fotogramas.len()
        );
        total += fotogramas.len();
        if fin {
            break;
        }
    }
    assert!((9..=11).contains(&total), "{total} fotogramas en total");
}
