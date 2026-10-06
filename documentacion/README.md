# Documentación de WoDW (Waves on Dark Web)

WoDW es un navegador de escritorio de **solo lectura** sobre Tor, escrito en Rust, para Windows y
Linux/Tails. Lleva Tor dentro (`arti-client`), procesa cada recurso descargado en un proceso aparte,
encerrado y de usar y tirar, y reacciona sola ante los ataques.

La presentación general está en el [README del repositorio](../README.md). Aquí está el detalle:

- [Funcionalidades](funcionalidades.md): todo lo que hace hoy.
- [Manual de uso](manual-de-uso.md): cómo se usa y cómo se configura.
- [Arquitectura](arquitectura.md): cómo está construida por dentro y por qué, con sus limitaciones.
- [Defectos conocidos](defectos-conocidos.md): lo que falla o falta, sin esconderlo.
- [Historial de cambios](historial-de-cambios.md).
- Infraestructura: [visión general](infraestructura/vision-general.md),
  [requisitos](infraestructura/requisitos.md), [servicios externos](infraestructura/servicios.md) y
  [guía de clonado](infraestructura/guia-de-clonado.md).
- Configuración: `wodw.ejemplo.toml`, en la raíz del repositorio, explica cada ajuste.

**Compilar y probar:**

```bash
cargo build --release
cargo test --all-features
```
