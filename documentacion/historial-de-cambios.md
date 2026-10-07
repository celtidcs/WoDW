# Historial de cambios: WoDW (Waves on Dark Web)

## Versión 0.2.0

Esta versión trae una corrección de seguridad importante, así que conviene actualizar cuanto antes.
En la 0.1.0, cuando se pulsaba el botón del pánico en Windows, el propio sistema guardaba en
`%LOCALAPPDATA%\CrashDumps` una copia de la memoria de WoDW, con lo que hubiera en pantalla en ese
momento, y además un informe de error. Era justo lo contrario de lo que debe hacer un botón pensado
para no dejar rastro. La 0.2.0 cierra la aplicación en unas décimas de segundo sin que Windows
llegue a escribir nada. Si usaste el pánico con la versión anterior, borra los archivos
`wodw.exe.*.dmp` de esa carpeta; en los [defectos conocidos](defectos-conocidos.md) se explica con
más detalle.

### WoDW ya muestra audio y vídeo

Hasta ahora WoDW solo enseñaba texto e imágenes. A partir de esta versión también reproduce audio
(MP3, OGG, Opus, FLAC, WAV y AAC) y vídeo (MP4 con H.264 y WebM con AV1, hasta 4K). Ninguno de los
dos se reproduce tal como llega. El sonido se reconstruye siempre a 48 kHz, se le quitan los
ultrasonidos, que podrían servir para rastrear dispositivos, y pasa por un limitador que impide
picos de volumen dañinos. Del vídeo se vuelve a dibujar cada fotograma a partir de sus píxeles, de
modo que del archivo original no sale nada más: ni metadatos, ni subtítulos, ni pistas ocultas.

Nada suena ni se descarga por su cuenta. Las imágenes, audios y vídeos de una página aparecen en
una lista y solo se abren cuando los pulsas, y el audio y el vídeo esperan a que pulses
«Reproducir». Hay botones para pausar, parar y ajustar el volumen.

Las imágenes admiten ahora también TIFF, ICO, QOI y AVIF. Como antes, se rehacen píxel a píxel y
pierden todos sus metadatos. El límite es 4K, y el tamaño se comprueba antes de abrir el archivo,
no después.

### Más difícil de atacar

El proceso aislado que abre cada archivo ya no puede conectarse a internet en Windows. Ahora se
ejecuta dentro de un AppContainer, una jaula del propio Windows sin permisos, de modo que, aunque
un archivo malicioso lograra engañar al decodificador, no podría comunicarse con nadie saltándose
Tor. En Linux ese corte ya existía. En ambos sistemas el proceso tiene además un tope de memoria.

Cada archivo se identifica por su contenido, sin hacer caso del tipo que anuncia el servidor, y su
estructura interna se revisa antes de entregárselo a ningún decodificador. Los formatos admitidos
forman una lista cerrada: lo que no está en ella no se abre.

### Texto más limpio y en más idiomas

El texto de las páginas pierde los caracteres invisibles, que pueden esconder mensajes o engañar a
la vista, y se normaliza para que dos palabras que parecen iguales lo sean de verdad. Si un enlace
dice llevar a un sitio y en realidad lleva a otro, o usa un nombre disfrazado con caracteres de
otros alfabetos, WoDW lo avisa. Las entidades HTML habituales se muestran ya como su carácter.

WoDW incluye ahora fuentes para cirílico, griego, vietnamita, árabe, persa, chino, japonés, coreano,
georgiano y jemer. En árabe, persa y jemer las letras se ven, pero todavía sueltas, sin unirse como
lo haría un navegador corriente.

### Otras novedades

Puedes activar un registro de la sesión para revisar después qué ha pasado. Tú decides si apunta
solo los sucesos de seguridad o todo, direcciones incluidas, y si se guarda solo al cerrar o
únicamente cuando lo pides. Cada opción explica para qué sirve y qué consecuencias tiene, y las más
reveladoras piden confirmación.

Al abrirse, WoDW comprueba a través de Tor si hay una versión nueva en GitHub y, si la hay, te
avisa. Nunca descarga ni instala nada por su cuenta.

También hay arreglos menores en la interfaz: la ventana ya no puede encogerse hasta esconder la
barra de direcciones, y los errores de conexión de Tor se explican en español.

Si subes la resolución máxima en `wodw.toml`, WoDW comprueba ahora al arrancar que el canal con el
proceso aislado tenga sitio para un fotograma de ese tamaño, en lugar de suponer siempre 4K.

Por dentro, el código se ha reorganizado en piezas más pequeñas, cada una con una sola tarea, y las
cifras sueltas se han sustituido por constantes con nombre. El comportamiento no cambia: se ha
comprobado que el resultado de procesar cada archivo de prueba es idéntico byte a byte.

Para compilar en Linux hace falta ahora también `libasound2-dev`, la biblioteca de sonido.

## Versión 0.1.0

Primera versión publicada.

### Navegación
- Navegador de solo lectura sobre Tor con Tor incluido (`arti-client` 0.47): sin JavaScript, sin
  WebView y sin instalar nada más.
- Pestañas con circuitos de Tor separados, Vanguards-Lite, puentes obfs4/Snowflake y HTTPS sobre Tor.
- Búsqueda con Ahmia, Torch y DuckDuckGo Onion; se pueden añadir otros motores.

### Seguridad
- Cada página, imagen o audio se procesa en un proceso aparte, encerrado y de usar y tirar.
  Windows: mitigaciones de proceso, integridad baja y Job Object. Linux: `prctl`, Landlock y
  seccomp, incluido el bloqueo de creación de procesos.
- Cliente HTTP desconfiado: plazos, límites y rechazo de respuestas ambiguas o inyectadas.
- Imágenes reconstruidas a píxeles sin metadatos y con límites contra bombas de descompresión;
  audio con filtro contra ultrasonidos.
- Defensa automática: bloqueo de sitios hostiles, cambio de circuitos, borrado de pestañas, pánico
  automático ante compromiso, borrado por inactividad y al cerrar.
- Botón del pánico con viñeta explicativa y atajo `Esc` ×3.
- Señuelos en disco (opcionales) y en memoria.

### Interfaz y configuración
- Interfaz nativa con `egui`, letterboxing e icono propio.
- Toda la configuración en `wodw.toml`, con `wodw.ejemplo.toml` comentado.

### Conocido
- En Windows se usa temporalmente una copia corregida de la dependencia `saturating-time` hasta
  que Arti publique la corrección de su arranque en Windows. Detalles en
  [arquitectura.md](arquitectura.md) y [defectos-conocidos.md](defectos-conocidos.md).
