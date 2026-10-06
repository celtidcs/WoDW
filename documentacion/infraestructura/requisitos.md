# Requisitos del sistema y herramientas: WoDW

## Plataformas

- **Windows** 10 y 11 de 64 bits (`x86_64-pc-windows-msvc`). Probado en Windows 11 Pro.
- **Linux** de 64 bits (`x86_64-unknown-linux-gnu`): Debian, Ubuntu, Tails OS. Las pruebas se han
  ejecutado en Debian 13 con un núcleo Linux 6.6. El encierro del Worker necesita un núcleo con
  Landlock activo; sin él, el Worker se niega a procesar contenido. Tails todavía no se ha probado.

## Para compilar

1. **Rust** estable con `rustfmt` y `clippy` (probado con Rust 1.96 en Windows y 1.98 en Linux).
   `cargo-audit` para revisar vulnerabilidades de las dependencias.
2. **Windows**: Visual Studio Build Tools con «Desarrollo para el escritorio con C++» (MSVC x64).
   Incluye el compilador de recursos que usa `build.rs` para meter el icono en el `.exe`.
3. **Linux**: `build-essential`, `pkg-config`, `libssl-dev` (TLS) y las bibliotecas de ventana que
   pide egui (`libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev`, según
   https://github.com/emilk/egui). La compilación y las pruebas en Linux se han hecho sin abrir
   ventana; abrir la interfaz gráfica en Linux todavía no se ha comprobado.

No hace falta SQLite en el sistema: se compila dentro del ejecutable (`rusqlite` con `bundled`).
