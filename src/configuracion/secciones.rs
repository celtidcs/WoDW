//! Secciones tipadas del archivo de configuración `wodw.toml`.
//!
//! Cada sección declara sus valores por defecto en un único sitio y se
//! deserializa con `deny_unknown_fields`, de modo que una clave mal escrita se
//! rechaza al arrancar en lugar de ignorarse en silencio.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Un mebibyte, base de los límites de tamaño expresados en bytes.
const MIB: usize = 1024 * 1024;
/// Un kibibyte.
const KIB: usize = 1024;

/// Parámetros de la pila HTTP del Proceso Maestro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfiguracionRedHttp {
    /// Tamaño máximo del cuerpo de una respuesta. 10 MiB cubre páginas y
    /// medios estáticos habituales sin permitir agotar la memoria.
    pub limite_cuerpo_bytes: usize,
    /// Tamaño máximo del bloque de cabeceras HTTP (64 KiB, como servidores comunes).
    pub limite_cabeceras_bytes: usize,
    /// Longitud máxima de una línea de tamaño de chunk. El RFC 9112 solo exige
    /// unos pocos dígitos hexadecimales y extensiones breves; 1 KiB es holgado.
    pub limite_linea_chunk_bytes: usize,
    /// Plazo para abrir un flujo Tor (los servicios onion pueden tardar decenas de segundos).
    pub tiempo_espera_conexion_ms: u64,
    /// Plazo máximo de inactividad entre dos lecturas del flujo.
    pub tiempo_espera_lectura_ms: u64,
    /// Retardo aleatorio mínimo previo a cada petición (dificulta correlación temporal).
    pub jitter_min_ms: u64,
    /// Retardo aleatorio máximo previo a cada petición.
    pub jitter_max_ms: u64,
    /// Número máximo de redirecciones HTTP seguidas antes de abortar.
    pub max_redirecciones: u8,
    /// User-Agent enviado; por defecto el de Tor Browser (que declara Windows en todas las plataformas).
    pub user_agent: String,
    /// Cabecera `Accept-Language` homogénea con Tor Browser.
    pub accept_language: String,
    /// Si es `true`, solo se permiten destinos `.onion`.
    pub solo_onion: bool,
}

impl Default for ConfiguracionRedHttp {
    fn default() -> Self {
        Self {
            limite_cuerpo_bytes: 10 * MIB,
            limite_cabeceras_bytes: 64 * KIB,
            limite_linea_chunk_bytes: KIB,
            tiempo_espera_conexion_ms: 120_000,
            tiempo_espera_lectura_ms: 60_000,
            jitter_min_ms: 20,
            jitter_max_ms: 120,
            max_redirecciones: 5,
            user_agent: "Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0"
                .to_string(),
            accept_language: "en-US,en;q=0.5".to_string(),
            solo_onion: false,
        }
    }
}

/// Modo de la defensa Vanguards de Arti contra el descubrimiento de guardias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModoVanguardias {
    /// Vanguards-Lite (recomendado para clientes).
    #[default]
    Lite,
    /// Vanguards completo (más latencia, más protección).
    Full,
    /// Desactivado.
    Disabled,
}

/// Transporte enchufable (pluggable transport) como `lyrebird` u `snowflake-client`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransporteEnchufable {
    /// Protocolos que ofrece el binario (por ejemplo `["obfs4"]` o `["snowflake"]`).
    pub protocolos: Vec<String>,
    /// Ruta al ejecutable del transporte.
    pub ruta: PathBuf,
    /// Argumentos adicionales del ejecutable.
    #[serde(default)]
    pub argumentos: Vec<String>,
}

/// Parámetros del cliente Tor embebido.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfiguracionTor {
    /// Modo Vanguards.
    pub vanguardias: ModoVanguardias,
    /// Líneas de puente en el formato de Tor (`obfs4 IP:PUERTO HUELLA cert=… iat-mode=0`).
    pub puentes: Vec<String>,
    /// Binarios de transportes enchufables necesarios para los puentes.
    pub transportes: Vec<TransporteEnchufable>,
    /// Directorio de estado de Arti; `None` usa el predeterminado de Arti.
    pub ruta_estado: Option<PathBuf>,
    /// Directorio de caché de Arti; `None` usa el predeterminado de Arti.
    pub ruta_cache: Option<PathBuf>,
}

/// Motor de búsqueda declarado en configuración.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotorConfigurado {
    /// Nombre visible.
    pub nombre: String,
    /// Descripción breve visible en la pantalla de bienvenida.
    pub descripcion: String,
    /// URL con el marcador `{consulta}` donde se inserta la búsqueda codificada.
    pub plantilla: String,
}

/// Catálogo de motores de búsqueda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfiguracionMotores {
    /// Nombre del motor seleccionado al arrancar.
    pub predeterminado: String,
    /// Motores disponibles.
    pub lista: Vec<MotorConfigurado>,
}

impl Default for ConfiguracionMotores {
    /// Solo incluye motores cuya dirección v3 tiene formato válido. Excavator y
    /// Phobos se retiraron porque sus direcciones no son comprobables; pueden
    /// añadirse en `wodw.toml`.
    fn default() -> Self {
        let motor = |nombre: &str, descripcion: &str, plantilla: &str| MotorConfigurado {
            nombre: nombre.to_string(),
            descripcion: descripcion.to_string(),
            plantilla: plantilla.to_string(),
        };
        Self {
            predeterminado: "Ahmia".to_string(),
            lista: vec![
                motor(
                    "Ahmia",
                    "Motor curado con filtrado activo anti-abuso",
                    "http://juhanurmihxlp77nkq76byazcldy2hlmovfu2epvl5ankdibsot4csyd.onion/search/?q={consulta}",
                ),
                motor(
                    "Torch",
                    "Motor sin moderación ni filtros editoriales",
                    "http://xmh57jrknzkhv6y3ls3ubitzfqnkrwxhopf5aygthi7d6rfdvgchu6ad.onion/cgi-bin/omega/omega?P={consulta}",
                ),
                motor(
                    "DuckDuckGo Onion",
                    "Búsqueda en web superficial a través de la red Onion (versión HTML sin JavaScript)",
                    "https://duckduckgogg42xjoc72x3sjasowoarfbgcmvfimaftt6twagswzczad.onion/html/?q={consulta}",
                ),
            ],
        }
    }
}

/// Parámetros del Proceso Worker. Se transmiten al subproceso serializados en
/// la línea de órdenes para que no necesite leer archivos tras confinarse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfiguracionWorker {
    /// Tamaño máximo de un mensaje IPC (16 MiB: cuerpo máximo de red más margen de serialización).
    pub limite_mensaje_ipc_bytes: usize,
    /// Ancho máximo de imagen aceptado antes de decodificar (protección contra bombas).
    pub ancho_maximo_imagen: u32,
    /// Alto máximo de imagen aceptado antes de decodificar.
    pub alto_maximo_imagen: u32,
    /// Memoria máxima que el decodificador de imágenes puede reservar.
    pub memoria_maxima_imagen_bytes: u64,
    /// Frecuencia de corte del filtro paso bajo de audio (Hz).
    pub frecuencia_corte_audio_hz: u32,
    /// Plazo máximo para que un sub-Worker entregue su respuesta.
    pub tiempo_espera_ms: u64,
}

impl Default for ConfiguracionWorker {
    fn default() -> Self {
        Self {
            limite_mensaje_ipc_bytes: 16 * MIB,
            ancho_maximo_imagen: 4096,
            alto_maximo_imagen: 4096,
            memoria_maxima_imagen_bytes: 128 * MIB as u64,
            frecuencia_corte_audio_hz: 18_000,
            tiempo_espera_ms: 15_000,
        }
    }
}

/// Parámetros del motor IDS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfiguracionIds {
    /// Capacidad del canal de entrada de eventos.
    pub capacidad_canal: usize,
    /// Capacidad del canal de difusión hacia la interfaz.
    pub capacidad_difusion: usize,
    /// Eventos recientes conservados en memoria.
    pub limite_historial: usize,
    /// Si las contramedidas se ejecutan automáticamente.
    pub mitigacion_automatica: bool,
    /// Intervalo de verificación de canarios y trampas de memoria.
    pub intervalo_verificacion_ms: u64,
}

impl Default for ConfiguracionIds {
    fn default() -> Self {
        Self {
            capacidad_canal: 256,
            capacidad_difusion: 128,
            limite_historial: 50,
            mitigacion_automatica: true,
            intervalo_verificacion_ms: 2_000,
        }
    }
}

/// Parámetros del botón del pánico.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfiguracionPanico {
    /// Pulsaciones consecutivas de `Esc` que lo disparan.
    pub pulsaciones: u8,
    /// Ventana máxima entre pulsaciones consecutivas.
    pub ventana_ms: u64,
    /// Si tras la purga se termina el proceso con `abort()`.
    pub abortar_proceso: bool,
}

impl Default for ConfiguracionPanico {
    fn default() -> Self {
        Self {
            pulsaciones: 3,
            ventana_ms: 1_500,
            abortar_proceso: true,
        }
    }
}

/// Parámetros de los archivos canario.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfiguracionCanarios {
    /// Desactivado por defecto: sembrar canarios escribe en disco, contrario al
    /// principio amnésico salvo que el directorio esté en RAM (Tails).
    pub habilitados: bool,
    /// Directorio de siembra; `None` usa un subdirectorio del temporal del sistema.
    pub directorio: Option<PathBuf>,
}

/// Parámetros de la interfaz gráfica y del letterboxing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfiguracionInterfaz {
    /// Ancho inicial de la ventana.
    pub ancho_inicial: f32,
    /// Alto inicial de la ventana.
    pub alto_inicial: f32,
    /// Incremento horizontal del letterboxing (200 px, como Tor Browser).
    pub incremento_horizontal: f32,
    /// Incremento vertical del letterboxing (100 px, como Tor Browser).
    pub incremento_vertical: f32,
    /// Ancho mínimo del área de contenido.
    pub ancho_minimo: f32,
    /// Alto mínimo del área de contenido.
    pub alto_minimo: f32,
    /// Ancho máximo del área de contenido.
    pub ancho_maximo: f32,
    /// Alto máximo del área de contenido.
    pub alto_maximo: f32,
    /// Caracteres visibles del título de una pestaña antes de truncarlo.
    pub longitud_titulo_pestana: usize,
}

impl Default for ConfiguracionInterfaz {
    fn default() -> Self {
        Self {
            ancho_inicial: 1000.0,
            alto_inicial: 800.0,
            incremento_horizontal: 200.0,
            incremento_vertical: 100.0,
            ancho_minimo: 600.0,
            alto_minimo: 400.0,
            ancho_maximo: 1400.0,
            alto_maximo: 1000.0,
            longitud_titulo_pestana: 20,
        }
    }
}

/// Respuestas automáticas sin intervención del usuario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfiguracionAutomatizacion {
    /// Bloquear durante la sesión los hosts que provoquen un incidente.
    pub bloquear_hosts_hostiles: bool,
    /// Minutos sin actividad tras los que se purgan pestañas e historial (0 = nunca).
    pub minutos_inactividad_purga: u64,
}

impl Default for ConfiguracionAutomatizacion {
    fn default() -> Self {
        Self {
            bloquear_hosts_hostiles: true,
            minutos_inactividad_purga: 30,
        }
    }
}
