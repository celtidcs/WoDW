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
  **por cada recurso descargado** (una página, una imagen, un audio). El sub-Worker se encierra a
  sí mismo antes de leer nada, procesa ese único recurso, devuelve el resultado y muere.

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
                              sub-Worker confinado: worker/{html,imagen,audio}
```

Maestro y sub-Worker se hablan por la entrada y salida estándar del proceso, con mensajes tipados
serializados con `postcard` y precedidos de su longitud. El tamaño máximo de un mensaje es
configurable, así que un Worker comprometido tampoco puede inundar al Maestro.

## 2. Módulos

| Módulo | Qué hace |
|---|---|
| `configuracion/` | Lee `wodw.toml`, lo valida al arrancar y define los valores por defecto. |
| `maestro/red/http/` | Un cliente HTTP/1.1 pequeño y desconfiado: construye peticiones validadas (`peticion`), lee con plazos y límites (`lector`), interpreta la cabecera de forma estricta (`respuesta`) y lee el cuerpo con aritmética comprobada (`cuerpo`). |
| `maestro/red/cliente.rs` | El cliente de Tor (`arti-client`), con **Stream Isolation** por pestaña y rotación de circuitos. |
| `maestro/red/configuracion_tor.rs` | Rutas de datos de Tor, puentes, transportes enchufables y **Vanguards**. |
| `maestro/red/transporte.rs` | TLS (`native-tls`) sobre la conexión Tor cuando el destino es `https`. |
| `maestro/navegacion/` | Sigue redirecciones, decide qué hacer según el tipo de contenido, lo manda al Worker y aplica la **respuesta automática** si algo es hostil. |
| `maestro/proceso_worker.rs` | Lanza el sub-Worker efímero, lo mete en un Job Object en Windows, le da un plazo y reconoce si murió por intentar algo prohibido. |
| `maestro/sesion.rs` | Un hilo propio con el runtime de Tokio: arranca Tor, alimenta el IDS y vigila los señuelos. |
| `ipc/` | Los mensajes entre Maestro y Worker y el canal que los transporta. |
| `worker/` | Lo que hace el Worker: `html` (texto, título, enlaces), `imagen` (a píxeles RGBA), `audio` (WAV con filtro) y `sandbox/` (el encierro). |
| `ids/` | Los eventos de seguridad y el motor que decide la contramedida. |
| `seguridad/` | Señuelos en disco (canario), página trampa en memoria (Honeypot), firma en memoria y borrado seguro con `zeroize`. |
| `ui/` | La ventana: estado del navegador sin dependencias gráficas (`estado.rs`), paneles, icono, y todos los textos visibles en un único sitio (`textos.rs`). |

## 3. Red

- **Tor dentro de la aplicación.** WoDW usa `arti-client`, la implementación de Tor en Rust del
  Proyecto Tor. No hace falta instalar Tor, no se abre ningún puerto local y los nombres se
  resuelven siempre dentro de Tor, nunca con el DNS del sistema.
- **Stream Isolation.** Cada pestaña tiene su propio cliente aislado, así que sus conexiones nunca
  comparten circuito con las de otra pestaña. Un sitio no puede relacionar lo que haces en dos
  pestañas por el circuito. Rotar descarta esos clientes y las peticiones siguientes salen por
  circuitos nuevos.
- **Vanguards** en modo `lite` por defecto, para dificultar que un servicio onion malicioso
  descubra el nodo de entrada que usas.
- **Puentes y transportes enchufables** (obfs4, Snowflake) para que el proveedor de internet no
  vea que usas Tor.
- **HTTP desconfiado.** Se envían las cabeceras de Tor Browser, se espera un instante aleatorio
  antes de cada petición, hay plazo en cada lectura y escritura y límites de tamaño en cabeceras,
  cuerpo y bloques. Se rechazan las respuestas ambiguas (`Transfer-Encoding` raro, `Content-Length`
  contradictorio, cabeceras plegadas) y los saltos de línea o caracteres de control colados en el
  host, la ruta o el tipo de contenido.

## 4. El encierro del sub-Worker

El sub-Worker se encierra **antes** de crear su runtime y antes de recibir ningún dato de fuera.

**Linux y Tails** (`worker/sandbox/linux.rs`):

1. `prctl`: no se puede depurar ni volcar su memoria (`PR_SET_DUMPABLE=0`), no puede ganar
   privilegios (`PR_SET_NO_NEW_PRIVS=1`) y muere si muere el Maestro (`PR_SET_PDEATHSIG=SIGKILL`).
2. **Landlock**: sin acceso al sistema de archivos y, en núcleos que lo admiten, sin
   `bind`/`connect` TCP. Si el núcleo no aplica Landlock, el Worker se niega a procesar nada.
3. **seccomp** (primer filtro): crear sockets de red, conectarse, escuchar, ejecutar programas,
   depurar otros procesos o leer su memoria **mata el proceso al instante**. El Maestro reconoce esa
   muerte y la trata como un incidente crítico. La única excepción es `socketpair(AF_UNIX)`, que
   Tokio necesita para arrancar: es un par de extremos locales sin nombre que no llega ni a la red
   ni al disco.
4. **seccomp** (segundo filtro): crear procesos (`fork`, `vfork`, `clone` sin `CLONE_THREAD`,
   `clone3`) falla. Un hijo no heredaría la orden de morir con el Maestro y podría sobrevivir al
   borrado de emergencia. Crear hilos sí está permitido. El error devuelto es `ENOSYS` porque el
   filtro no puede leer las opciones de `clone3`; así la biblioteca de C reintenta con `clone`,
   donde sí se distingue un hilo de un proceso.

**Windows** (`worker/sandbox/windows.rs`):

1. Políticas de mitigación del proceso: sin código generado en ejecución, **sin procesos hijo**,
   sin puntos de extensión, comprobación estricta de descriptores, solo DLL firmadas por Microsoft
   y sin cargar imágenes remotas ni de integridad baja.
2. Nivel de **integridad baja**: Windows le niega escribir en el perfil del usuario y en casi todo
   el disco.
3. El Maestro lo mete en un **Job Object** que mata al Worker si se cierra el Maestro y no le deja
   tener más procesos. Si no puede meterlo, lo destruye y la navegación falla: nunca se procesa nada
   sin encierro.

## 5. Qué se muestra y cómo se limpia (modo «Safest»)

- **Páginas**: un recorrido propio, sin construir un DOM, saca el título, el texto visible y los
  enlaces. Ignora `script`, `style`, `noscript`, `template`, `iframe`, `object`, `embed`, `svg` y
  `math`, quita caracteres de control y marcas de dirección de texto (el truco *Trojan Source*) y
  decodifica las entidades HTML.
- **Enlaces**: se resuelven respecto a la página y solo se aceptan `http` y `https`. Nada de
  `javascript:`, `file:` o `data:`.
- **Imágenes**: PNG, JPEG, GIF, WebP y BMP. El formato declarado tiene que coincidir con los bytes,
  y el tamaño y la memoria se limitan **antes** de decodificar, para frenar las bombas de
  descompresión. El resultado son píxeles RGBA sin metadatos (ni EXIF ni perfiles de color). El
  Maestro vuelve a comprobar que los píxeles cuadran con las dimensiones: tampoco se fía del Worker.
- **Audio**: WAV PCM de 16 bits pasado por un filtro **paso bajo** Butterworth de orden 8 con corte
  en 18 kHz (configurable), que elimina ultrasonidos que podrían usarse para rastrear dispositivos.
  No se reproduce: solo se informa.
- **Todo lo demás** (ejecutables, PDF, documentos) no se procesa ni se guarda.

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
olvida los bloqueos y, si `panico.abortar_proceso` está activo, termina el proceso al instante con
`abort()`. Los sub-Workers mueren con el Maestro (Job Object en Windows, `PR_SET_PDEATHSIG` en Linux).

## 7. Señuelos

- **canario** en disco (desactivados por defecto, porque escriben en disco): archivos señuelo que
  se vigilan. Detectan que alguien los modifique, los sustituya, los borre o les quite el acceso.
  **No** detectan que alguien solo los lea; ningún sistema de archivos lo permite de forma fiable
  sin privilegios.
- **Honeypot** en memoria: una página de memoria sin permisos. Un código inyectado que recorra la
  memoria a ciegas la toca y el proceso termina en el acto.
- **Firma en memoria** que la sesión comprueba de forma periódica.

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

- **El sub-Worker de Windows conserva acceso a la red.** La integridad baja no lo impide.
  Cortarlo del todo exige AppContainer, que obliga a cambiar los permisos del propio ejecutable.
- **`DisallowWin32kSystemCalls` no se puede aplicar** porque el ejecutable único carga la parte
  gráfica de Windows (`user32.dll`). Haría falta un ejecutable de Worker separado y sin interfaz.
- **Tails no se ha probado.** El encierro de Linux se ha comprobado en un núcleo Linux 6.6.
- **Renderizado**: `eframe` dibuja con la GPU (`wgpu`), no por software.
- **No implementado** todavía: WebAssembly (`wasmtime`), `memfd`/`SCM_RIGHTS`, `mlock`, portapapeles
  que se borra solo, detección de homoglifos, PGP integrado, reproducción de audio y vídeo y
  normalización Unicode NFC.
