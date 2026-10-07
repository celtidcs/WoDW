//! Invariantes de la configuración, comprobadas al cargar (fail fast).

use super::secciones::{
    ConfiguracionInterfaz, ConfiguracionMotores, ConfiguracionPanico, ConfiguracionRedHttp,
    ConfiguracionWorker,
};
use super::ConfiguracionWodw;
use crate::error::{ErrorApp, Resultado};

/// Marcador que una plantilla de motor debe contener.
pub(crate) const MARCADOR_CONSULTA: &str = "{consulta}";

/// Longitud de la etiqueta de una dirección onion v3: 35 bytes en base32 = 56 caracteres
/// (rend-spec-v3, sección «Encoding onion addresses»).
const LONGITUD_ETIQUETA_ONION_V3: usize = 56;

/// Sufijo de los servicios onion.
const SUFIJO_ONION: &str = ".onion";

/// Comprueba que `host` sea una dirección onion v3 con formato válido:
/// 56 caracteres del alfabeto base32 en minúsculas (`a-z`, `2-7`) seguidos de `.onion`.
/// Admite subdominios delante de la etiqueta v3.
pub fn es_direccion_onion_v3(host: &str) -> bool {
    let Some(sin_sufijo) = host.strip_suffix(SUFIJO_ONION) else {
        return false;
    };
    let etiqueta = sin_sufijo.rsplit('.').next().unwrap_or_default();
    etiqueta.len() == LONGITUD_ETIQUETA_ONION_V3
        && etiqueta
            .bytes()
            .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
}

/// Construye un error de configuración para `campo`.
fn invalido(campo: &str, motivo: impl Into<String>) -> ErrorApp {
    ErrorApp::Configuracion {
        campo: campo.to_string(),
        motivo: motivo.into(),
    }
}

/// Falla si `valor` es cero.
fn exigir_positivo(campo: &str, valor: u64) -> Resultado<()> {
    if valor == 0 {
        return Err(invalido(campo, "debe ser mayor que cero"));
    }
    Ok(())
}

/// Valida la configuración completa.
pub(crate) fn validar(cfg: &ConfiguracionWodw) -> Resultado<()> {
    validar_red(&cfg.red)?;
    validar_motores(&cfg.motores)?;
    validar_worker(&cfg.worker)?;
    validar_limites_ipc(cfg)?;
    exigir_positivo("registro.max_entradas", cfg.registro.max_entradas as u64)?;
    for (campo, valor) in [
        ("actualizaciones.url_api", &cfg.actualizaciones.url_api),
        (
            "actualizaciones.url_publicaciones",
            &cfg.actualizaciones.url_publicaciones,
        ),
    ] {
        let url = url::Url::parse(valor).map_err(|e| invalido(campo, e.to_string()))?;
        if url.scheme() != "https" {
            return Err(invalido(campo, "debe ser https"));
        }
    }
    validar_panico(&cfg.panico)?;
    validar_interfaz(&cfg.interfaz)?;
    exigir_positivo(
        "reproduccion.bufer_audio_ms",
        cfg.reproduccion.bufer_audio_ms,
    )?;
    exigir_positivo(
        "reproduccion.max_fotogramas_en_bufer",
        cfg.reproduccion.max_fotogramas_en_bufer as u64,
    )?;
    if cfg.reproduccion.volumen_inicial_por_ciento > crate::unidades::POR_CIENTO {
        return Err(invalido(
            "reproduccion.volumen_inicial_por_ciento",
            "el volumen solo atenúa: máximo 100",
        ));
    }
    exigir_positivo("ids.capacidad_canal", cfg.ids.capacidad_canal as u64)?;
    exigir_positivo("ids.capacidad_difusion", cfg.ids.capacidad_difusion as u64)?;
    exigir_positivo("ids.limite_historial", cfg.ids.limite_historial as u64)?;
    exigir_positivo(
        "ids.intervalo_verificacion_ms",
        cfg.ids.intervalo_verificacion_ms,
    )
}

/// Valida la sección `[red]`.
fn validar_red(red: &ConfiguracionRedHttp) -> Resultado<()> {
    exigir_positivo("red.limite_cuerpo_bytes", red.limite_cuerpo_bytes as u64)?;
    exigir_positivo(
        "red.limite_cuerpo_medios_bytes",
        red.limite_cuerpo_medios_bytes as u64,
    )?;
    exigir_positivo(
        "red.limite_cabeceras_bytes",
        red.limite_cabeceras_bytes as u64,
    )?;
    exigir_positivo(
        "red.limite_linea_chunk_bytes",
        red.limite_linea_chunk_bytes as u64,
    )?;
    exigir_positivo(
        "red.tiempo_espera_conexion_ms",
        red.tiempo_espera_conexion_ms,
    )?;
    exigir_positivo("red.tiempo_espera_lectura_ms", red.tiempo_espera_lectura_ms)?;
    if red.jitter_min_ms > red.jitter_max_ms {
        return Err(invalido(
            "red.jitter_min_ms",
            "no puede superar jitter_max_ms",
        ));
    }
    for (campo, valor) in [
        ("red.user_agent", &red.user_agent),
        ("red.accept_language", &red.accept_language),
    ] {
        if valor.is_empty() || valor.chars().any(char::is_control) {
            return Err(invalido(campo, "vacío o con caracteres de control"));
        }
    }
    Ok(())
}

/// Valida la sección `[motores]`.
fn validar_motores(motores: &ConfiguracionMotores) -> Resultado<()> {
    if motores.lista.is_empty() {
        return Err(invalido("motores.lista", "debe haber al menos un motor"));
    }
    for motor in &motores.lista {
        if !motor.plantilla.contains(MARCADOR_CONSULTA) {
            return Err(invalido(
                "motores.lista.plantilla",
                format!("«{}» no contiene {MARCADOR_CONSULTA}", motor.nombre),
            ));
        }
        let url = url::Url::parse(&motor.plantilla.replace(MARCADOR_CONSULTA, ""))
            .map_err(|e| invalido("motores.lista.plantilla", e.to_string()))?;
        let host = url.host_str().unwrap_or_default();
        if host.ends_with(SUFIJO_ONION) && !es_direccion_onion_v3(host) {
            return Err(invalido(
                "motores.lista.plantilla",
                format!(
                    "«{}»: {host} no es una dirección onion v3 válida",
                    motor.nombre
                ),
            ));
        }
    }
    if !motores
        .lista
        .iter()
        .any(|m| m.nombre == motores.predeterminado)
    {
        return Err(invalido(
            "motores.predeterminado",
            "no coincide con ningún motor de la lista",
        ));
    }
    Ok(())
}

/// Margen de serialización sobre el cuerpo de un archivo dentro de una orden.
const MARGEN_SERIALIZACION: usize = crate::unidades::BYTES_POR_MIB;

/// Coherencia entre los límites de red y los del canal con el Worker.
fn validar_limites_ipc(cfg: &ConfiguracionWodw) -> Resultado<()> {
    let mayor_descarga = cfg
        .red
        .limite_cuerpo_bytes
        .max(cfg.red.limite_cuerpo_medios_bytes);
    if cfg.worker.limite_orden_ipc_bytes < mayor_descarga.saturating_add(MARGEN_SERIALIZACION) {
        return Err(invalido(
            "worker.limite_orden_ipc_bytes",
            "debe superar en 1 MiB el mayor límite de descarga de [red]",
        ));
    }
    let fotograma = cfg.worker.bytes_fotograma_maximo();
    if cfg.worker.limite_mensaje_ipc_bytes as u64 <= fotograma {
        return Err(invalido(
            "worker.limite_mensaje_ipc_bytes",
            format!(
                "debe caber un fotograma de la resolución máxima ({fotograma} bytes) y su audio"
            ),
        ));
    }
    if u32::try_from(cfg.worker.limite_orden_ipc_bytes).is_err() {
        return Err(invalido(
            "worker.limite_orden_ipc_bytes",
            "el prefijo de trama es u32",
        ));
    }
    Ok(())
}

/// Valida la sección `[worker]`.
pub(crate) fn validar_worker(worker: &ConfiguracionWorker) -> Resultado<()> {
    exigir_positivo(
        "worker.limite_mensaje_ipc_bytes",
        worker.limite_mensaje_ipc_bytes as u64,
    )?;
    if u32::try_from(worker.limite_mensaje_ipc_bytes).is_err() {
        return Err(invalido(
            "worker.limite_mensaje_ipc_bytes",
            "el prefijo de trama es u32",
        ));
    }
    exigir_positivo(
        "worker.lado_corto_maximo_px",
        u64::from(worker.lado_corto_maximo_px),
    )?;
    if worker.lado_largo_maximo_px < worker.lado_corto_maximo_px {
        return Err(invalido(
            "worker.lado_largo_maximo_px",
            "no puede ser menor que worker.lado_corto_maximo_px",
        ));
    }
    exigir_positivo(
        "worker.memoria_maxima_imagen_bytes",
        worker.memoria_maxima_imagen_bytes,
    )?;
    exigir_positivo(
        "worker.max_medios_por_pagina",
        worker.max_medios_por_pagina as u64,
    )?;
    exigir_positivo(
        "worker.max_caracteres_texto",
        worker.max_caracteres_texto as u64,
    )?;
    exigir_positivo(
        "worker.frecuencia_corte_audio_hz",
        u64::from(worker.frecuencia_corte_audio_hz),
    )?;
    exigir_positivo(
        "worker.duracion_bloque_ms",
        u64::from(worker.duracion_bloque_ms),
    )?;
    if worker.pico_maximo_audio_por_mil == 0
        || worker.pico_maximo_audio_por_mil > crate::unidades::POR_MIL
    {
        return Err(invalido(
            "worker.pico_maximo_audio_por_mil",
            "debe estar entre 1 y 1000",
        ));
    }
    exigir_positivo(
        "worker.memoria_maxima_worker_bytes",
        worker.memoria_maxima_worker_bytes,
    )?;
    exigir_positivo("worker.tiempo_espera_ms", worker.tiempo_espera_ms)
}

/// Valida la sección `[panico]`.
fn validar_panico(panico: &ConfiguracionPanico) -> Resultado<()> {
    if panico.pulsaciones < 2 {
        return Err(invalido(
            "panico.pulsaciones",
            "al menos 2 para evitar disparos accidentales",
        ));
    }
    exigir_positivo("panico.ventana_ms", panico.ventana_ms)
}

/// Valida la sección `[interfaz]`.
fn validar_interfaz(ui: &ConfiguracionInterfaz) -> Resultado<()> {
    let positivos = [
        ("interfaz.ancho_inicial", ui.ancho_inicial),
        ("interfaz.alto_inicial", ui.alto_inicial),
        ("interfaz.incremento_horizontal", ui.incremento_horizontal),
        ("interfaz.incremento_vertical", ui.incremento_vertical),
        ("interfaz.ancho_minimo", ui.ancho_minimo),
        ("interfaz.alto_minimo", ui.alto_minimo),
    ];
    for (campo, valor) in positivos {
        if !(valor.is_finite() && valor > 0.0) {
            return Err(invalido(campo, "debe ser un número positivo"));
        }
    }
    if ui.ancho_minimo > ui.ancho_maximo || ui.alto_minimo > ui.alto_maximo {
        return Err(invalido(
            "interfaz",
            "los mínimos no pueden superar los máximos",
        ));
    }
    exigir_positivo(
        "interfaz.longitud_titulo_pestana",
        ui.longitud_titulo_pestana as u64,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direcciones_v3_validas_e_invalidas() {
        assert!(es_direccion_onion_v3(
            "juhanurmihxlp77nkq76byazcldy2hlmovfu2epvl5ankdibsot4csyd.onion"
        ));
        assert!(es_direccion_onion_v3(
            "www.juhanurmihxlp77nkq76byazcldy2hlmovfu2epvl5ankdibsot4csyd.onion"
        ));
        assert!(!es_direccion_onion_v3(
            "2fd6avmvmvavahuqio2phiiym3ix7hx22mr3bsnqdUM.onion"
        ));
        assert!(!es_direccion_onion_v3(
            "phobosxikamwc366g4v5e6w6e2q4j2o4v4z3e3x.onion"
        ));
        assert!(!es_direccion_onion_v3("ejemplo.com"));
    }

    #[test]
    fn valores_por_defecto_son_validos() {
        validar(&ConfiguracionWodw::default()).unwrap();
    }

    #[test]
    fn motor_con_onion_invalida_se_rechaza() {
        let mut cfg = ConfiguracionWodw::default();
        cfg.motores.lista[0].plantilla =
            "http://phobosxikamwc366g4v5e6w6e2q4j2o4v4z3e3x.onion/search?query={consulta}".into();
        assert!(validar(&cfg).is_err());
    }

    #[test]
    fn jitter_invertido_se_rechaza() {
        let mut cfg = ConfiguracionWodw::default();
        cfg.red.jitter_min_ms = 500;
        cfg.red.jitter_max_ms = 10;
        assert!(validar(&cfg).is_err());
    }
}
