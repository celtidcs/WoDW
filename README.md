<p align="center">
  <img src="recursos/icono.png" alt="Icono de WoDW: una cebolla de Tor sobre olas" width="128" height="128">
</p>

# WoDW — Waves on Dark Web

Navegador de escritorio de **solo lectura** para la red Tor, escrito en Rust, para Windows y
Linux/Tails. Lleva Tor dentro, no ejecuta nada de lo que descarga y abre cada página en un proceso
aparte, encerrado y de usar y tirar. Si detecta un ataque, reacciona solo.

## Sobre Celtilander

Soy un usuario novel en el mundo de la programación y la IA. He «ayudado a crear» esta app y
espero que a alguien le pueda servir para algo... sin más.

Lo que no funciona no lo escondo: está escrito en
[defectos conocidos](documentacion/defectos-conocidos.md). Si encuentras algo que no esté ahí,
cuéntamelo.

Para realizar esta app he montado un sistema colaborativo y de consenso entre ChatGPT Codex,
Claude Code y Gemini Antigravity. Yo puse la idea y dirigía el proyecto, mientras que ellos
diseñaban, repartían y verificaban el trabajo. Ningún código fue revisado por el agente que lo
había escrito, y una vez terminado todo debía ser aprobado por unanimidad. Hasta que no se
conseguía el consenso, no se daba una tarea por concluida. Cuando por algún motivo no se llegaba
a un consenso, era yo quien tomaba la decisión final, así que los fallos que pueda haber serán
más míos que de ellos.

Saludos a todo el mundo.

## El problema que resuelve

Un navegador normal es un programa enorme que ejecuta código de cualquier sitio que visitas:
JavaScript, decodificadores de imagen, vídeo, tipos de letra. En la red Tor hay sitios que buscan
justo eso, un fallo en alguna de esas piezas para salir del navegador y llegar a tu equipo o
descubrir quién eres.

WoDW renuncia a casi todo a cambio de seguridad. No ejecuta JavaScript ni nada que venga de la
página. Solo muestra texto, enlaces e imágenes. Y aun así, desconfía de su propio código: lo poco
que tiene que interpretar lo hace dentro de una jaula de la que no se puede salir.

## Cómo se ve

<p align="center">
  <img src="recursos/capturas/wodw-ventana-principal.png" alt="La ventana principal de WoDW conectada a Tor, con la barra de búsqueda, el selector de motor, el indicador del IDS y el botón del pánico">
</p>

La ventana recién abierta y ya **conectada a Tor** (abajo a la izquierda). Arriba están las
pestañas y la barra de navegación: atrás y adelante, el **selector de buscador** (Ahmia, Torch o
DuckDuckGo Onion), la barra donde escribir una dirección `.onion` o unas palabras, el indicador
verde del **IDS**, que abre el panel defensivo, y el botón rojo del **pánico**, que borra todo y
cierra la aplicación. Si dejas el ratón quieto sobre él, una viñeta te avisa de lo que hace.

## Qué hace

- **Navega por Tor sin instalar Tor.** Lo lleva dentro (`arti-client`, la implementación en Rust
  del Proyecto Tor). No abre puertos en tu equipo y nunca usa el DNS del sistema.
- **Separa tus pestañas.** Cada una sale por sus propios circuitos, así que un sitio no puede
  relacionar lo que haces en dos pestañas distintas.
- **Busca** en Ahmia, Torch o DuckDuckGo Onion, y puedes añadir otros buscadores.
- **Muestra solo lo inofensivo**: el texto, el título y los enlaces de las páginas; las imágenes
  rehechas píxel a píxel y sin metadatos; los audios WAV filtrados contra ultrasonidos. Ni
  ejecutables, ni documentos, ni descargas.
- **Se defiende sola.** Bloquea los sitios que envían cosas hostiles, cambia de circuitos, borra la
  pestaña afectada y, si detecta un compromiso grave, lo borra todo y se cierra.
- **No deja rastro** en la sesión: borra pestañas e historial tras 30 minutos sin uso y al cerrar.
- **Oculta que usas Tor**, si quieres, con puentes obfs4 o Snowflake.
- **Es portable**: un único ejecutable, sin instalación.

## Cómo lo hace

**Dos papeles, un ejecutable.** El proceso que ves (el *Maestro*) lleva la ventana, Tor y el
detector de incidentes. Cuando descarga algo, no lo abre él: lanza una copia de sí mismo en modo
*Worker*, que se encierra, procesa ese único recurso, devuelve texto limpio o píxeles y muere. Hay
un Worker nuevo para cada página, imagen o audio. Si un archivo malicioso consigue engañar al
decodificador, se queda atrapado en un proceso que va a morir en milisegundos.

**La jaula.** Antes de leer un solo byte de fuera, el Worker se encierra:

- En **Linux y Tails**, con Landlock (sin acceso al disco ni a la red) y dos filtros seccomp: uno
  que lo mata si intenta abrir conexiones, ejecutar programas o espiar otros procesos, y otro que le
  impide crear procesos. Además, muere si muere el Maestro.
- En **Windows**, con las mitigaciones del sistema (sin procesos hijo, sin código generado al vuelo,
  solo bibliotecas firmadas por Microsoft), con nivel de integridad baja (no puede escribir en tu
  perfil) y dentro de un Job Object que lo mata si se cierra el Maestro.

**Desconfianza a cada paso.** El cliente HTTP pone plazo a cada lectura, limita el tamaño de todo y
rechaza las respuestas ambiguas o con caracteres colados. Las imágenes se miden antes de abrirlas,
para frenar las «bombas» que ocupan gigas al descomprimirse. Y el Maestro vuelve a comprobar lo que
le devuelve el Worker: tampoco se fía de él.

**Respuesta automática.** Un detector de incidentes (IDS) clasifica lo que pasa por gravedad y
actúa sin preguntarte. Si el Worker intenta salir de su jaula, o algún señuelo en disco o en
memoria aparece tocado, salta el **pánico**: se sobrescribe con ceros todo lo que hay en memoria de
la sesión y la aplicación se cierra al instante.

Todos los detalles están en la [arquitectura](documentacion/arquitectura.md).

## Lo que no hace

- No ejecuta JavaScript, así que muchas webs modernas no se verán bien o no funcionarán.
- No reproduce audio ni vídeo, ni abre documentos.
- No te hace invisible: si inicias sesión con tu nombre o das datos personales, ningún programa
  puede protegerte de eso.
- Tiene limitaciones técnicas conocidas, por ejemplo que en Windows el Worker todavía puede abrir
  conexiones de red. Están todas en [defectos conocidos](documentacion/defectos-conocidos.md).

## Usarla

Descarga `wodw.exe` desde [Releases](https://github.com/celtidcs/WoDW/releases) y ábrelo. La barra
de abajo te dirá cuándo está conectado a Tor. Cómo navegar, qué hace sola y cómo configurarla está
en el [manual de uso](documentacion/manual-de-uso.md).

Para ajustar algo, copia `wodw.ejemplo.toml` como `wodw.toml` junto al ejecutable: cada opción
está explicada dentro.

## Compilar y ejecutar

Necesitas [Rust](https://rustup.rs) estable. En Windows, además, Visual Studio Build Tools con
«Desarrollo para el escritorio con C++». En Linux, las bibliotecas de la
[lista de requisitos](documentacion/infraestructura/requisitos.md).

```bash
git clone https://github.com/celtidcs/WoDW.git
cd WoDW
cargo build --release
```

El ejecutable queda en `target/release/wodw` (o `wodw.exe`). Para comprobar que todo está bien:

```bash
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
cargo audit
```

La [guía de clonado](documentacion/infraestructura/guia-de-clonado.md) lo explica paso a paso, con
scripts que preparan el entorno en Windows y en Linux.

## Documentación

- [Funcionalidades](documentacion/funcionalidades.md)
- [Manual de uso](documentacion/manual-de-uso.md)
- [Arquitectura](documentacion/arquitectura.md)
- [Defectos conocidos](documentacion/defectos-conocidos.md)
- [Historial de cambios](documentacion/historial-de-cambios.md)
- [Infraestructura](documentacion/infraestructura/vision-general.md)

## Estado del proyecto

Versión **0.1.0**, la primera publicada. Probada en Windows 11 y en Linux (Debian 13). En Tails
todavía no se ha probado.

Hay un detalle temporal: Arti 0.47 se bloquea al arrancar en Windows por un fallo en una de sus
dependencias, ya reportado a sus desarrolladores. Mientras lo corrigen, WoDW incluye una copia
arreglada de esa pieza en `parches/saturating-time/`. Se quitará en cuanto salga la versión
corregida de Arti.

Los problemas se reportan y siguen como [issues](https://github.com/celtidcs/WoDW/issues) de
GitHub.

## Licencia

**GPL-3.0 o posterior.** El texto íntegro está en [`LICENSE`](LICENSE).

En corto: puedes usar el programa para lo que quieras, estudiar cómo funciona, modificarlo y
repartirlo. La única condición es que **si repartes una versión modificada, publiques también su
código**, con esta misma licencia.

Se eligió copyleft y no una licencia permisiva a propósito. Un navegador que promete proteger a
quien lo usa solo merece confianza si cualquiera puede leer su código y comprobar que hace lo que
dice; una versión cerrada no podría demostrarlo. Lo que se construya encima vuelve a todos.

La copia de `saturating-time` incluida en `parches/` conserva su licencia original (MIT o
Apache-2.0), compatible con la GPL.

Se entrega **sin ninguna garantía**, como dice la licencia.
