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
    /// Tamaño máximo de un audio o un vídeo descargado. 256 MiB caben en
    /// memoria sin riesgo y cubren vídeos de varios minutos.
    pub limite_cuerpo_medios_bytes: usize,
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
            limite_cuerpo_medios_bytes: 256 * MIB,
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

/// Hasta dónde se ha comprobado que una dirección es la auténtica del sitio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fiabilidad {
    /// Dirección confirmada por una fuente oficial del propio sitio (o por un
    /// directorio que guarda la firma del sitio) y abierta por Tor desde WoDW.
    Verificado,
    /// Dirección sin fuente oficial que la confirme: puede ser una copia
    /// falsa, estar caída o haber cambiado.
    #[default]
    SinVerificar,
}

/// Motivo que se muestra para una entrada añadida a mano sin motivo propio.
pub const MOTIVO_SIN_COMPROBAR: &str =
    "Añadido en wodw.toml; WoDW no ha comprobado que la dirección sea la auténtica.";

/// Motivo por defecto de las entradas que no declaran uno.
fn motivo_sin_comprobar() -> String {
    MOTIVO_SIN_COMPROBAR.to_string()
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
    /// Fiabilidad de la dirección; sin declarar, «sin verificar».
    #[serde(default)]
    pub fiabilidad: Fiabilidad,
    /// Por qué tiene esa fiabilidad (se muestra en la interfaz).
    #[serde(default = "motivo_sin_comprobar")]
    pub motivo: String,
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
    /// Los verificados primero y, después, los que no se han podido verificar.
    /// Fuera: Torch, Haystak y Phobos (sin servicio publicado en Tor: apagados o
    /// abandonados; la dirección de Torch de la 0.2.0 además era inválida),
    /// Excavator (sin dirección fiable) y la Hidden Wiki (muchas copias falsas).
    fn default() -> Self {
        let motor = |nombre: &str,
                     descripcion: &str,
                     plantilla: &str,
                     fiabilidad: Fiabilidad,
                     motivo: &str| MotorConfigurado {
            nombre: nombre.to_string(),
            descripcion: descripcion.to_string(),
            plantilla: plantilla.to_string(),
            fiabilidad,
            motivo: motivo.to_string(),
        };
        Self {
            predeterminado: "Ahmia".to_string(),
            lista: vec![
                motor(
                    "Ahmia",
                    "Buscador curado que filtra el contenido de abusos",
                    "http://juhanurmihxlp77nkq76byazcldy2hlmovfu2epvl5ankdibsot4csyd.onion/search/?q={consulta}",
                    Fiabilidad::Verificado,
                    "Dirección publicada en su web oficial (ahmia.fi) y comprobada abriéndola por Tor desde WoDW el 2026-10-07.",
                ),
                motor(
                    "DuckDuckGo Onion",
                    "Búsqueda en la web normal a través de Tor (versión HTML sin JavaScript)",
                    "https://duckduckgogg42xjoc72x3sjasowoarfbgcmvfimaftt6twagswzczad.onion/html/?q={consulta}",
                    Fiabilidad::Verificado,
                    "Dirección recogida por los directorios antiphishing dark.fail y tor.taxi y comprobada abriéndola por Tor desde WoDW el 2026-10-07.",
                ),
                motor(
                    "OnionLand",
                    "Buscador de servicios onion con un índice amplio",
                    "http://3bbad7fauom4d6sgppalyqddsqbf5u5p56b5k5uk2zxsy3d6ey2jobad.onion/search?q={consulta}",
                    Fiabilidad::Verificado,
                    "Dirección publicada en su web oficial (onionlandsearchengine.net) y comprobada abriéndola por Tor desde WoDW el 2026-10-07.",
                ),
                motor(
                    "VormWeb",
                    "Buscador de servicios onion (interfaz en alemán)",
                    "http://volkancfgpi4c7ghph6id2t7vcntenuly66qjt6oedwtjmyj4tkk5oqd.onion/search?q={consulta}",
                    Fiabilidad::Verificado,
                    "Dirección publicada en su web oficial (vormweb.de), firmada por el propio sitio en tor.taxi y comprobada abriéndola por Tor desde WoDW el 2026-10-07.",
                ),
                motor(
                    "Tor66",
                    "Buscador y directorio de servicios onion",
                    "http://tor66sewebgixwhcqfnp5inzp5x5uohhdy3kvtnyfxc2e5mxiuh34iid.onion/search?q={consulta}",
                    Fiabilidad::SinVerificar,
                    "Solo la citan blogs, no una fuente oficial: respondía por Tor el 2026-10-07, pero no se ha podido confirmar que sea el auténtico.",
                ),
            ],
        }
    }
}

/// Acceso directo a un sitio (por ejemplo, un directorio de direcciones).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccesoDirecto {
    /// Nombre visible.
    pub nombre: String,
    /// Qué es el sitio y qué conviene saber antes de abrirlo.
    pub descripcion: String,
    /// Dirección `http` o `https` que se abre al pulsarlo.
    pub url: String,
    /// Fiabilidad de la dirección; sin declarar, «sin verificar».
    #[serde(default)]
    pub fiabilidad: Fiabilidad,
    /// Por qué tiene esa fiabilidad (se muestra en la interfaz).
    #[serde(default = "motivo_sin_comprobar")]
    pub motivo: String,
}

/// Accesos directos del panel «Accesos».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfiguracionAccesos {
    /// Accesos disponibles, en el orden en que se muestran.
    pub lista: Vec<AccesoDirecto>,
}

impl Default for ConfiguracionAccesos {
    fn default() -> Self {
        let acceso =
            |nombre: &str, descripcion: &str, url: &str, fiabilidad: Fiabilidad, motivo: &str| {
                AccesoDirecto {
                    nombre: nombre.to_string(),
                    descripcion: descripcion.to_string(),
                    url: url.to_string(),
                    fiabilidad,
                    motivo: motivo.to_string(),
                }
            };
        Self {
            lista: vec![
                acceso(
                    "dark.fail",
                    "Directorio antiphishing: dice qué sitios están en línea y publica sus direcciones verificadas con PGP. Ojo: también lista mercados ilegales.",
                    "http://darkfailenbsdla5mal2mxn2uz66od5vtzd5qozslagrfzachha3f3id.onion/",
                    Fiabilidad::Verificado,
                    "Dirección publicada en su web oficial (dark.fail) y comprobada abriéndola por Tor desde WoDW el 2026-10-07.",
                ),
                acceso(
                    "tor.taxi",
                    "Directorio antiphishing con las direcciones firmadas por cada sitio. Ojo: también lista mercados ilegales.",
                    "http://tortaxi2dev6xjwbaydqzla77rrnth7yn2oqzjfmiuwn5h6vsk2a4syd.onion/",
                    Fiabilidad::Verificado,
                    "Dirección publicada en su web oficial (tor.taxi) y comprobada abriéndola por Tor desde WoDW el 2026-10-07.",
                ),
                acceso(
                    "Tor Project",
                    "Web oficial del Proyecto Tor.",
                    "http://2gzyxa5ihm7nsggfxnu52rck2vv4rvmdlkiu3zzui5du4xyclen53wid.onion/",
                    Fiabilidad::Verificado,
                    "Dirección recogida por dark.fail y tor.taxi y comprobada abriéndola por Tor desde WoDW el 2026-10-07.",
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
    /// Tamaño máximo de un mensaje del Worker al Maestro. 48 MiB: un fotograma
    /// 4K en RGBA (33,2 MB) más el audio de su bloque y margen.
    pub limite_mensaje_ipc_bytes: usize,
    /// Tamaño máximo de una orden del Maestro al Worker (lleva el archivo
    /// entero): el mayor límite de descarga más 1 MiB de margen.
    pub limite_orden_ipc_bytes: usize,
    /// Lado largo máximo de una imagen o un vídeo, en cualquier orientación.
    /// Se comprueba en la cabecera **antes** de decodificar (bombas de
    /// descompresión). 3840 px: 4K UHD.
    pub lado_largo_maximo_px: u32,
    /// Lado corto máximo de una imagen o un vídeo (2160 px, 4K UHD).
    pub lado_corto_maximo_px: u32,
    /// Memoria máxima que el decodificador de imágenes puede reservar.
    pub memoria_maxima_imagen_bytes: u64,
    /// Tope de caracteres del texto visible de una página. Un millón equivale a
    /// un libro largo; por encima solo sirve para agotar memoria o la interfaz.
    pub max_caracteres_texto: usize,
    /// Tope de imágenes, audios y vídeos listados de una página: 200 cubren
    /// cualquier galería razonable sin inflar la respuesta.
    pub max_medios_por_pagina: usize,
    /// Frecuencia de corte del filtro paso bajo de audio (Hz): por encima de
    /// 18 kHz casi nadie oye nada y ahí viajan las balizas ultrasónicas.
    pub frecuencia_corte_audio_hz: u32,
    /// Pico máximo del audio en milésimas de la escala completa. 891 ‰ ≈ −1 dBFS:
    /// margen frente a la saturación y protección de oídos y altavoces.
    pub pico_maximo_audio_por_mil: u16,
    /// Duración aproximada de cada bloque de un medio (ms). Medio segundo
    /// equilibra la latencia al pulsar «Reproducir» y el número de mensajes.
    pub duracion_bloque_ms: u32,
    /// Memoria máxima de un sub-Worker (CA-C3). Un archivo malicioso que
    /// intente agotar la memoria mata solo al Worker. 2 GiB caben un medio de
    /// 256 MiB, los búferes de un vídeo 4K y la pila de su hilo con holgura.
    pub memoria_maxima_worker_bytes: u64,
    /// Plazo máximo para que un sub-Worker entregue su respuesta.
    pub tiempo_espera_ms: u64,
}

impl ConfiguracionWorker {
    /// `true` si una imagen o un fotograma de `ancho`×`alto` cabe en el límite
    /// de resolución, en cualquier orientación (horizontal o vertical).
    pub fn admite_resolucion(&self, ancho: u32, alto: u32) -> bool {
        ancho.max(alto) <= self.lado_largo_maximo_px && ancho.min(alto) <= self.lado_corto_maximo_px
    }

    /// Bytes RGBA del mayor fotograma o imagen admitido por el límite de
    /// resolución: lo mínimo que tiene que caber en una respuesta del Worker.
    pub fn bytes_fotograma_maximo(&self) -> u64 {
        u64::from(self.lado_largo_maximo_px)
            * u64::from(self.lado_corto_maximo_px)
            * crate::ipc::mensajes::BYTES_POR_PIXEL_RGBA as u64
    }

    /// Valor absoluto máximo de una muestra PCM de 16 bits según
    /// `pico_maximo_audio_por_mil` (que la validación acota entre 1 y 1000).
    pub fn pico_maximo_muestra(&self) -> i16 {
        let pico = i32::from(i16::MAX) * i32::from(self.pico_maximo_audio_por_mil)
            / i32::from(crate::unidades::POR_MIL);
        i16::try_from(pico).unwrap_or(i16::MAX)
    }
}

impl Default for ConfiguracionWorker {
    fn default() -> Self {
        Self {
            limite_mensaje_ipc_bytes: 48 * MIB,
            limite_orden_ipc_bytes: 257 * MIB,
            lado_largo_maximo_px: 3840,
            lado_corto_maximo_px: 2160,
            memoria_maxima_imagen_bytes: 128 * MIB as u64,
            max_caracteres_texto: 1_000_000,
            max_medios_por_pagina: 200,
            frecuencia_corte_audio_hz: 18_000,
            pico_maximo_audio_por_mil: 891,
            duracion_bloque_ms: 500,
            memoria_maxima_worker_bytes: 2 * 1024 * MIB as u64,
            tiempo_espera_ms: 15_000,
        }
    }
}

/// Parámetros de la reproducción de audio y vídeo en el Maestro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfiguracionReproduccion {
    /// Audio que se pide por adelantado al Worker (ms). Dos segundos absorben
    /// los altibajos de la decodificación sin acumular memoria.
    pub bufer_audio_ms: u64,
    /// Fotogramas que se guardan por adelantado. Ocho fotogramas 4K ocupan
    /// unos 265 MB; es el tope de memoria de vídeo en el Maestro.
    pub max_fotogramas_en_bufer: usize,
    /// Volumen inicial en porcentaje (el volumen solo atenúa: máximo 100).
    pub volumen_inicial_por_ciento: u32,
}

impl Default for ConfiguracionReproduccion {
    fn default() -> Self {
        Self {
            bufer_audio_ms: 2_000,
            max_fotogramas_en_bufer: 8,
            volumen_inicial_por_ciento: 80,
        }
    }
}

/// Qué guarda el registro de la sesión.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContenidoRegistro {
    /// Solo sucesos de seguridad, sin páginas visitadas (lo más discreto).
    #[default]
    Seguridad,
    /// Además, cada página abierta y cada reproducción.
    Completo,
}

/// Cuándo llega el registro al disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuardadoRegistro {
    /// Solo al pulsar «Guardar registro» (lo más discreto).
    #[default]
    Manual,
    /// Al cerrar WoDW con normalidad, en `ruta_automatica`.
    Automatico,
}

/// Registro de la sesión.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfiguracionRegistro {
    /// Contenido inicial (se cambia desde la interfaz).
    pub contenido: ContenidoRegistro,
    /// Guardado inicial (se cambia desde la interfaz).
    pub guardado: GuardadoRegistro,
    /// Archivo del guardado automático y propuesta del manual; `None` usa
    /// `wodw-registro.txt` junto al ejecutable.
    pub ruta_automatica: Option<std::path::PathBuf>,
    /// Entradas que se conservan en memoria (las más antiguas se descartan):
    /// 5000 cubren una sesión larga con pocos cientos de kilobytes.
    pub max_entradas: usize,
}

impl Default for ConfiguracionRegistro {
    fn default() -> Self {
        Self {
            contenido: ContenidoRegistro::default(),
            guardado: GuardadoRegistro::default(),
            ruta_automatica: None,
            max_entradas: 5_000,
        }
    }
}

/// Aviso de versión nueva (consulta a GitHub por Tor al arrancar).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfiguracionActualizaciones {
    /// Si se consulta al conectar con Tor. Solo avisa: nunca descarga nada.
    pub comprobar_al_iniciar: bool,
    /// API de la última publicación del repositorio oficial.
    pub url_api: String,
    /// Prefijo de la página de una publicación (se le añade `vX.Y.Z`).
    pub url_publicaciones: String,
}

impl Default for ConfiguracionActualizaciones {
    fn default() -> Self {
        Self {
            comprobar_al_iniciar: true,
            url_api: "https://api.github.com/repos/celtidcs/WoDW/releases/latest".to_string(),
            url_publicaciones: "https://github.com/celtidcs/WoDW/releases/tag/".to_string(),
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
    /// Ancho mínimo de la ventana (puntos): por debajo, el campo de dirección
    /// se quedaba sin sitio entre los botones (medido: 11 puntos a 480).
    pub ancho_minimo_ventana: f32,
    /// Alto mínimo de la ventana (puntos).
    pub alto_minimo_ventana: f32,
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
            ancho_minimo_ventana: 800.0,
            alto_minimo_ventana: 500.0,
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
