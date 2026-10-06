//! Canal asíncrono y enmarcado binario seguro para el protocolo IPC.
//!
//! Implementa la serialización y deserialización de mensajes tipados con prefijo de longitud
//! de 4 bytes (`u32` big-endian), imponiendo el límite configurado antes de reservar memoria
//! para evitar ataques de denegación de servicio por agotamiento de memoria.

use crate::error::{ErrorApp, Resultado};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Escribe un mensaje serializado con prefijo de longitud en un escritor asíncrono.
pub async fn escribir_mensaje_framed<W, M>(
    escritor: &mut W,
    mensaje: &M,
    limite: usize,
) -> Resultado<()>
where
    W: AsyncWrite + Unpin,
    M: serde::Serialize,
{
    let bytes_serializados = postcard::to_stdvec(mensaje)
        .map_err(|e| ErrorApp::Ipc(format!("Error serializando mensaje IPC: {}", e)))?;

    let longitud = u32::try_from(bytes_serializados.len())
        .ok()
        .filter(|_| bytes_serializados.len() <= limite)
        .ok_or(ErrorApp::LimiteExcedido {
            recurso: "mensaje IPC saliente",
            limite,
        })?;
    escritor
        .write_u32(longitud)
        .await
        .map_err(|e| ErrorApp::Ipc(format!("Error escribiendo longitud de trama IPC: {}", e)))?;
    escritor
        .write_all(&bytes_serializados)
        .await
        .map_err(|e| ErrorApp::Ipc(format!("Error escribiendo carga útil de trama IPC: {}", e)))?;
    escritor
        .flush()
        .await
        .map_err(|e| ErrorApp::Ipc(format!("Error vaciando búfer de trama IPC: {}", e)))?;

    Ok(())
}

/// Lee un mensaje deserializado con prefijo de longitud desde un lector asíncrono.
///
/// Retorna `Ok(None)` si el extremo opuesto cerró la conexión limpiamente antes de iniciar una trama.
pub async fn leer_mensaje_framed<R, M>(lector: &mut R, limite: usize) -> Resultado<Option<M>>
where
    R: AsyncRead + Unpin,
    M: for<'de> serde::Deserialize<'de>,
{
    let longitud = match lector.read_u32().await {
        Ok(len) => len as usize,
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(ErrorApp::Ipc(format!("Error leyendo longitud IPC: {}", e))),
    };

    if longitud > limite {
        return Err(ErrorApp::LimiteExcedido {
            recurso: "mensaje IPC entrante",
            limite,
        });
    }

    let mut buffer = vec![0u8; longitud];
    lector
        .read_exact(&mut buffer)
        .await
        .map_err(|e| ErrorApp::Ipc(format!("Error leyendo cuerpo de trama IPC: {}", e)))?;

    let mensaje = postcard::from_bytes(&buffer)
        .map_err(|e| ErrorApp::Ipc(format!("Error deserializando mensaje IPC: {}", e)))?;

    Ok(Some(mensaje))
}

/// Canal bidireccional asíncrono para intercambio de mensajes estructurados entre procesos.
pub struct CanalIpc<R, W> {
    lector: R,
    escritor: W,
    limite: usize,
}

impl<R, W> CanalIpc<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    /// Inicializa un canal IPC con un límite de `limite` bytes por trama.
    pub fn nuevo(lector: R, escritor: W, limite: usize) -> Self {
        Self {
            lector,
            escritor,
            limite,
        }
    }

    /// Envía un mensaje tipado a través del canal enmarcado.
    pub async fn enviar<M: serde::Serialize>(&mut self, mensaje: &M) -> Resultado<()> {
        escribir_mensaje_framed(&mut self.escritor, mensaje, self.limite).await
    }

    /// Recibe el siguiente mensaje tipado disponible en el canal.
    pub async fn recibir<M: for<'de> serde::Deserialize<'de>>(&mut self) -> Resultado<Option<M>> {
        leer_mensaje_framed(&mut self.lector, self.limite).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::mensajes::{Enlace, FormatoImagen, OrdenWorker, RespuestaWorker};

    /// Límite de trama en pruebas (16 MiB, el valor por defecto).
    const LIMITE_PRUEBA: usize = 16 * 1024 * 1024;
    use tokio::io::duplex;

    #[tokio::test]
    async fn canal_ipc_transmite_orden_y_respuesta_correctamente() {
        let (lado_maestro, lado_worker) = duplex(4096);
        let (lector_m, escritor_m) = tokio::io::split(lado_maestro);
        let (lector_w, escritor_w) = tokio::io::split(lado_worker);

        let mut canal_maestro = CanalIpc::nuevo(lector_m, escritor_m, LIMITE_PRUEBA);
        let mut canal_worker = CanalIpc::nuevo(lector_w, escritor_w, LIMITE_PRUEBA);

        // Maestro envía orden ProcesarHtml
        let orden = OrdenWorker::ProcesarHtml {
            id_tarea: 101,
            url_origen: "http://target.onion/index.html".to_string(),
            contenido_html: b"<html><head><title>Test</title></head><body>Hola</body></html>"
                .to_vec(),
        };

        canal_maestro.enviar(&orden).await.unwrap();

        // Worker recibe la orden
        let orden_recibida: Option<OrdenWorker> = canal_worker.recibir().await.unwrap();
        assert_eq!(Some(orden), orden_recibida);

        // Worker responde con HtmlProcesado
        let respuesta = RespuestaWorker::HtmlProcesado {
            id_tarea: 101,
            titulo: "Test".to_string(),
            texto_limpio: "Hola".to_string(),
            enlaces: vec![Enlace {
                texto: "Inicio".to_string(),
                url: "http://target.onion/".to_string(),
            }],
        };

        canal_worker.enviar(&respuesta).await.unwrap();

        // Maestro recibe la respuesta
        let respuesta_recibida: Option<RespuestaWorker> = canal_maestro.recibir().await.unwrap();
        assert_eq!(Some(respuesta), respuesta_recibida);
    }

    #[tokio::test]
    async fn canal_ipc_transmite_orden_imagen_rgba() {
        let (lado_maestro, lado_worker) = duplex(8192);
        let (lector_m, escritor_m) = tokio::io::split(lado_maestro);
        let (lector_w, escritor_w) = tokio::io::split(lado_worker);

        let mut canal_maestro = CanalIpc::nuevo(lector_m, escritor_m, LIMITE_PRUEBA);
        let mut canal_worker = CanalIpc::nuevo(lector_w, escritor_w, LIMITE_PRUEBA);

        let orden = OrdenWorker::ProcesarImagen {
            id_tarea: 202,
            formato: FormatoImagen::Png,
            datos_crudos: vec![0x89, 0x50, 0x4E, 0x47],
        };

        canal_maestro.enviar(&orden).await.unwrap();
        let recibida: Option<OrdenWorker> = canal_worker.recibir().await.unwrap();
        assert_eq!(Some(orden), recibida);

        let respuesta = RespuestaWorker::ImagenProcesada {
            id_tarea: 202,
            ancho: 1,
            alto: 1,
            datos_rgba: vec![255, 0, 0, 255], // 1 pixel rojo RGBA puro
        };

        canal_worker.enviar(&respuesta).await.unwrap();
        let resp_recibida: Option<RespuestaWorker> = canal_maestro.recibir().await.unwrap();
        assert_eq!(Some(respuesta), resp_recibida);
    }

    #[tokio::test]
    async fn canal_ipc_detecta_cierre_limpio_de_conexion() {
        let (lado_maestro, lado_worker) = duplex(1024);
        let (lector_m, _escritor_m) = tokio::io::split(lado_maestro);
        let (_, escritor_w) = tokio::io::split(lado_worker);

        // Cerramos el escritor del worker
        drop(escritor_w);

        let mut canal_maestro = CanalIpc::nuevo(lector_m, tokio::io::sink(), LIMITE_PRUEBA);
        let res: Option<RespuestaWorker> = canal_maestro.recibir().await.unwrap();
        assert_eq!(res, None, "Debe retornar None al cerrarse el canal");
    }

    #[tokio::test]
    async fn canal_ipc_rechaza_longitud_que_excede_limite_de_seguridad() {
        let (mut lado_maestro, mut lado_worker) = duplex(1024);

        // Escribir una longitud falsa gigante (32 MiB > 16 MiB)
        let longitud_excesiva: u32 = 32 * 1024 * 1024;
        lado_maestro.write_u32(longitud_excesiva).await.unwrap();
        drop(lado_maestro); // Cierra el emisor para evitar cuelgues si el guardián fallara

        let mut canal_worker = CanalIpc::nuevo(&mut lado_worker, tokio::io::sink(), LIMITE_PRUEBA);
        let resultado: Resultado<Option<OrdenWorker>> = canal_worker.recibir().await;

        assert!(resultado.is_err(), "Debe fallar al exceder el límite");
        let err = resultado.unwrap_err();
        assert!(matches!(err, ErrorApp::LimiteExcedido { .. }));
    }
}
