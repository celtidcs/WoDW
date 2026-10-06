# Defectos conocidos: WoDW (Waves on Dark Web)

Aquí está todo lo que sé que falla o falta. Cada ficha explica qué pasa, qué se ha comprobado, qué
queda por confirmar y cuál sería el siguiente paso. Si encuentras algo que no esté aquí, abre una
[incidencia](https://github.com/celtidcs/WoDW/issues).

| ID | Componente | Gravedad | Estado |
|---|---|---|---|
| D-1 | Worker en Windows | Alta | Abierto |
| D-2 | Worker en Windows | Media | Abierto |
| D-3 | Worker en Linux / Tails | Alta | Corregido en Linux; Tails sin probar |
| D-4 | Dependencias | Media | Aceptado con excepción |
| D-5 | Arranque de Tor en Windows | Alta | Resuelto con parche temporal |

## D-1 — El Worker conserva acceso a la red en Windows

1. **Qué pasa**: una vez encerrado, el Worker de Windows ya no puede escribir en el perfil del
   usuario ni lanzar procesos, pero todavía puede abrir conexiones de red: la integridad baja no lo
   impide.
2. **Comprobado**: las políticas de mitigación de procesos de Windows no incluyen ninguna de red.
3. **Por confirmar**: AppContainer lo resolvería, pero exige dar permiso de lectura y ejecución al
   grupo «ALL APPLICATION PACKAGES» sobre el propio ejecutable.
4. **Siguiente paso**: lanzar el Worker dentro de un AppContainer sin capacidades. No se ha hecho
   porque implica cambiar los permisos de un archivo del usuario en cada arranque.

## D-2 — `DisallowWin32kSystemCalls` no se puede aplicar

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
