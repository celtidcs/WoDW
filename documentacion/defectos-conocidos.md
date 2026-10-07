# Defectos conocidos: WoDW (Waves on Dark Web)

Aquí está todo lo que sé que falla o falta. Cada ficha explica qué pasa, qué se ha comprobado, qué
queda por confirmar y cuál sería el siguiente paso. Si encuentras algo que no esté aquí, abre una
[incidencia](https://github.com/celtidcs/WoDW/issues).

| ID | Componente | Gravedad | Estado |
|---|---|---|---|
| D-1 | Worker en Windows | Alta | **Corregido en 0.2.0** |
| D-2 | Worker en Windows | Media | **Corregido en 0.2.1** |
| D-3 | Worker en Linux / Tails | Alta | Corregido en Linux; Tails sin probar |
| D-4 | Dependencias | Media | Aceptado con excepción |
| D-5 | Arranque de Tor en Windows | Alta | Resuelto con parche temporal |
| D-6 | Botón del pánico (Windows) | Crítica | **Corregido en 0.2.0** |
| D-7 | Texto en árabe, persa y jemer | Baja | Abierto |
| D-8 | Buscador Torch | Media | **Corregido en 0.2.1** |
| D-9 | Consola en Windows | Media | **Corregido en 0.2.1** |
| D-10 | Proceso aislado en Windows | Alta | **Corregido en 0.2.1** |

## D-1 — El Worker conserva acceso a la red en Windows

> **Corregido en la 0.2.0**: el Worker se ejecuta en un AppContainer sin permisos y no puede abrir
> conexiones de red (comprobado contra internet real). Lo que sigue describe el problema original.

1. **Qué pasa**: una vez encerrado, el Worker de Windows ya no puede escribir en el perfil del
   usuario ni lanzar procesos, pero todavía puede abrir conexiones de red: la integridad baja no lo
   impide.
2. **Comprobado**: las políticas de mitigación de procesos de Windows no incluyen ninguna de red.
3. **Por confirmar**: AppContainer lo resolvería, pero exige dar permiso de lectura y ejecución al
   grupo «ALL APPLICATION PACKAGES» sobre el propio ejecutable.
4. **Siguiente paso**: lanzar el Worker dentro de un AppContainer sin capacidades. No se ha hecho
   porque implica cambiar los permisos de un archivo del usuario en cada arranque.

## D-2 — `DisallowWin32kSystemCalls` no se puede aplicar

> **Corregido en la 0.2.1**: el proceso aislado es ahora un ejecutable propio, `wodw-worker`, que no
> carga nada gráfico y se encierra sin acceso al núcleo gráfico de Windows. Lo que sigue describe el
> problema original.

1. **Qué pasa**: Windows rechaza esta mitigación (error 19) en el ejecutable real.
2. **Comprobado**: no es un problema de parámetros; la misma llamada funciona en un binario que no
   carga `user32.dll`.
3. **Por confirmar**: que la causa sea que el ejecutable único carga la parte gráfica de Windows.
4. **Siguiente paso**: un ejecutable de Worker separado y sin interfaz. Cambia el empaquetado (dos
   ejecutables) y está pendiente de decidir.

## D-3 — Encierro del Worker en Linux

1. **Qué pasaba**: al probarlo en Linux aparecieron dos fallos. El Worker no llegaba a arrancar,
   porque el filtro seccomp prohibía un `socketpair` local que Tokio necesita, y el encierro no
   impedía crear procesos con `fork`. Una copia del Worker habría podido sobrevivir al borrado de
   emergencia.
2. **Corregido**: se permite solo `socketpair(AF_UNIX)` y se bloquea la creación de procesos sin
   bloquear los hilos. Hay pruebas que fallan si cualquiera de las dos cosas se deshace. Landlock
   funciona en el núcleo probado (Linux 6.6).
3. **Por confirmar**: en núcleos sin Landlock el Worker se niega a procesar, por diseño; podría
   pasar en Tails si su núcleo no lo activa.
4. **Siguiente paso**: probarlo en Tails real.

## D-4 — Avisos de seguridad en dependencias de Arti

1. **Qué pasa**: `cargo audit` señala RUSTSEC-2023-0071 en la biblioteca `rsa`, que llega a través
   de `arti-client`, y avisa de tres bibliotecas indirectas sin mantenimiento.
2. **Comprobado**: no hay una versión corregida de `rsa`; la excepción está justificada en
   `.cargo/audit.toml`.
3. **Siguiente paso**: actualizar en cuanto Arti cambie esas dependencias.

## D-5 — Tor se quedaba parado en el 15 % en Windows

1. **Qué pasaba**: en Windows, Tor no pasaba del 15 % («descargando el consenso»). En Linux, en el
   mismo equipo y la misma red, arrancaba en 24 segundos.
2. **Comprobado**: no era la red, ni el antivirus, ni el cortafuegos (con Defender desactivado
   pasaba igual, y el Tor clásico oficial arrancaba en 8 segundos en el mismo Windows). La causa es
   un bucle infinito en `saturating-time`, una dependencia de Arti: el reloj de Windows tiene una
   resolución de 100 ns y un cálculo de límites nunca termina. Está reportado en Arti
   ([#2678](https://gitlab.torproject.org/tpo/core/arti/-/issues/2678),
   [#2726](https://gitlab.torproject.org/tpo/core/arti/-/issues/2726)).
3. **Solución temporal**: WoDW compila con una copia corregida de esa dependencia
   (`parches/saturating-time/`). Con ella Tor arranca en Windows en unos 4 segundos.
4. **Siguiente paso**: cuando Arti publique la corrección, actualizar Arti y quitar la copia.

## D-6 — El pánico dejaba un volcado de memoria en el disco (0.1.0)

En la versión 0.1.0, el botón del pánico cerraba WoDW de una manera que Windows interpretaba como un
fallo. El sistema tardaba unos tres segundos y medio en cerrar la aplicación y, mientras tanto,
guardaba en `%LOCALAPPDATA%\CrashDumps` una copia de toda su memoria, de unos 40 MB, con lo que
hubiera en pantalla. También dejaba un informe en
`%ProgramData%\Microsoft\Windows\WER\ReportArchive`.

La versión 0.2.0 lo corrige. El cierre es inmediato, dura unas décimas de segundo y no deja ni
volcados ni informes, tampoco si la aplicación falla o se queda sin memoria. Una prueba automática
lo comprueba en cada compilación.

Si usaste el pánico con la 0.1.0, borra los archivos `wodw.exe.*.dmp` de `%LOCALAPPDATA%\CrashDumps`
y las carpetas `AppCrash_wodw.exe_*` de la carpeta de informes. En el visor de eventos de Windows
quedarán además algunas líneas que dicen que `wodw.exe` se cerró por un error. No contienen nada de
tu sesión, pero sí la hora a la que ocurrió. Para borrarlas hacen falta permisos de administrador.

## D-7 — El árabe, el persa y el jemer se ven con las letras sueltas

En estos alfabetos las letras se unen entre sí y cambian de forma según su posición, y el jemer
además reordena algunos signos. WoDW dibuja cada letra, pero no hace esas uniones, así que el texto
se puede leer con esfuerzo, aunque no se ve como en un navegador corriente.

No es un problema de fuentes: están todas incluidas. La biblioteca con la que se dibuja la interfaz
(`egui`) todavía no sabe componer este tipo de escritura. Arreglarlo exige añadir un motor de
composición de texto, un cambio grande que queda pendiente.

## D-8 — La dirección de Torch estaba mal escrita (0.1.0 y 0.2.0)

La dirección de Torch que traían estas versiones tenía el aspecto de una dirección `.onion` correcta,
pero no superaba la suma de control que llevan dentro todas las direcciones modernas, así que ningún
servicio podía tenerla y la búsqueda con Torch nunca funcionó. WoDW solo comprobaba la longitud y las
letras de la dirección.

La versión 0.2.1 comprueba la suma de control de todas las direcciones `.onion` de buscadores y
accesos directos, y rechaza al arrancar las que no la superan. Torch ya no viene de serie: su
dirección actual no la confirma ninguna fuente oficial y la red Tor no tiene publicado su servicio.

## D-9 — En Windows se abría una terminal junto a la ventana (0.1.0 y 0.2.0)

Junto a la ventana de WoDW aparecía una terminal con mensajes técnicos, entre ellos el nombre del
sitio en cada incidente de seguridad. Si se cerraba, WoDW terminaba de golpe, sin guardar el registro
automático ni borrar el perfil de aislamiento. En la 0.2.1 la terminal ya no aparece, y los errores
que impiden arrancar se muestran en una ventana de aviso.

## D-10 — El proceso aislado dependía del escritorio de Windows (0.2.0)

En la 0.2.0, el proceso aislado que abre cada página era el mismo ejecutable que la interfaz y
cargaba las bibliotecas gráficas de Windows. En el escritorio normal del usuario funcionaba, pero
tenía acceso a ese escritorio, cosa que un proceso aislado no debería tener; y en cualquier otro
escritorio, como los de algunos entornos remotos o automatizados, moría al arrancar (error
`0xc0000142`) y WoDW no podía abrir ninguna página.

Desde la 0.2.1, el proceso aislado es un ejecutable propio, `wodw-worker`, sin nada de interfaz:
funciona en cualquier escritorio, no tiene acceso al del usuario y está cortado del núcleo gráfico
de Windows. Por eso WoDW son ahora dos ejecutables que tienen que ir juntos en la misma carpeta.

