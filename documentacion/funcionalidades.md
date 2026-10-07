# Funcionalidades de WoDW (Waves on Dark Web)

Este documento describe todo lo que WoDW hace hoy. Lo que todavía no hace está en
[arquitectura.md, Limitaciones conocidas](arquitectura.md#limitaciones-conocidas).

## 1. Conexión y anonimato

WoDW lleva Tor dentro (`arti-client`), así que no hay que instalar nada más ni abrir puertos en el
equipo. Cada pestaña usa sus propios circuitos y dos pestañas nunca comparten uno, de modo que un
sitio no puede relacionar lo que haces en una con lo que haces en otra. Vanguards-Lite viene
activado de serie para dificultar los ataques que intentan descubrir tu nodo de entrada a Tor.

Si no quieres que tu proveedor de internet sepa que usas Tor, puedes configurar puentes obfs4 o
Snowflake en `wodw.toml`. WoDW abre tanto servicios onion v3 como la web normal, siempre a través de
Tor; con `solo_onion = true` se limita a las direcciones `.onion`. Cuando el destino usa HTTPS, la
conexión cifrada va dentro del circuito de Tor.

Los circuitos de todas las pestañas se pueden renovar con un botón del panel defensivo, y la propia
aplicación los renueva cuando detecta una amenaza.

## 2. Búsqueda

La barra de direcciones acepta una dirección `.onion`, una URL o simplemente palabras. Las palabras
se buscan con el motor elegido en el desplegable. De serie vienen tres:

**Ahmia:** un buscador curado que filtra activamente el contenido de abusos.

**Torch:** un buscador sin moderación ni filtros editoriales.

**DuckDuckGo Onion:** la versión HTML, sin JavaScript, del conocido buscador, servida desde su
propio servicio onion.

Puedes añadir otros en la sección `[motores]` de `wodw.toml`. Excavator y Phobos no vienen de serie
porque las direcciones que publican no tenían un formato onion v3 válido; si tienes una dirección
comprobada, puedes añadirlos tú.

## 3. Visor «Safest»

WoDW no ejecuta nada de lo que descarga. Cada archivo se abre en un proceso aislado y sin red, y al
navegador solo llegan datos reconstruidos desde cero, que este vuelve a comprobar antes de
mostrarlos. Antes de entregar un archivo a ningún decodificador, WoDW lo identifica por su
contenido, sin hacer caso de lo que diga el servidor, y revisa que su estructura interna sea
coherente. Solo se abren los formatos de una lista cerrada; los ejecutables, los documentos y
cualquier otro formato ni se procesan ni se guardan.

**Texto.** De las páginas HTML, XHTML y de texto plano se muestran el título, el texto y los
enlaces, y solo se pueden pulsar los enlaces `http` y `https`. Antes de mostrarlo, el texto se
limpia: se quitan los caracteres de control, las marcas que invierten la dirección de la escritura
(el truco conocido como *Trojan Source*) y los caracteres invisibles, como los de anchura cero o las
etiquetas Unicode, que pueden esconder mensajes. Después se normaliza a NFC, para que dos palabras
que parecen iguales lo sean de verdad, y se traducen las entidades HTML habituales (`&mdash;`,
`&aacute;`…). Si el texto de un enlace aparenta una dirección distinta de la real, o si el destino
usa un nombre internacionalizado (`xn--`) que podría imitar a otro, el enlace aparece marcado con
un aviso y la dirección verdadera. El texto tiene un tope de longitud y, si se recorta, se avisa.

WoDW incluye fuentes para los alfabetos latino, cirílico, griego, árabe, persa, chino, japonés,
coreano, georgiano y jemer. El árabe, el persa y el jemer se ven, pero con las letras sueltas, sin
unirse como lo haría un navegador corriente.

**Imágenes.** Admite PNG, JPEG, GIF (solo el primer fotograma), WebP, BMP, TIFF, ICO, QOI y AVIF.
Cada imagen se reconstruye píxel a píxel y pierde todos sus metadatos. El tamaño máximo es 4K, es
decir, 3840 × 2160 en cualquier orientación, y se comprueba en la cabecera antes de decodificar, de
forma que una imagen preparada para ocupar gigas al descomprimirse se rechaza sin llegar a abrirse.

**Audio.** Admite MP3, OGG/Vorbis, Opus, FLAC, WAV y AAC/M4A. Sea cual sea el original, el sonido
sale siempre a 48 kHz y en estéreo. Antes y después de cambiar la frecuencia pasa por un filtro
que elimina todo lo que está por encima de 18 kHz, donde podrían esconderse ultrasonidos usados para
rastrear dispositivos, y un limitador impide que supere −1 dBFS, para que ningún archivo pueda dar
un golpe de volumen dañino.

**Vídeo.** Admite MP4 (con H.264 y AAC) y WebM (con AV1 y Opus o Vorbis), hasta 4K. Cada fotograma
se vuelve a dibujar a partir de sus píxeles y el sonido pasa por la misma cadena que el audio. Del
archivo original no sale nada más: ni metadatos, ni subtítulos, ni pistas ocultas.

**Nada se reproduce ni se descarga por su cuenta.** El audio y el vídeo esperan a que pulses
«Reproducir», y tienes botones para pausar, parar y ajustar el volumen, que solo puede bajarse. La
reproducción se detiene sola si cambias de pestaña, navegas a otra cosa, se purga la sesión o
pulsas el pánico. Las imágenes, audios y vídeos que contiene una página aparecen en una lista
debajo del texto, y ninguno se descarga hasta que lo pulsas.

**Letterboxing.** El área de contenido se redondea a pasos de 200 × 100 píxeles para que el tamaño
exacto de tu ventana no sirva para identificarte.

## 4. Aislamiento

Cada recurso descargado se procesa en un proceso aparte, encerrado y de usar y tirar, que en la
documentación técnica se llama sub-Worker efímero. Si un archivo malicioso lograra engañar a un
decodificador, se encontraría atrapado en ese proceso, sin red y a punto de desaparecer.

En Windows, ese proceso se ejecuta dentro de un AppContainer sin ningún permiso, una jaula del
propio sistema que le impide abrir cualquier conexión de red. Además tiene activadas las
mitigaciones de proceso de Windows, funciona con integridad baja y está dentro de un Job Object que
limita su memoria. En Linux y Tails el encierro se hace con `prctl`, Landlock y seccomp, que le
impiden crear procesos y abrir conexiones de red, y su memoria se limita con `RLIMIT_AS`. Esto está
comprobado en Linux 6.6; en Tails todavía no se ha probado.

Ni el pánico, ni un fallo, ni quedarse sin memoria dejan en el disco un volcado con lo que había en
pantalla: WoDW se cierra sin pasar por el sistema de informes de errores.

## 5. Defensa automática

La aplicación se protege sin que tengas que pulsar nada. Bloquea durante la sesión los sitios que
envían respuestas o contenido hostil, cambia los circuitos ante una amenaza y borra la pestaña
afectada. Si detecta un compromiso grave, como un proceso aislado que intenta salir de su encierro
o unos señuelos alterados, ejecuta el pánico por su cuenta. Las pestañas y el historial se borran
tras 30 minutos sin uso, un plazo que puedes cambiar, y también al cerrar la ventana.

También puedes pulsar el botón del pánico tú mismo, o la tecla `Esc` tres veces seguidas. Al pasar
el ratón por encima del botón, una viñeta explica lo que hace. El panel defensivo muestra los
incidentes de la sesión y la respuesta que se aplicó a cada uno.

## 6. Registro de la sesión y versiones

El botón «Registro» abre un panel donde puedes activar un registro de lo que pasa durante la
sesión, para revisarlo después. Eliges qué apunta: «Seguridad» guarda los sucesos de seguridad sin
ninguna página visitada, y «Completo» lo apunta todo, direcciones incluidas. También eliges cómo se
guarda: en modo «Manual» solo cuando pulsas «Guardar registro ahora», y en modo «Automático» cada
vez que cierras WoDW con normalidad. Cada opción explica para qué sirve y qué consecuencias tiene,
y las dos más reveladoras piden confirmación antes de activarse. El pánico borra el registro sin
guardarlo.

Al conectarse a Tor, WoDW consulta en GitHub, a través de Tor, si hay una versión posterior y, si la
hay, la anuncia con su enlace. Nunca descarga ni instala nada. La consulta se puede desactivar en
`wodw.toml`.

## 7. Señuelos

WoDW puede dejar archivos señuelo en el disco, si lo activas, y mantiene en memoria una página
trampa y una firma que comprueba cada poco tiempo. Si alguien toca cualquiera de ellos, es señal de
que el equipo está comprometido y se dispara la defensa automática.

## 8. Configuración

Todos los ajustes viven en un archivo opcional, `wodw.toml`. El archivo `wodw.ejemplo.toml` explica
cada opción con su valor por defecto. Si el archivo tiene un error, la aplicación no arranca y dice
qué campo falla, para que nunca funcione con una configuración distinta de la que crees.

## 9. Presentación

WoDW tiene su propio icono en la ventana, en la barra de tareas y en el archivo `.exe` de Windows.
Es un único ejecutable portable que no necesita instalación.
