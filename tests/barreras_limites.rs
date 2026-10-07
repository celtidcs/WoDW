//! Barreras de CA-G2: límites separados por dirección y por tipo de contenido.
//!
//! - Un medio puede pesar más que una página (límite de cuerpo propio para
//!   audio y vídeo), sin subir el de las páginas.
//! - Lo que el Maestro envía al Worker (el archivo) tiene su propio límite; lo
//!   que el Worker devuelve al Maestro tiene otro, ajustado a un fotograma 4K.

use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};
use wodw::configuracion::{ConfiguracionRedHttp, ConfiguracionWodw};
use wodw::error::ErrorApp;
use wodw::ipc::CanalIpc;
use wodw::maestro::red::http::ejecutar_peticion_en_flujo;
use wodw::maestro::red::PeticionHttp;

const MIB: usize = 1024 * 1024;

/// Responde con `tipo` y un cuerpo de `bytes` bytes.
async fn descargar(tipo: &'static str, bytes: usize) -> Result<usize, ErrorApp> {
    let (mut cliente, mut servidor) = duplex(1024 * 1024);
    tokio::spawn(async move {
        let mut peticion = [0u8; 4096];
        let _ = servidor.read(&mut peticion).await;
        let cabecera =
            format!("HTTP/1.1 200 OK\r\nContent-Type: {tipo}\r\nContent-Length: {bytes}\r\n\r\n");
        let _ = servidor.write_all(cabecera.as_bytes()).await;
        let bloque = vec![b'A'; 64 * 1024];
        let mut restante = bytes;
        while restante > 0 {
            let n = restante.min(bloque.len());
            if servidor.write_all(&bloque[..n]).await.is_err() {
                return;
            }
            restante -= n;
        }
    });
    let red = ConfiguracionRedHttp {
        jitter_min_ms: 0,
        jitter_max_ms: 0,
        limite_cuerpo_bytes: MIB,
        limite_cuerpo_medios_bytes: 3 * MIB,
        ..Default::default()
    };
    let peticion = PeticionHttp::get("a.onion", 80, "/")?;
    ejecutar_peticion_en_flujo(&mut cliente, &peticion, &red)
        .await
        .map(|r| r.cuerpo().len())
}

#[tokio::test]
async fn los_medios_tienen_su_propio_limite_de_descarga() {
    assert_eq!(descargar("audio/mpeg", 2 * MIB).await.unwrap(), 2 * MIB);
    assert_eq!(descargar("video/webm", 2 * MIB).await.unwrap(), 2 * MIB);
    assert!(matches!(
        descargar("text/html", 2 * MIB).await,
        Err(ErrorApp::LimiteExcedido { .. })
    ));
    assert!(matches!(
        descargar("audio/mpeg", 4 * MIB).await,
        Err(ErrorApp::LimiteExcedido { .. })
    ));
}

#[tokio::test]
async fn cada_direccion_del_canal_tiene_su_limite() {
    let (a, b) = duplex(64 * 1024);
    let (la, ea) = tokio::io::split(a);
    let (lb, eb) = tokio::io::split(b);
    // Lado «Maestro»: envía hasta 4 MiB y recibe hasta 1 MiB. Lado «Worker»: al revés.
    let mut maestro = CanalIpc::con_limites(la, ea, MIB, 4 * MIB);
    let mut worker = CanalIpc::con_limites(lb, eb, 4 * MIB, MIB);
    let envio = tokio::spawn(async move {
        maestro.enviar(&vec![7u8; 3 * MIB]).await.unwrap();
        let respuesta: Result<Option<Vec<u8>>, _> = maestro.recibir().await;
        respuesta
    });
    let recibido: Vec<u8> = worker.recibir().await.unwrap().unwrap();
    assert_eq!(recibido.len(), 3 * MIB, "la orden grande cabe");
    assert!(
        matches!(
            worker.enviar(&vec![7u8; 2 * MIB]).await,
            Err(ErrorApp::LimiteExcedido { .. })
        ),
        "el Worker no puede devolver más que su límite"
    );
    drop(worker);
    let _ = envio.await;
}

#[test]
fn los_valores_por_defecto_cubren_un_fotograma_4k_y_un_medio_entero() {
    let cfg = ConfiguracionWodw::default();
    let fotograma_4k = 3840 * 2160 * 4;
    assert!(cfg.worker.limite_mensaje_ipc_bytes > fotograma_4k);
    assert!(cfg.worker.limite_orden_ipc_bytes > cfg.red.limite_cuerpo_medios_bytes);
    assert!(cfg.red.limite_cuerpo_medios_bytes > cfg.red.limite_cuerpo_bytes);
}

#[test]
fn limites_incoherentes_se_rechazan_al_arrancar() {
    let corto =
        "[red]\nlimite_cuerpo_medios_bytes = 1000000\n[worker]\nlimite_orden_ipc_bytes = 1000\n";
    assert!(
        ConfiguracionWodw::desde_texto(corto).is_err(),
        "la orden no cabría en el canal"
    );
    let sin_4k = "[worker]\nlimite_mensaje_ipc_bytes = 1000000\n";
    assert!(
        ConfiguracionWodw::desde_texto(sin_4k).is_err(),
        "un fotograma 4K no cabría"
    );
}

/// AUD-8: lo que tiene que caber en una respuesta es un fotograma de la
/// resolución configurada, no un 4K fijo.
#[test]
fn el_limite_de_respuesta_sigue_a_la_resolucion_configurada() {
    let ocho_k = "[worker]\nlado_largo_maximo_px = 7680\nlado_corto_maximo_px = 4320\n";
    assert!(
        ConfiguracionWodw::desde_texto(ocho_k).is_err(),
        "con 8K, un fotograma (132 MiB) no cabe en la respuesta por defecto (48 MiB)"
    );
    let ocho_k_con_sitio = format!(
        "{ocho_k}limite_mensaje_ipc_bytes = {}\n",
        7680 * 4320 * 4 + 16 * 1024 * 1024
    );
    assert!(
        ConfiguracionWodw::desde_texto(&ocho_k_con_sitio).is_ok(),
        "con sitio para el fotograma 8K, la configuración es válida"
    );
}
