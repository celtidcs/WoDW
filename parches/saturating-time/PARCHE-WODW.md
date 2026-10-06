# Copia parcheada de `saturating-time` 0.5.0 — TEMPORAL

Origen: crate `saturating-time` 0.5.0 de crates.io (licencia MIT OR Apache-2.0, proyecto Arti,
https://gitlab.torproject.org/tpo/core/arti). Sustituye a la oficial mediante `[patch.crates-io]` en
el `Cargo.toml` raíz de WoDW.

## Por qué

En Windows el reloj del sistema tiene una resolución de 100 ns. `find_limit` reduce el paso a la
mitad hasta 1 ns; los pasos menores de 100 ns no cambian la hora, la operación devuelve `Some` con el
mismo valor y el bucle no termina nunca. Arti lo usa al leer el consenso, así que Tor se queda
parado en el 15 % al arrancar. Avisos:
https://gitlab.torproject.org/tpo/core/arti/-/issues/2678 y
https://gitlab.torproject.org/tpo/core/arti/-/issues/2726.

## Cambios (los únicos respecto a 0.5.0)

1. `src/internal.rs`: `find_limit` termina si un paso no hace avanzar la hora
   (`Some(st) if st == res => return res`); el rasgo interno exige `PartialEq` para compararlo.
2. `src/lib.rs`: `saturating_add`/`saturating_sub` usan `unwrap_or_else`, de modo que el límite solo
   se calcula cuando la operación desborda de verdad.

## Cuándo se retira

En cuanto Arti publique una versión con la corrección: se actualiza Arti, se borra esta carpeta y la
sección `[patch.crates-io]` del `Cargo.toml` raíz, y se publica una versión nueva de WoDW.
La barrera `tests/parche_saturating_time.rs` debe seguir en verde sin el parche.
