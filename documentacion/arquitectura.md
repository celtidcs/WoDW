# Arquitectura de WoDW (Waves on Dark Web)

Este documento explica cómo está construido WoDW por dentro y por qué. Describe solo lo que el
código hace hoy. Lo que todavía no hace está al final, en
[Limitaciones conocidas](#limitaciones-conocidas), y los fallos abiertos en
[defectos-conocidos.md](defectos-conocidos.md).

## La idea en una frase

Todo lo que llega de internet se trata como si fuera un ataque: se descarga por Tor, se abre en un
proceso aparte, encerrado y de usar y tirar, y a la ventana solo vuelve texto limpio, píxeles y
números. Si algo sale mal, la aplicación reacciona sola.

## 1. Un ejecutable, dos papeles

Hay un único ejecutable (`wodw` en Linux, `wodw.exe` en Windows) que puede trabajar de dos formas:

- **Maestro**: es el proceso que ves. Lleva la ventana, el cliente de Tor, el detector de
  incidentes (IDS) y decide qué hacer con cada página.
- **sub-Worker efímero**: el mismo ejecutable arrancado con `--modo-worker`. El Maestro lanza uno
  **por cada recurso descargado** (una página, una imagen) y otro por cada audio o vídeo que se
  reproduce, que vive solo mientras dura la reproducción. El sub-Worker se encierra a sí mismo
  antes de leer nada, procesa ese único recurso, devuelve el resultado y muere.

¿Por qué así? Los programas que interpretan formatos complejos (HTML, imágenes) son donde suelen
esconderse los fallos que un atacante aprovecha. Si un archivo malicioso consigue engañar al
decodificador, se queda atrapado en un proceso que no puede leer el disco, no puede conectarse a la
red, no puede lanzar programas y va a morir en unos milisegundos.

```text
┌──────────────────────── Maestro ────────────────────────┐
│ ui/ (egui)  ──órdenes──►  maestro/sesion.rs (hilo Tokio) │
│   ▲ eventos                 │                             │
│   └─────────────────────────┤                             │
│        maestro/navegacion ──┼─► maestro/red (arti-client) ──► Tor
│              │              │                             │
│              ▼              └─► ids/ (MotorIds)           │
│     maestro/proceso_worker ──IPC (postcard, tramas u32)─┐ │
└─────────────────────────────────────────────────────────┼─┘
                                                          ▼
             sub-Worker confinado: worker/{html, imagen, avif, av1, yuv, medios/}
```

Maestro y sub-Worker se hablan por la entrada y salida estándar del proceso, con mensajes tipados
serializados con `postcard` y precedidos de su longitud. Cada dirección tiene su tope
configurable: lo que el Maestro envía (el archivo entero, hasta 257 MiB) y lo que el Worker devuelve
(un bloque, hasta 48 MiB, lo justo para un fotograma 4K y su audio), así que un Worker comprometido
tampoco puede inundar al Maestro.

## 2. Módulos

| Módulo | Qué hace |
|---|---|
| `configuracion/` | Lee `wodw.toml`, lo valida al arrancar y define los valores por defecto. |
| `maestro/red/http/` | Un cliente HTTP/1.1 pequeño y desconfiado: construye peticiones validadas (`peticion`), lee con plazos y límites (`lector`), interpreta la cabecera de forma estricta (`respuesta`) y lee el cuerpo con aritmética comprobada (`cuerpo`). |
| `maestro/red/cliente.rs` | El cliente de Tor (`arti-client`), con **Stream Isolation** por pestaña y rotación de circuitos. |
| `maestro/red/configuracion_tor.rs` | Rutas de datos de Tor, puentes, transportes enchufables y **Vanguards**. |
| `maestro/red/transporte.rs` | TLS (`native-tls`) sobre la conexión Tor cuando el destino es `https`. |
| `maestro/navegacion/` | Sigue redirecciones, decide qué hacer según el tipo de contenido, lo manda al Worker y aplica la **respuesta automática** si algo es hostil. |
| `maestro/proceso_worker.rs` | Lanza el sub-Worker efímero (en Windows, suspendido dentro de un AppContainer sin red y de un Job Object con límite de memoria), le da un plazo y reconoce si murió por intentar algo prohibido. Permite varias peticiones al mismo Worker para los medios. |
| `maestro/medios/` | Reproducción en el Maestro: segunda validación de cada bloque (`validar_bloque`), tarea que pide bloques al Worker (`productor`) y estado compartido con búfer, reloj, pausa y volumen (`reproduccion`). |
| `maestro/versiones.rs` | Aviso de versión nueva: interpreta la respuesta de GitHub como no confiable. |
| `registro.rs` | Registro de la sesión con contenido y guardado elegibles. |
| `maestro/sesion.rs` | Un hilo propio con el runtime de Tokio: arranca Tor, alimenta el IDS y vigila los señuelos. |
| `ipc/` | Los mensajes entre Maestro y Worker y el canal que los transporta. |
| `worker/` | Lo que hace el Worker: `html` (texto, título, enlaces y medios incrustados), `imagen` y `avif` (a píxeles RGBA), `av1` y `yuv` (decodificación AV1 y reconstrucción de fotogramas), `medios/` (identificación y revisión estructural de contenedores, audio con `symphonia`/`opus-decoder` y su cadena de 48 kHz, vídeo en `fuente_video/` con una pieza para MP4, otra para WebM y otra para H.264), `entidades_html` (entidades HTML) y `sandbox/` (el encierro; `appcontainer/` separa el perfil y sus permisos, la creación del proceso y el citado de la línea de órdenes). |
| `seguridad/texto.rs`, `seguridad/enlaces.rs` | Higiene Unicode del texto y detección de enlaces engañosos, compartidas por Worker y Maestro. |
| `seguridad/sin_volcados.rs` | Terminación inmediata sin volcados de memoria, asignador que termina en vez de abortar y filtro de excepciones. |
| `ids/` | Los eventos de seguridad y el motor que decide la contramedida. |
| `seguridad/` | Señuelos en disco (canario), página trampa en memoria (Honeypot), firma en memoria y borrado seguro con `zeroize`. |
| `unidades.rs` | Las constantes de conversión de unidades (milisegundos, minutos, tantos por ciento y por mil, KiB), para que ninguna cifra de conversión quede suelta. |
| `ui/` | La ventana (`app/`: estado y eventos, anotaciones del registro y dibujado por separado), estado del navegador sin dependencias gráficas (`estado.rs`), paneles, reproductor (`reproductor.rs`, `salida_audio.rs` con `cpal`), panel del registro, fuentes embebidas (`fuentes.rs`), icono, y todos los textos visibles en un único sitio (`textos.rs`). |

## 3. Red

WoDW lleva Tor dentro gracias a `arti-client`, la implementación de Tor en Rust del propio Proyecto
Tor. No hace falta instalar Tor aparte, no se abre ningún puerto local y los nombres de dominio se
resuelven siempre dentro de Tor, nunca con el DNS del sistema.

Cada pestaña tiene su propio cliente aislado (*Stream Isolation*), de modo que sus conexiones nunca
comparten circuito con las de otra pestaña y un sitio no puede relacionar por el circuito lo que
haces en dos de ellas. Cuando se rotan los circuitos, esos clientes se descartan y las peticiones
siguientes salen por circuitos nuevos. Vanguards funciona en modo `lite` de serie, para dificultar
que un servicio onion malicioso descubra qué nodo de entrada usas, y los puentes con transportes
enchufables (obfs4, Snowflake) permiten ocultar al proveedor de internet que se está usando Tor.

El cliente HTTP es pequeño y desconfiado. Envía las mismas cabeceras que Tor Browser, espera un
instante aleatorio antes de cada petición, pone plazo a cada lectura y escritura y limita el tamaño
de las cabeceras, del cuerpo y de cada bloque. Rechaza las respuestas ambiguas, como un
`Transfer-Encoding` extraño, un `Content-Length` contradictorio o cabeceras plegadas, y también los
saltos de línea o caracteres de control colados en el host, la ruta o el tipo de contenido. Los
audios y vídeos tienen un tope de tamaño propio, mayor que el de las páginas.

## 4. El encierro del sub-Worker

El sub-Worker se encierra **antes** de crear su runtime y antes de recibir ningún dato de fuera.

**Linux y Tails** (`worker/sandbox/linux.rs`). La primera capa son las opciones de `prctl`: el
proceso no se puede depurar ni volcar (`PR_SET_DUMPABLE=0`), no puede ganar privilegios
(`PR_SET_NO_NEW_PRIVS=1`) y muere si muere el Maestro (`PR_SET_PDEATHSIG=SIGKILL`). Después,
Landlock le quita todo acceso al sistema de archivos y, en los núcleos que lo admiten, también la
posibilidad de hacer `bind` o `connect` por TCP. Si el núcleo no aplica Landlock, el Worker se niega
a procesar nada.

Encima van dos filtros seccomp. El primero mata el proceso al instante si intenta crear sockets de
red, conectarse, escuchar, ejecutar programas, depurar otros procesos o leer su memoria; el Maestro
reconoce esa muerte y la trata como un incidente crítico. La única excepción es
`socketpair(AF_UNIX)`, que Tokio necesita para arrancar y que crea un par de extremos locales sin
nombre que no llegan ni a la red ni al disco. El segundo filtro hace fallar cualquier intento de
crear procesos (`fork`, `vfork`, `clone` sin `CLONE_THREAD` y `clone3`), porque un hijo no heredaría
la orden de morir con el Maestro y podría sobrevivir al borrado de emergencia. Los hilos sí están
permitidos. El error que se devuelve es `ENOSYS`, porque el filtro no puede leer las opciones de
`clone3`; así la biblioteca de C reintenta con `clone`, donde sí se distingue un hilo de un proceso.
Por último, `RLIMIT_AS` limita la memoria que puede reservar.

**Windows** (`worker/sandbox/appcontainer.rs` y `worker/sandbox/windows.rs`). El Maestro crea el
Worker suspendido y dentro de un AppContainer sin ninguna capacidad. Al no tener `internetClient` ni
ninguna otra, Windows le impide abrir conexiones de red, incluso hacia `127.0.0.1`; se comprobó
contra internet real que el mismo programa conecta fuera de la jaula y no dentro. El AppContainer
necesita un perfil, vacío, en el registro del usuario, que se crea al arrancar WoDW y se borra al
cerrarlo con normalidad, y su identificador necesita permiso de lectura y ejecución sobre el propio
ejecutable. Antes de reanudarlo, el Maestro lo mete en un Job Object, así que el Worker no llega a
ejecutar ni una instrucción fuera de él.

El Job Object mata al Worker si se cierra el Maestro, le impide tener otros procesos y limita su
memoria, a 2 GiB de serie. Si algo falla al encerrarlo, el Maestro lo destruye y la navegación
falla: nunca se procesa nada sin encierro. Ya en marcha, el propio Worker activa las políticas de
mitigación de Windows (sin código generado en ejecución, sin procesos hijo, sin puntos de extensión,
comprobación estricta de descriptores, solo DLL firmadas por Microsoft y sin cargar imágenes remotas
ni de integridad baja) y baja a integridad baja, con lo que Windows le niega escribir en el perfil
del usuario y en casi todo el disco.

En los dos sistemas, el trabajo del Worker se hace en un hilo con 16 MiB de pila, porque los
decodificadores de vídeo desbordaban el hilo principal, que en Windows tiene solo 1 MiB.

**Sin volcados de memoria** (`seguridad/sin_volcados.rs`, en el Maestro y en el Worker). Las
primeras versiones salían con `abort()`, y en Windows eso hacía que el sistema escribiera en
`%LOCALAPPDATA%\CrashDumps` un volcado de toda la memoria de la sesión; se midieron unos 41 MB por
cada pánico. Ahora la salida inmediata usa `TerminateProcess` en Windows y `_exit` en Linux, que no
pasan por el informe de errores. El asignador de memoria global termina de la misma forma si el
sistema se queda sin memoria, en lugar de abortar, y un filtro de excepciones no controladas hace
lo mismo ante cualquier fallo. En Linux, además, el Maestro también se declara no volcable y sin
*core*.

## 5. Qué se muestra y cómo se limpia (modo «Safest»)

**Páginas.** Un recorrido propio del HTML, sin construir un DOM, saca el título, el texto visible,
los enlaces y la lista de medios incrustados (`img`, `audio`, `video` y `source`; solo sus
direcciones, porque nada se descarga sin pedirlo). Ignora por completo `script`, `style`,
`noscript`, `template`, `iframe`, `object`, `embed`, `svg` y `math`. Del texto quita los caracteres
de control, las marcas de dirección que usa el truco *Trojan Source* y los caracteres invisibles; lo
normaliza a NFC, decodifica las entidades HTML y lo corta si supera un tope de caracteres. El
Maestro repite la limpieza por su cuenta y marca los enlaces engañosos, es decir, aquellos cuyo
texto aparenta otro destino o cuyo nombre empieza por `xn--`. Los enlaces se resuelven respecto a la
página y solo se aceptan los `http` y `https`; nada de `javascript:`, `file:` o `data:`.

**Imágenes.** PNG, JPEG, GIF, WebP, BMP, TIFF, ICO y QOI se decodifican con `image`, y AVIF con
`avif-parse` y `re_rav1d`. La firma de los bytes tiene que coincidir con el formato declarado, y la
resolución, con un máximo de 4K en cualquier orientación, se comprueba en la cabecera **antes** de
decodificar. Lo que sale del Worker son píxeles RGBA sin metadatos, y el Maestro vuelve a comprobar
las dimensiones y el límite.

**Audio y vídeo** (`worker/medios/`). Se procesan por bloques de medio segundo y solo cuando el
usuario pulsa «Reproducir». Primero se identifica el contenedor por su firma y se exige que
pertenezca a la familia declarada, audio o vídeo. Después, una revisión estructural propia comprueba
el contenedor antes de que ningún decodificador lo toque: que las cajas MP4 encajen unas dentro de
otras, que las páginas Ogg tengan un CRC correcto, que un FLAC empiece por su bloque STREAMINFO, que
los fragmentos RIFF, la etiqueta ID3 y las tramas ADTS cuadren, y que los elementos EBML de
Matroska queden dentro de su contenedor.

El audio se decodifica con `symphonia` y `opus-decoder`, ambos escritos en Rust sin `unsafe`. Se
mezcla a estéreo, pasa por un filtro paso bajo Butterworth de orden 8 con corte en 18 kHz
(configurable), se remuestrea a 48 kHz con una interpolación cúbica propia, vuelve a pasar por el
mismo filtro y termina en un limitador de pico. El segundo filtro no es redundante: se midió que el
residuo del remuestreo en 20,9 kHz queda 58 dB por debajo con él y solo 9,6 dB sin él.

El vídeo MP4 se desmonta con `re_mp4` y se decodifica con OpenH264 de Cisco si es H.264, la única
pieza escrita en C, o con `re_rav1d` si es AV1. El WebM se desmonta con `matroska-demuxer` y se
decodifica con `re_rav1d`. Cada fotograma se reconstruye a RGBA en `worker/yuv.rs` y se entrega en
orden de presentación. Si el vídeo no tiene pista de audio, se envía silencio para que el reloj de
reproducción avance igualmente.

Ya en el Maestro, cada bloque se valida otra vez: que sea estéreo, que no supere el tope de
muestras ni el pico permitido, que los fotogramas cuadren con sus dimensiones y que las marcas de
tiempo no retrocedan. Un bloque incoherente se trata como un incidente. El sonido sale por `cpal`, y
el reloj que sincroniza la imagen es el audio que ya se ha reproducido.

Todo lo demás, desde ejecutables y PDF hasta cualquier formato fuera de la lista, ni se procesa ni
se guarda.

## 6. Detección y respuesta automática

La aplicación se defiende sola, sin que tengas que pulsar nada:

| Qué detecta | Gravedad | Qué hace |
|---|---|---|
| Respuesta hostil (límites superados, protocolo inválido, servidor que retiene la conexión) | Alta | Bloquea el sitio durante la sesión, cambia los circuitos y avisa |
| Contenido hostil (bomba de descompresión, Worker colgado, píxeles incoherentes) | Alta | Lo anterior y borra la pestaña |
| El sub-Worker se cae de forma anómala | Alta | Lo anterior y borra la pestaña |
| El sub-Worker intenta una llamada prohibida (seccomp) | Crítica | **Pánico automático** |
| Un señuelo o la firma en memoria aparecen alterados | Crítica | **Pánico automático** |
| 30 minutos sin usar la aplicación | — | Borra pestañas, historial y barra de direcciones |
| Cerrar la ventana | — | Borra todo |

El **pánico** (automático, con el **Botón del Pánico** o pulsando `Esc` tres veces seguidas)
sobrescribe con `zeroize` las pestañas, el historial, la barra y los textos, renueva los circuitos,
olvida los bloqueos y, si `panico.abortar_proceso` está activo, termina el proceso al instante sin
dejar volcados (véase el apartado 4). Los sub-Workers mueren con el Maestro (Job Object en Windows, `PR_SET_PDEATHSIG` en Linux).

## 7. Señuelos

Los señuelos en disco (el «canario») están desactivados de serie, porque escriben en el disco. Si se
activan, WoDW crea unos archivos y los vigila, y detecta que alguien los modifique, los sustituya,
los borre o les quite el acceso. Lo que no puede detectar es que alguien solo los lea, porque ningún
sistema de archivos lo permite de forma fiable sin privilegios.

En memoria hay dos trampas más. La primera es una página de memoria sin permisos (el **Honeypot**): un
código inyectado que recorra la memoria a ciegas acabará tocándola, y el proceso terminará en el
acto. La segunda es una firma que la sesión comprueba de forma periódica.

## 8. Interfaz

`egui`/`eframe`, sin WebView ni JavaScript. El contenido se muestra con *letterboxing* (márgenes
que redondean el área visible a pasos de 200×100 px) para que el tamaño de la ventana no sirva para
identificarte. El estado del navegador es una pieza sin dependencias gráficas que se prueba sin
abrir ventana; los textos y colores visibles viven todos en `ui/textos.rs`. El icono va incrustado
en el ejecutable: en la ventana mediante `eframe` y en el archivo `.exe` de Windows mediante
`build.rs`.

## 9. Configuración

Todo lo ajustable vive en `wodw.toml`, que es opcional (junto al ejecutable o indicado con
`--config <ruta>`). `wodw.ejemplo.toml` explica cada ajuste con su valor por defecto, y una prueba
comprueba que ambos coinciden. Una clave desconocida o un valor inválido impiden arrancar, con un
mensaje que dice qué campo falla.

## 10. Parche temporal de Arti en Windows

Arti 0.47 se bloquea en Windows al arrancar (se queda en el 15 %). La causa está en una dependencia
suya, `saturating-time`: un cálculo de límites de tiempo entra en un bucle infinito porque el reloj
de Windows tiene una resolución de 100 ns. Los desarrolladores de Arti ya lo conocen
([#2678](https://gitlab.torproject.org/tpo/core/arti/-/issues/2678),
[#2726](https://gitlab.torproject.org/tpo/core/arti/-/issues/2726)).

Mientras publican la corrección, WoDW compila con una copia corregida de esa dependencia en
`parches/saturating-time/`, conectada desde la sección `[patch.crates-io]` de `Cargo.toml`. El
cambio son cuatro líneas y está explicado en `parches/saturating-time/PARCHE-WODW.md`. La prueba
`tests/parche_saturating_time.rs` falla si alguien quita el parche antes de tiempo. Cuando Arti
publique la versión corregida, la copia y esa sección se eliminarán.

## Limitaciones conocidas

El AppContainer deja dos huellas en el sistema. Crea un perfil vacío en el registro del usuario, que
se borra al cerrar WoDW con normalidad; tras un pánico se queda y se reutiliza en el siguiente
arranque. Además, concede al identificador del Worker permiso de lectura y ejecución sobre el
ejecutable.

`egui` no compone texto complejo, así que el árabe, el persa y el jemer se ven con las letras
sueltas, sin unir ni reordenar. Tampoco se admiten los vídeos VP8, VP9 ni Ogg Theora, porque solo
existen decodificadores escritos en C que no se pueden incluir de forma portable.

La mitigación `DisallowWin32kSystemCalls` no se puede aplicar, porque el ejecutable único carga la
parte gráfica de Windows (`user32.dll`); haría falta un ejecutable de Worker separado y sin interfaz.

Tails todavía no se ha probado; el encierro de Linux se ha comprobado en un núcleo 6.6. La interfaz
se dibuja con la GPU (`wgpu`), no por software.

Quedan sin implementar el aislamiento con WebAssembly (`wasmtime`), el paso de datos por
`memfd`/`SCM_RIGHTS`, el bloqueo de memoria con `mlock`, un portapapeles que se borre solo y PGP
integrado.
