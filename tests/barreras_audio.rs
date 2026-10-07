//! Barreras del audio (CA-A1–A3, CA-A5, CA-C1, CA-C5): decodificación en el
//! Worker por bloques, cadena fija 48 kHz estéreo con filtro anti-ultrasonidos
//! y limitador, identificación por contenido, revisión estructural y segunda
//! validación en el Maestro. Sin red, sin tarjeta de sonido.

use std::f64::consts::PI;
use wodw::configuracion::ConfiguracionWorker;
use wodw::ipc::mensajes::{
    FamiliaMedio, OrdenWorker, RespuestaWorker, CANALES_SALIDA, FRECUENCIA_SALIDA_HZ,
};
use wodw::maestro::medios::validar_bloque;
use wodw::worker::EstadoWorker;

const MP3: &[u8] = include_bytes!("datos/tono.mp3");
const OGG: &[u8] = include_bytes!("datos/tono.ogg");
const FLAC: &[u8] = include_bytes!("datos/tono.flac");
const OPUS: &[u8] = include_bytes!("datos/tono.opus");
const M4A: &[u8] = include_bytes!("datos/tono.m4a");
/// 1 s de 1 kHz + 20 kHz a 44,1 kHz.
const ULTRASONIDO: &[u8] = include_bytes!("datos/ultrasonido.flac");

/// Resultado de reproducir un medio entero en el Worker.
struct Reproducido {
    pcm: Vec<i16>,
    bloques: usize,
}

/// Abre el medio y pide bloques hasta el final; la apertura o un bloque
/// fallidos devuelven la respuesta recibida.
fn reproducir(
    familia: FamiliaMedio,
    datos: &[u8],
    cfg: &ConfiguracionWorker,
) -> Result<Reproducido, RespuestaWorker> {
    let mut worker = EstadoWorker::nuevo(cfg.clone());
    let apertura = worker
        .atender(OrdenWorker::AbrirMedio {
            id_tarea: 1,
            familia,
            datos_crudos: datos.to_vec(),
        })
        .expect("respuesta");
    if !matches!(apertura, RespuestaWorker::MedioAbierto { .. }) {
        return Err(apertura);
    }
    let mut pcm = Vec::new();
    for bloques in 1.. {
        let r = worker
            .atender(OrdenWorker::SiguienteBloque { id_tarea: 1 })
            .expect("respuesta");
        validar_bloque(&r, cfg).unwrap_or_else(|e| panic!("bloque rechazado por el Maestro: {e}"));
        let RespuestaWorker::BloqueMedio { audio_pcm, fin, .. } = r else {
            return Err(r);
        };
        pcm.extend(audio_pcm);
        if fin {
            return Ok(Reproducido { pcm, bloques });
        }
        assert!(bloques < 1000, "el medio no termina");
    }
    unreachable!()
}

/// Amplitud (0–1) de la componente `frecuencia` en un canal (Goertzel).
fn amplitud(pcm: &[i16], frecuencia: f64) -> f64 {
    let canal: Vec<f64> = pcm
        .iter()
        .step_by(usize::from(CANALES_SALIDA))
        .map(|&m| f64::from(m) / 32768.0)
        .collect();
    // Se descartan los bordes (transitorios del filtro y del codificador).
    let tramo = &canal[canal.len() / 5..canal.len() * 4 / 5];
    let w = 2.0 * PI * frecuencia / f64::from(FRECUENCIA_SALIDA_HZ);
    let (mut s1, mut s2) = (0.0, 0.0);
    for &x in tramo {
        let s = x + 2.0 * w.cos() * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    let potencia = s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2;
    2.0 * potencia.sqrt() / tramo.len() as f64
}

/// CA-A1 + CA-A2: los cinco formatos salen a 48 kHz estéreo con su duración y su tono.
#[test]
fn cinco_formatos_salen_a_48_khz_estereo_con_su_tono() {
    for (nombre, datos) in [
        ("mp3", MP3),
        ("ogg", OGG),
        ("flac", FLAC),
        ("opus", OPUS),
        ("m4a", M4A),
    ] {
        let r = reproducir(FamiliaMedio::Audio, datos, &ConfiguracionWorker::default())
            .unwrap_or_else(|e| panic!("{nombre}: {e:?}"));
        let segundos =
            r.pcm.len() as f64 / f64::from(FRECUENCIA_SALIDA_HZ) / f64::from(CANALES_SALIDA);
        assert!((0.95..1.1).contains(&segundos), "{nombre}: {segundos} s");
        assert_eq!(r.pcm.len() % usize::from(CANALES_SALIDA), 0);
        let tono = amplitud(&r.pcm, 1000.0);
        assert!(
            tono > 0.05,
            "{nombre}: el tono de 1 kHz desapareció ({tono})"
        );
        assert!(r.bloques > 1, "{nombre}: debe entregarse por bloques");
    }
}

/// CA-A2: lo audible pasa y los ultrasonidos se eliminan (20 kHz ≥ 40 dB por debajo).
#[test]
fn ultrasonidos_se_eliminan() {
    let r = reproducir(
        FamiliaMedio::Audio,
        ULTRASONIDO,
        &ConfiguracionWorker::default(),
    )
    .unwrap();
    let (audible, ultrasonido) = (amplitud(&r.pcm, 1000.0), amplitud(&r.pcm, 20_000.0));
    assert!(audible > 0.3, "1 kHz: {audible}");
    assert!(
        ultrasonido < audible / 100.0,
        "20 kHz: {ultrasonido} frente a {audible}"
    );
}

/// CA-A2: el limitador impide picos por encima del máximo configurado.
#[test]
fn limitador_impide_picos() {
    let cfg = ConfiguracionWorker {
        pico_maximo_audio_por_mil: 200,
        ..Default::default()
    };
    let r = reproducir(FamiliaMedio::Audio, ULTRASONIDO, &cfg).unwrap();
    let pico = r.pcm.iter().map(|m| m.unsigned_abs()).max().unwrap();
    assert!(u32::from(pico) <= 32_767 * 200 / 1000, "pico {pico}");
    assert!(
        u32::from(pico) > 32_767 * 200 / 1000 * 9 / 10,
        "el limitador no debe silenciar: pico {pico}"
    );
}

/// CA-C1: lo que no es audio se rechaza aunque se declare como audio.
#[test]
fn contenido_que_no_es_audio_se_rechaza() {
    let png = include_bytes!("../recursos/icono.png");
    for datos in [&png[..], b"texto cualquiera", &[]] {
        let r = reproducir(FamiliaMedio::Audio, datos, &ConfiguracionWorker::default());
        assert!(
            matches!(r, Err(RespuestaWorker::ErrorTarea { .. })),
            "{:?}",
            r.map(|x| x.bloques)
        );
    }
}

/// CA-C5: estructuras incoherentes se rechazan antes de decodificar.
#[test]
fn estructura_incoherente_se_rechaza() {
    // MP4 cuya primera caja declara más bytes de los que hay.
    let mut m4a = M4A.to_vec();
    m4a[..4].copy_from_slice(&u32::MAX.to_be_bytes());
    // OGG con una página alterada (la suma de comprobación ya no cuadra).
    let mut ogg = OGG.to_vec();
    ogg[40] ^= 0xFF;
    // FLAC cuyo bloque de metadatos declara más bytes de los que hay.
    let mut flac = FLAC.to_vec();
    flac[5..8].copy_from_slice(&[0xFF, 0xFF, 0xFF]);
    for (nombre, datos) in [("m4a", m4a), ("ogg", ogg), ("flac", flac)] {
        let r = reproducir(FamiliaMedio::Audio, &datos, &ConfiguracionWorker::default());
        match r {
            Err(RespuestaWorker::ErrorTarea { mensaje, .. }) => {
                assert!(mensaje.contains("estructura"), "{nombre}: {mensaje}")
            }
            otro => panic!(
                "{nombre}: se esperaba rechazo estructural: {:?}",
                otro.map(|x| x.bloques)
            ),
        }
    }
}

/// CA-A5: el Maestro rechaza bloques incoherentes de un Worker comprometido.
#[test]
fn maestro_rechaza_bloques_incoherentes() {
    let cfg = ConfiguracionWorker::default();
    let bloque = |audio_pcm: Vec<i16>| RespuestaWorker::BloqueMedio {
        id_tarea: 1,
        audio_pcm,
        fotogramas: vec![],
        fin: false,
    };
    assert!(validar_bloque(&bloque(vec![0, 0]), &cfg).is_ok());
    assert!(
        validar_bloque(&bloque(vec![0, 0, 0]), &cfg).is_err(),
        "longitud no múltiplo de 2"
    );
    assert!(
        validar_bloque(&bloque(vec![i16::MAX, 0]), &cfg).is_err(),
        "pico por encima del máximo"
    );
    let enorme = vec![0; usize::try_from(FRECUENCIA_SALIDA_HZ).unwrap() * 2 * 60];
    assert!(
        validar_bloque(&bloque(enorme), &cfg).is_err(),
        "bloque más largo que lo pedido"
    );
}

/// CA-C1: los tipos MIME de audio y vídeo se reconocen.
#[test]
fn tipos_mime_de_medios() {
    for mime in [
        "audio/mpeg",
        "audio/ogg",
        "audio/flac",
        "audio/opus",
        "audio/mp4",
        "audio/aac",
        "audio/wav",
        "audio/x-wav",
    ] {
        assert_eq!(
            FamiliaMedio::desde_mime(mime),
            Some(FamiliaMedio::Audio),
            "{mime}"
        );
    }
    for mime in ["video/mp4", "video/webm"] {
        assert_eq!(
            FamiliaMedio::desde_mime(mime),
            Some(FamiliaMedio::Video),
            "{mime}"
        );
    }
    assert_eq!(FamiliaMedio::desde_mime("application/pdf"), None);
}

/// CA-C1: el contenedor debe ser de la familia declarada.
#[test]
fn contenedor_de_otra_familia_se_rechaza() {
    let webm = include_bytes!("datos/video.webm");
    for (familia, datos) in [(FamiliaMedio::Audio, &webm[..]), (FamiliaMedio::Video, MP3)] {
        let r = reproducir(familia, datos, &ConfiguracionWorker::default());
        assert!(
            matches!(&r, Err(RespuestaWorker::ErrorTarea { mensaje, .. }) if mensaje.contains("no es un")),
            "{familia:?}: {:?}",
            r.map(|x| x.bloques)
        );
    }
}

/// CA-C5: el primer bloque FLAC debe ser STREAMINFO.
#[test]
fn flac_sin_streaminfo_se_rechaza() {
    let mut flac = FLAC.to_vec();
    flac[4] = (flac[4] & 0x80) | 4; // el bloque pasa a declararse como comentario
    let r = reproducir(FamiliaMedio::Audio, &flac, &ConfiguracionWorker::default());
    assert!(
        matches!(&r, Err(RespuestaWorker::ErrorTarea { mensaje, .. }) if mensaje.contains("STREAMINFO")),
        "{:?}",
        r.map(|x| x.bloques)
    );
}

/// CA-A2: el remuestreo no deja restos por encima del corte. Un tono de 17 kHz
/// a 44,1 kHz produce al remuestrear una imagen en 20,9 kHz; el paso bajo
/// posterior debe dejarla al menos 40 dB por debajo del tono.
#[test]
fn remuestreo_no_deja_imagenes_ultrasonicas() {
    use wodw::worker::medios::cadena_audio::CadenaAudio;
    let mut cadena = CadenaAudio::nueva(44_100, 1, &ConfiguracionWorker::default()).unwrap();
    let entrada: Vec<f32> = (0..44_100)
        .map(|i| 0.5 * (2.0 * PI * 17_000.0 * f64::from(i) / 44_100.0).sin() as f32)
        .collect();
    let pcm = cadena.procesar(&entrada);
    let (tono, imagen) = (amplitud(&pcm, 17_000.0), amplitud(&pcm, 20_900.0));
    assert!(
        imagen < tono / 100.0,
        "imagen {imagen} frente a tono {tono}"
    );
}
