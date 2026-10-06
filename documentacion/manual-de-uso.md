# Manual de uso de WoDW (Waves on Dark Web)

## 1. Qué es

WoDW es un navegador de **solo lectura** para la red Tor. Te enseña el texto, los enlaces y las
imágenes de las páginas, pero no ejecuta nada de ellas: ni JavaScript, ni complementos, ni
descargas. Todo el tráfico sale por Tor, que viene dentro de la propia aplicación.

## 2. Arrancar

- **Windows**: haz doble clic en `wodw.exe`. No hace falta instalarlo.
- **Linux / Tails**: ejecuta `./wodw`.

Al abrirse, la barra de abajo dice «Conectando a Tor… N %». Cuando pone **«Conectado a Tor.»**, ya
puedes navegar. Si escribes una dirección antes, la aplicación espera a que Tor esté listo.

## 3. Navegar y buscar

1. Escribe en la barra una dirección `.onion`, una URL que empiece por `http://` o `https://`, o
   unas palabras de búsqueda. Pulsa **Intro** o **Ir / Buscar**.
2. Las palabras se buscan con el motor que elijas en el desplegable de la izquierda (Ahmia, Torch o
   DuckDuckGo Onion).
3. Los enlaces de la página aparecen al final, en la sección **Enlaces**. Pulsa uno para abrirlo.
4. **⏴** y **⏵** van atrás y adelante. **+** abre una pestaña nueva y **×** cierra la pestaña.

Cada pestaña sale a internet por circuitos de Tor distintos de los de las demás.

## 4. Lo que la aplicación hace sola

No tienes que hacer nada para protegerte:

- Si un sitio manda respuestas o archivos hostiles, **se bloquea** durante la sesión, se cambian
  los circuitos y, si hace falta, se borra la pestaña. El motivo aparece en la propia pestaña.
- Si algo indica un ataque grave, salta el **pánico automático**: se borra todo de la memoria y la
  aplicación se cierra al instante.
- Si pasas 30 minutos sin usarla, se borran las pestañas y el historial.
- Al cerrar la ventana se borra todo.

## 5. Panel defensivo

El botón **IDS** de la barra abre y cierra el panel defensivo. Está en verde mientras no pasa nada y
cambia de color y de texto en cuanto hay un incidente. Ahí ves cuántos incidentes ha
habido en la sesión, de qué gravedad y qué se hizo con cada uno. También puedes pedir **circuitos
nuevos para todas las pestañas**.

## 6. Botón del pánico

El botón rojo **⚠ PÁNICO** (o pulsar `Esc` tres veces en menos de un segundo y medio) borra todo al
instante y cierra la aplicación. **No pide confirmación.** Si dejas el ratón quieto encima del botón,
aparece una viñeta que te lo recuerda.

## 7. Configuración

Copia `wodw.ejemplo.toml` con el nombre `wodw.toml` junto al ejecutable y cambia lo que necesites.
Cada ajuste está explicado en el propio archivo. Algunos ejemplos:

- **Que tu proveedor de internet no vea que usas Tor**: añade puentes en `[tor] puentes` (los
  consigues en https://bridges.torproject.org) y el programa del transporte en `[[tor.transportes]]`.
- **Solo sitios .onion**: `solo_onion = true` en `[red]`.
- **Que no se borre nada por inactividad**: `minutos_inactividad_purga = 0`.
- **Añadir un buscador**: un bloque `[[motores.lista]]` con `nombre`, `descripcion` y `plantilla`
  (la dirección de búsqueda con `{consulta}` donde van las palabras).

Si el archivo tiene un error, la aplicación no arranca y te dice qué campo falla.

## 8. ¿Hace falta una VPN?

No. En Tails no funciona. En Windows, una VPN del sistema trabaja por debajo de WoDW (tú → VPN →
Tor), pero quien te da la VPN ve tu dirección real. Si lo que quieres es ocultar que usas Tor, usa
puentes.

## 9. Lo que WoDW no puede hacer por ti

Ningún programa te hace invisible por sí solo. Si inicias sesión en un sitio con tu nombre, das
datos personales o descargas archivos con otro programa, WoDW no puede protegerte de eso. Las
limitaciones técnicas conocidas están en [arquitectura.md](arquitectura.md#limitaciones-conocidas).
