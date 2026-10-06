# Funcionalidades de WoDW (Waves on Dark Web)

Aquí está todo lo que WoDW hace hoy. Lo que todavía no hace está en
[arquitectura.md, Limitaciones conocidas](arquitectura.md#limitaciones-conocidas).

## 1. Conexión y anonimato

- **Tor incluido.** WoDW lleva Tor dentro (`arti-client`). No hay que instalar nada más ni abrir
  puertos en el equipo.
- **Cada pestaña, sus propios circuitos.** Dos pestañas nunca comparten circuito, así que un sitio
  no puede relacionar lo que haces en una con lo que haces en otra.
- **Vanguards-Lite**, activado por defecto, contra los ataques que buscan descubrir tu nodo de
  entrada a Tor.
- **Puentes obfs4 y Snowflake** configurables en `wodw.toml`, para que tu proveedor de internet no
  sepa que usas Tor.
- **Servicios onion v3 y web normal a través de Tor.** Con `solo_onion = true` solo se permiten
  direcciones `.onion`.
- **HTTPS** sobre el circuito Tor cuando el destino lo usa.
- **Circuitos nuevos para todas las pestañas** con un botón del panel defensivo, o solos cuando la
  aplicación detecta una amenaza.

## 2. Búsqueda

La barra acepta una dirección `.onion`, una URL o simplemente palabras. Las palabras se buscan con
el motor elegido en el desplegable. Los motores por defecto son:

- **Ahmia:** motor curado con filtrado activo anti-abuso.
- **Torch:** motor sin moderación ni filtros editoriales.
- **DuckDuckGo Onion:** su versión HTML sin JavaScript, a través de su servicio onion.

Se pueden añadir otros en `[motores]` de `wodw.toml`. Excavator y Phobos no vienen por defecto
porque sus direcciones publicadas no tenían un formato onion v3 válido; si tienes una dirección
comprobada, puedes añadirlos.

## 3. Visor «Safest»

WoDW no ejecuta nada de lo que descarga. Muestra:

- El **texto**, el **título** y los **enlaces** de las páginas HTML, XHTML y de texto plano. Solo se
  pueden pulsar enlaces `http` y `https`.
- **Imágenes** PNG, JPEG, GIF, WebP y BMP reconstruidas píxel a píxel, sin metadatos y con límites
  contra bombas de descompresión.
- **Audio** WAV PCM de 16 bits pasado por un filtro que elimina ultrasonidos (por encima de
  18 kHz). Se informa de él, no se reproduce.
- Cualquier otro tipo de archivo (ejecutables, documentos) ni se procesa ni se guarda.
- **Letterboxing**: el área de contenido se redondea a pasos de 200×100 píxeles para que el tamaño
  de tu ventana no te identifique.

## 4. Aislamiento

- Cada recurso descargado se procesa en un **proceso aparte, encerrado y de usar y tirar**
  (sub-Worker efímero).
- **Windows**: mitigaciones de proceso, integridad baja y Job Object.
- **Linux y Tails**: `prctl`, Landlock y seccomp, incluido el bloqueo de creación de procesos.
  Comprobado en Linux 6.6; Tails aún no se ha probado.

## 5. Defensa automática

La aplicación se protege sin que tengas que pulsar nada:

- Bloquea durante la sesión los sitios que envían respuestas o contenido hostil.
- Cambia los circuitos ante una amenaza.
- Borra la pestaña afectada.
- Ejecuta el **pánico automático** si detecta un compromiso grave (un sub-Worker que intenta salir
  de su encierro, señuelos o firma en memoria alterados).
- Borra pestañas e historial tras 30 minutos sin uso (configurable) y al cerrar la ventana.
- **Botón del Pánico** manual, con una viñeta que explica lo que hace al pasar el ratón por encima,
  y el atajo `Esc` pulsado tres veces seguidas.
- **Panel defensivo** con los incidentes de la sesión y la respuesta que se aplicó a cada uno.

## 6. Señuelos

- Archivos señuelo en disco (opcionales), una página trampa en memoria y una firma en memoria que
  se comprueba cada poco.

## 7. Configuración

- `wodw.toml` opcional con todos los ajustes. `wodw.ejemplo.toml` explica cada uno con su valor por
  defecto.
- Si el archivo tiene un error, la aplicación no arranca y dice qué campo falla.

## 8. Presentación

- Icono propio en la ventana, la barra de tareas y el archivo `.exe` de Windows.
- Un único ejecutable portable: no necesita instalación.
