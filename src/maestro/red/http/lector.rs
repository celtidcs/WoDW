//! Lector de flujo acotado en tamaño y en tiempo.
//!
//! Centraliza las dos defensas que toda lectura de red necesita: ninguna lectura
//! espera más que el plazo configurado y ningún búfer crece sin límite. La
//! búsqueda de delimitadores es incremental (no reexamina lo ya examinado), de
//! modo que el coste es lineal en los bytes recibidos.

use crate::error::{ErrorApp, Resultado};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};

/// Tamaño de cada lectura del flujo subyacente. Las celdas Tor transportan
/// unos 500 bytes, así que 4 KiB agrupa varias sin inflar el búfer.
const TAMANO_BLOQUE_LECTURA: usize = 4096;

/// Lector con búfer propio, límite por operación y plazo por lectura.
pub(crate) struct LectorAcotado<'a, S> {
    flujo: &'a mut S,
    pendiente: Vec<u8>,
    plazo: Duration,
}

impl<'a, S: AsyncRead + Unpin> LectorAcotado<'a, S> {
    /// Envuelve `flujo` imponiendo `plazo` a cada lectura individual.
    pub(crate) fn nuevo(flujo: &'a mut S, plazo: Duration) -> Self {
        Self {
            flujo,
            pendiente: Vec::new(),
            plazo,
        }
    }

    /// Lee un bloque y lo añade al búfer. Devuelve los bytes leídos (0 = fin de flujo).
    async fn rellenar(&mut self) -> Resultado<usize> {
        let mut bloque = [0u8; TAMANO_BLOQUE_LECTURA];
        let lectura = tokio::time::timeout(self.plazo, self.flujo.read(&mut bloque))
            .await
            .map_err(|_| ErrorApp::TiempoAgotado {
                operacion: "lectura HTTP",
                milisegundos: u64::try_from(self.plazo.as_millis()).unwrap_or(u64::MAX),
            })?;
        let leidos = lectura?;
        self.pendiente.extend_from_slice(&bloque[..leidos]);
        Ok(leidos)
    }

    /// Extrae los primeros `n` bytes del búfer.
    fn extraer(&mut self, n: usize) -> Vec<u8> {
        let resto = self.pendiente.split_off(n);
        std::mem::replace(&mut self.pendiente, resto)
    }

    /// Lee hasta `delimitador` (excluido, pero consumido).
    ///
    /// # Errors
    /// [`ErrorApp::LimiteExcedido`] si no aparece en `limite` bytes,
    /// [`ErrorApp::ProtocoloHttp`] si el flujo termina antes, y los de lectura.
    pub(crate) async fn leer_hasta(
        &mut self,
        delimitador: &[u8],
        limite: usize,
        recurso: &'static str,
    ) -> Resultado<Vec<u8>> {
        let mut examinados = 0usize;
        loop {
            let desde = examinados.saturating_sub(delimitador.len().saturating_sub(1));
            if let Some(relativa) = self.pendiente[desde..]
                .windows(delimitador.len())
                .position(|ventana| ventana == delimitador)
            {
                let posicion = desde + relativa;
                let contenido = self.extraer(posicion);
                self.extraer(delimitador.len());
                return Ok(contenido);
            }
            examinados = self.pendiente.len();
            if examinados > limite.saturating_add(delimitador.len()) {
                return Err(ErrorApp::LimiteExcedido { recurso, limite });
            }
            if self.rellenar().await? == 0 {
                return Err(ErrorApp::ProtocoloHttp(format!(
                    "flujo cerrado antes de completar {recurso}"
                )));
            }
        }
    }

    /// Lee exactamente `n` bytes.
    ///
    /// # Errors
    /// [`ErrorApp::ProtocoloHttp`] si el flujo termina antes, y los de lectura.
    pub(crate) async fn leer_exacto(&mut self, n: usize) -> Resultado<Vec<u8>> {
        while self.pendiente.len() < n {
            if self.rellenar().await? == 0 {
                return Err(ErrorApp::ProtocoloHttp(
                    "flujo cerrado antes de recibir el cuerpo completo".to_string(),
                ));
            }
        }
        Ok(self.extraer(n))
    }

    /// Lee hasta el fin del flujo sin superar `limite` bytes.
    ///
    /// # Errors
    /// [`ErrorApp::LimiteExcedido`] si llegan más de `limite` bytes, y los de lectura.
    pub(crate) async fn leer_hasta_fin(
        &mut self,
        limite: usize,
        recurso: &'static str,
    ) -> Resultado<Vec<u8>> {
        loop {
            if self.pendiente.len() > limite {
                return Err(ErrorApp::LimiteExcedido { recurso, limite });
            }
            if self.rellenar().await? == 0 {
                return Ok(std::mem::take(&mut self.pendiente));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    const PLAZO_PRUEBA: Duration = Duration::from_secs(5);

    #[tokio::test]
    async fn delimitador_partido_entre_dos_lecturas_se_encuentra() {
        let (mut a, mut b) = tokio::io::duplex(64);
        tokio::spawn(async move {
            b.write_all(b"abc\r").await.unwrap();
            tokio::task::yield_now().await;
            b.write_all(b"\nresto").await.unwrap();
        });
        let mut lector = LectorAcotado::nuevo(&mut a, PLAZO_PRUEBA);
        assert_eq!(
            lector.leer_hasta(b"\r\n", 16, "línea").await.unwrap(),
            b"abc"
        );
        assert_eq!(lector.leer_exacto(5).await.unwrap(), b"resto");
    }

    #[tokio::test]
    async fn hasta_fin_respeta_el_limite() {
        let (mut a, mut b) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            b.write_all(&[7u8; 10_000]).await.unwrap();
        });
        let mut lector = LectorAcotado::nuevo(&mut a, PLAZO_PRUEBA);
        let err = lector.leer_hasta_fin(100, "cuerpo").await.unwrap_err();
        assert!(matches!(err, ErrorApp::LimiteExcedido { limite: 100, .. }));
    }
}
