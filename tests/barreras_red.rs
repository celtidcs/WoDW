//! Pruebas de la pila HTTP del Maestro frente a respuestas malformadas u hostiles.
//! Viven fuera del código que vigilan.

use std::time::{Duration, Instant};
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt, DuplexStream};
use wodw::configuracion::ConfiguracionRedHttp;
use wodw::error::ErrorApp;
use wodw::maestro::red::http::ejecutar_peticion_en_flujo;
use wodw::maestro::red::PeticionHttp;

/// Configuración de prueba: sin jitter, límites pequeños y plazo corto.
fn red(plazo_ms: u64) -> ConfiguracionRedHttp {
    ConfiguracionRedHttp {
        jitter_min_ms: 0,
        jitter_max_ms: 0,
        limite_cuerpo_bytes: 1024,
        tiempo_espera_lectura_ms: plazo_ms,
        ..Default::default()
    }
}

/// Servidor que lee la petición y escribe `prefijo` seguido de `relleno` bytes `A`.
fn servidor(
    mut extremo: DuplexStream,
    prefijo: &'static [u8],
    relleno: usize,
) -> tokio::task::JoinHandle<usize> {
    tokio::spawn(async move {
        let mut peticion = [0u8; 4096];
        let _ = extremo.read(&mut peticion).await;
        if extremo.write_all(prefijo).await.is_err() {
            return 0;
        }
        let bloque = [b'A'; 1024];
        let mut enviados = 0;
        while enviados < relleno {
            if extremo.write_all(&bloque).await.is_err() {
                break;
            }
            enviados += bloque.len();
        }
        enviados
    })
}

async fn pedir(cliente: &mut DuplexStream, cfg: &ConfiguracionRedHttp) -> Result<(), ErrorApp> {
    let peticion = PeticionHttp::get("destino.onion", 80, "/").unwrap();
    ejecutar_peticion_en_flujo(cliente, &peticion, cfg)
        .await
        .map(|_| ())
}

/// Un tamaño de chunk gigantesco es un error, nunca un pánico.
#[tokio::test]
async fn chunk_gigante_es_error_y_no_panico() {
    let (mut cliente, extremo) = duplex(64 * 1024);
    let _s = servidor(
        extremo,
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nA\r\nFFFFFFFFFFFFFFFF\r\n",
        0,
    );
    let cfg = red(5_000);
    let r = tokio::spawn(async move { pedir(&mut cliente, &cfg).await }).await;
    let r = r.expect("la petición no debe entrar en pánico");
    assert!(matches!(r, Err(ErrorApp::LimiteExcedido { .. })), "{r:?}");
}

/// La línea de tamaño de chunk está acotada y se rechaza pronto (coste lineal).
#[tokio::test]
async fn linea_de_chunk_sin_fin_se_rechaza_pronto() {
    let (mut cliente, extremo) = duplex(64 * 1024);
    let enviador = servidor(
        extremo,
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n",
        20 * 1024 * 1024,
    );
    let inicio = Instant::now();
    let r = pedir(&mut cliente, &red(5_000)).await;
    let duracion = inicio.elapsed();
    drop(cliente);
    let aceptados = enviador.await.unwrap();
    assert!(matches!(r, Err(ErrorApp::LimiteExcedido { .. })), "{r:?}");
    assert!(
        aceptados < 256 * 1024,
        "el cliente consumió {aceptados} bytes"
    );
    assert!(duracion < Duration::from_secs(2), "tardó {duracion:?}");
}

/// CR/LF en la ruta, el host o el tipo de contenido se rechazan al construir.
#[test]
fn inyeccion_crlf_se_rechaza_al_construir() {
    assert!(PeticionHttp::get("x.onion", 80, "/a HTTP/1.1\r\nX-Inyectada: si").is_err());
    assert!(PeticionHttp::get("x.onion\r\nX: 1", 80, "/").is_err());
    assert!(PeticionHttp::post("x.onion", 80, "/", vec![], "text/plain\r\nX: 1").is_err());
    assert!(PeticionHttp::get("x.onion", 80, "/ruta?q=a%20b").is_ok());
}

/// Un servidor mudo agota el plazo configurado.
#[tokio::test]
async fn servidor_mudo_agota_el_plazo() {
    let (mut cliente, mut extremo) = duplex(4096);
    let _retener = tokio::spawn(async move {
        let mut peticion = [0u8; 4096];
        let _ = extremo.read(&mut peticion).await;
        tokio::time::sleep(Duration::from_secs(30)).await;
        drop(extremo);
    });
    let inicio = Instant::now();
    let r = pedir(&mut cliente, &red(200)).await;
    assert!(matches!(r, Err(ErrorApp::TiempoAgotado { .. })), "{r:?}");
    assert!(inicio.elapsed() < Duration::from_secs(5));
}

/// Content-Length superior al límite se rechaza antes de descargar el cuerpo.
#[tokio::test]
async fn content_length_excesivo_se_rechaza_sin_descargar() {
    let (mut cliente, extremo) = duplex(64 * 1024);
    let enviador = servidor(
        extremo,
        b"HTTP/1.1 200 OK\r\nContent-Length: 104857600\r\n\r\n",
        10 * 1024 * 1024,
    );
    let r = pedir(&mut cliente, &red(5_000)).await;
    drop(cliente);
    assert!(matches!(r, Err(ErrorApp::LimiteExcedido { .. })), "{r:?}");
    assert!(enviador.await.unwrap() < 256 * 1024);
}
