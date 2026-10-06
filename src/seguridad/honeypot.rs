//! Tecnología de engaño en memoria: página honeypot y trampa con firma.
//!
//! - [`HoneypotRam`]: página sin permisos (`PAGE_NOACCESS` / `PROT_NONE`). Un
//!   código inyectado que recorra la memoria del proceso a ciegas la toca y el
//!   sistema termina el proceso al instante (fallo de página). No genera un
//!   evento: su efecto es detener al atacante en seco.
//! - [`TrampaMemoria`]: búfer señuelo con firma. El IDS lo verifica
//!   periódicamente; una alteración delata una escritura arbitraria en memoria.

use crate::error::{ErrorApp, Resultado};
use crate::ids::eventos::{EventoDefensivo, NivelSeveridad, VectorAmenaza};

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Memory::{
    VirtualAlloc, VirtualFree, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_NOACCESS,
};

/// Página de memoria sin permisos. Cualquier acceso termina el proceso.
pub struct HoneypotRam {
    puntero: *mut u8,
    tamano: usize,
}

// SAFETY: el puntero nunca se desreferencia; solo se libera en `Drop`.
unsafe impl Send for HoneypotRam {}
// SAFETY: ver arriba; no hay mutación interior.
unsafe impl Sync for HoneypotRam {}

/// Tamaño de página asumido si el sistema no lo informa (4 KiB, el de x86-64 y ARM64 habitual).
const TAMANO_PAGINA_POR_DEFECTO: usize = 4096;

/// Tamaño de página del sistema.
fn tamano_pagina() -> usize {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: sysconf no tiene precondiciones.
        let tamano = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        usize::try_from(tamano).unwrap_or(TAMANO_PAGINA_POR_DEFECTO)
    }
    #[cfg(not(target_os = "linux"))]
    {
        TAMANO_PAGINA_POR_DEFECTO
    }
}

impl HoneypotRam {
    /// Reserva una página sin permisos.
    ///
    /// # Errors
    /// [`ErrorApp::Sandbox`] si el sistema rechaza la reserva.
    pub fn nueva_pagina_protegida() -> Resultado<Self> {
        let tamano = tamano_pagina();

        #[cfg(target_os = "windows")]
        {
            // SAFETY: reserva anónima sin dirección fija.
            let ptr = unsafe {
                VirtualAlloc(
                    std::ptr::null_mut(),
                    tamano,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_NOACCESS,
                )
            };
            if ptr.is_null() {
                return Err(ErrorApp::Sandbox(
                    "Fallo al asignar página PAGE_NOACCESS en Windows".to_string(),
                ));
            }
            Ok(Self {
                puntero: ptr as *mut u8,
                tamano,
            })
        }

        #[cfg(target_os = "linux")]
        {
            // SAFETY: mapeo anónimo privado sin dirección fija.
            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    tamano,
                    libc::PROT_NONE,
                    libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };
            if ptr == libc::MAP_FAILED {
                return Err(ErrorApp::Sandbox(
                    "Fallo al asignar página PROT_NONE en Linux".to_string(),
                ));
            }
            Ok(Self {
                puntero: ptr as *mut u8,
                tamano,
            })
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            Ok(Self {
                puntero: std::ptr::null_mut(),
                tamano: 0,
            })
        }
    }

    /// Retorna la dirección de memoria de la página trampa.
    pub fn direccion(&self) -> usize {
        self.puntero as usize
    }

    /// Retorna el tamaño en bytes de la página de memoria protegida.
    pub fn tamano(&self) -> usize {
        self.tamano
    }
}

impl Drop for HoneypotRam {
    fn drop(&mut self) {
        if self.puntero.is_null() {
            return;
        }

        #[cfg(target_os = "windows")]
        // SAFETY: puntero devuelto por VirtualAlloc, liberado una sola vez.
        unsafe {
            VirtualFree(self.puntero as *mut _, 0, MEM_RELEASE);
        }

        #[cfg(target_os = "linux")]
        // SAFETY: mapeo propio de `tamano` bytes, liberado una sola vez.
        unsafe {
            libc::munmap(self.puntero as *mut _, self.tamano);
        }
    }
}

/// Multiplicadores del hash polinómico de la firma (primos pequeños clásicos).
const MULTIPLICADOR_ETIQUETA: u64 = 31;
/// Ver [`MULTIPLICADOR_ETIQUETA`].
const MULTIPLICADOR_CEBO: u64 = 37;

/// Búfer señuelo con firma de integridad.
#[derive(Debug, Clone)]
pub struct TrampaMemoria {
    etiqueta: String,
    token_cebo: Vec<u8>,
    firma_esperada: u64,
}

impl TrampaMemoria {
    /// Constante mágica para centinelas de memoria.
    pub const VALOR_CENTINELA: u64 = 0xDEAD_BEEF_CAFE_BABE;

    /// Crea la trampa con un cebo verosímil.
    pub fn nueva(etiqueta: impl Into<String>, cebo: &[u8]) -> Self {
        let etiqueta_str = etiqueta.into();
        let firma = Self::calcular_firma(&etiqueta_str, cebo);

        Self {
            etiqueta: etiqueta_str,
            token_cebo: cebo.to_vec(),
            firma_esperada: firma,
        }
    }

    /// Calcula la firma de integridad de la estructura señuelo.
    fn calcular_firma(etiqueta: &str, cebo: &[u8]) -> u64 {
        let mut hash = Self::VALOR_CENTINELA;
        for b in etiqueta.bytes() {
            hash = hash
                .wrapping_mul(MULTIPLICADOR_ETIQUETA)
                .wrapping_add(b as u64);
        }
        for &b in cebo {
            hash = hash.wrapping_mul(MULTIPLICADOR_CEBO).wrapping_add(b as u64);
        }
        hash
    }

    /// Comprueba la integridad del señuelo en memoria.
    ///
    /// Si el buffer fue alterado o corrompido durante un intento de explotación en memoria,
    /// devuelve un [`EventoDefensivo`] crítico.
    pub fn verificar_integridad(&self) -> Option<EventoDefensivo> {
        let firma_actual = Self::calcular_firma(&self.etiqueta, &self.token_cebo);
        (firma_actual != self.firma_esperada).then(|| {
            EventoDefensivo::nuevo(
                NivelSeveridad::Critico,
                VectorAmenaza::MemoriaTrampaAlterada,
            )
        })
    }

    /// Método para simular una alteración de memoria en pruebas unitarias.
    #[cfg(test)]
    pub fn simular_corrupcion(&mut self) {
        if let Some(primer_byte) = self.token_cebo.first_mut() {
            *primer_byte ^= 0xFF;
        } else {
            self.token_cebo.push(0xAA);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_honeypot_asigna_pagina_protegida_con_direccion_valida() {
        let honeypot = HoneypotRam::nueva_pagina_protegida();
        assert!(
            honeypot.is_ok(),
            "Debe asignar la página honeypot correctamente"
        );
        let honeypot = honeypot.unwrap();
        assert_ne!(honeypot.direccion(), 0);
    }

    #[test]
    fn canary_memory_trap_verifica_integridad_limpia() {
        let trampa = TrampaMemoria::nueva("clave_privada_simulada", b"SECRET-PGP-BAIT-TOKEN");
        let evento = trampa.verificar_integridad();
        assert!(evento.is_none(), "En estado basal no debe haber alerta");
    }

    #[test]
    fn canary_memory_trap_detecta_corrupcion_y_genera_evento_critico() {
        let mut trampa = TrampaMemoria::nueva("clave_privada_simulada", b"SECRET-PGP-BAIT-TOKEN");
        trampa.simular_corrupcion();

        let evento = trampa.verificar_integridad();
        assert!(evento.is_some(), "Debe detectar la corrupción en memoria");
        let ev = evento.unwrap();
        assert_eq!(ev.severidad, NivelSeveridad::Critico);
        assert_eq!(ev.vector, VectorAmenaza::MemoriaTrampaAlterada);
    }
}
