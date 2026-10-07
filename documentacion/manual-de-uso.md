# Manual de uso de WoDW (Waves on Dark Web)

## 1. Qué es

WoDW es un navegador de **solo lectura** para la red Tor. Te enseña el texto, los enlaces, las
imágenes, el audio y el vídeo de las páginas, pero no ejecuta nada de ellas: ni JavaScript, ni
complementos, ni descargas. Todo lo que muestra se abre antes en un proceso aislado y sin red, y te
llega reconstruido desde cero. Todo el tráfico sale por Tor, que viene dentro de la aplicación.

## 2. Arrancar

WoDW son dos archivos que van juntos: `wodw`, la aplicación, y `wodw-worker`, el proceso aislado
que abre cada página. Descomprime la descarga completa y no los separes. En Windows basta con hacer
doble clic en `wodw.exe`; no hace falta instalarlo. En Linux o en Tails, ejecuta `./wodw` desde la
carpeta donde lo hayas descomprimido.

Al abrirse, la barra de abajo dice «Conectando a Tor… N %». Cuando pone **«Conectado a Tor.»**, ya
puedes navegar. Si escribes una dirección antes de que termine, la aplicación espera a que Tor esté
listo y la abre entonces.

## 3. Navegar y buscar

Escribe en la barra una dirección `.onion`, una URL que empiece por `http://` o `https://`, o
simplemente unas palabras, y pulsa **Intro** o **Ir / Buscar**. Las palabras se buscan con el
buscador que tengas elegido en el desplegable de la izquierda: Ahmia, DuckDuckGo Onion, OnionLand,
VormWeb o Tor66.

No todos son igual de fiables. Ahmia, DuckDuckGo Onion, OnionLand y VormWeb están verificados: una
fuente oficial de cada uno confirma su dirección. Tor66 no lo está, porque solo citan su dirección
algunos blogs; por eso lleva la marca ⚠, y si lo eliges su nombre se ve en ámbar. Al pasar el ratón
por cualquiera de ellos se explica el motivo.

El botón **Accesos** abre una lista de sitios útiles que se abren con un clic en la pestaña activa,
también con su fiabilidad y el motivo. Trae dos directorios antiphishing, dark.fail y tor.taxi, que
te dicen qué sitios están en línea y cuáles son sus direcciones auténticas, y la web del Tor
Project. Ten en cuenta que esos directorios también listan mercados ilegales.

Los enlaces de la página aparecen al final, en la sección **Enlaces**, y se abren al pulsarlos. Los
botones **⏴** y **⏵** van atrás y adelante, **+** abre una pestaña nueva y **×** cierra la que
tienes delante. Cada pestaña sale a internet por circuitos de Tor distintos de los de las demás.

Si un enlace aparenta llevar a una dirección pero en realidad lleva a otra, a su lado aparece un
aviso en naranja con el destino verdadero. Si no te fías, no lo pulses.

## 4. Imágenes, audio y vídeo

Las imágenes, audios y vídeos de una página no se descargan solos. Aparecen en una sección llamada
**«Imágenes, audios y vídeos de la página»**, y solo se abren cuando pulsas el que quieres ver. Se
abren en la misma pestaña, y con **⏴** vuelves a la página.

Una imagen se muestra en cuanto llega, rehecha a partir de sus píxeles. Un audio o un vídeo, en
cambio, espera a que pulses **⏵ Reproducir**. A partir de ahí tienes **⏸ Pausa**, **⏹ Parar** y el
control de volumen. La reproducción se detiene sola si cambias de pestaña, navegas a otra cosa, se
purga la sesión o pulsas el pánico.

WoDW abre imágenes PNG, JPEG, GIF, WebP, BMP, TIFF, ICO, QOI y AVIF; audio MP3, OGG, Opus, FLAC, WAV y
AAC/M4A; y vídeo MP4 (H.264) y WebM (AV1), siempre hasta 4K. Cualquier otro archivo aparece como «no
soportado» y no se abre.

Ten paciencia con los vídeos: por Tor, uno de pocos megas puede tardar uno o dos minutos en llegar.

## 5. Lo que la aplicación hace sola

No tienes que hacer nada para protegerte. Si un sitio manda respuestas o archivos hostiles, WoDW lo
bloquea durante el resto de la sesión, cambia los circuitos y, si hace falta, borra la pestaña; el
motivo aparece en la propia pestaña. Si algo indica un ataque grave, salta el pánico automático:
se borra todo de la memoria y la aplicación se cierra al instante.

Si pasas 30 minutos sin usarla, se borran las pestañas y el historial, y al cerrar la ventana se
borra todo.

## 6. Panel defensivo

El botón **IDS** de la barra abre y cierra el panel defensivo. Está en verde mientras no pasa nada,
y cambia de color y de texto en cuanto hay un incidente. En el panel ves cuántos incidentes ha
habido en la sesión, de qué gravedad y qué se hizo con cada uno. Desde ahí también puedes pedir
circuitos nuevos para todas las pestañas.

## 7. Botón del pánico

El botón rojo **⚠ PÁNICO** borra todo al instante y cierra la aplicación, sin dejar nada en el
disco. Lo mismo ocurre si pulsas `Esc` tres veces en menos de un segundo y medio. **No pide
confirmación.** Si dejas el ratón quieto encima del botón, aparece una viñeta que te lo recuerda.

## 8. Registro de la sesión

El botón **Registro** abre un panel donde decides si WoDW lleva un registro de lo que pasa y cómo
lo guarda. Debajo de cada opción se explica para qué sirve, qué apunta y qué consecuencias tiene;
merece la pena leerlo antes de elegir.

La primera elección es qué se apunta. Con **Seguridad**, la opción recomendada, el registro guarda
solo lo relacionado con la seguridad, como la conexión con Tor, los ataques, los bloqueos o los
fallos, y nunca las páginas que visitas. Con **Completo** apunta además cada dirección que abres. Es
útil para una auditoría, pero quien llegue a leer ese registro sabrá qué sitios visitaste.

La segunda elección es cómo se guarda. En modo **Manual**, también el recomendado, el registro vive
solo en la memoria hasta que pulsas **Guardar registro ahora**. En modo **Automático** se escribe en
el disco cada vez que cierras WoDW con normalidad.

Pasar a Completo o a Automático pide confirmación. El pánico borra el registro y nunca lo guarda. El
botón **Borrar registro** vacía el que hay en memoria, pero no toca los archivos que ya guardaste.

## 9. Aviso de versión nueva

Al conectarse a Tor, WoDW pregunta a GitHub, también por Tor, si hay una versión más reciente. Si la
hay, aparece abajo un aviso en verde con el enlace a su página. WoDW no descarga nada por su cuenta:
descárgala tú y comprueba su huella con `SHA256SUMS.txt`. Si prefieres que no haga esta consulta,
pon `comprobar_al_iniciar = false` en la sección `[actualizaciones]` de `wodw.toml`.

## 10. Configuración

Para cambiar algún ajuste, copia `wodw.ejemplo.toml` con el nombre `wodw.toml` junto al ejecutable y
edítalo. Cada opción está explicada dentro del propio archivo.

Por ejemplo, si no quieres que tu proveedor de internet vea que usas Tor, añade puentes en
`[tor] puentes` (los consigues en https://bridges.torproject.org) y el programa del transporte en
`[[tor.transportes]]`. Para navegar solo por sitios `.onion`, pon `solo_onion = true` en `[red]`. Si
no quieres que se borre nada por inactividad, pon `minutos_inactividad_purga = 0`. Las opciones con
las que arranca el registro están en `[registro]`, aunque también se cambian desde su panel, y el
volumen inicial y los búferes de reproducción, en `[reproduccion]`. Para añadir un buscador, crea un
bloque `[[motores.lista]]` con un `nombre`, una `descripcion` y una `plantilla`, que es la dirección
de búsqueda con `{consulta}` en el lugar donde van las palabras. Los accesos directos se añaden igual,
en bloques `[[accesos.lista]]` con `nombre`, `descripcion` y `url`. Puedes indicar la `fiabilidad`
(`verificado` o `sin_verificar`) y el `motivo`; si no lo haces, se muestran como «sin verificar».
WoDW comprueba que cada dirección `.onion` esté bien escrita, incluida su suma de control interna.

Si el archivo tiene un error, la aplicación no arranca y te dice qué campo falla: en Windows, en una
ventana de aviso; en Linux, en la terminal desde la que la lanzaste.

## 11. ¿Hace falta una VPN?

No. En Tails, de hecho, no funciona. En Windows, una VPN del sistema trabaja por debajo de WoDW (tú,
luego la VPN, luego Tor), pero quien te da la VPN ve tu dirección real. Si lo que quieres es ocultar
que usas Tor, usa puentes.

## 12. Lo que WoDW no puede hacer por ti

Ningún programa te hace invisible por sí solo. Si inicias sesión en un sitio con tu nombre, das
datos personales o descargas archivos con otro programa, WoDW no puede protegerte de eso. Las
limitaciones técnicas conocidas están en [arquitectura.md](arquitectura.md#limitaciones-conocidas).
