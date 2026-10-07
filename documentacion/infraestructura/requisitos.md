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
   Incluye el compilador de recursos que usa `build.rs` para meter el icono en el `.exe` y el
   compilador de C con el que se construye el decodificador H.264 (OpenH264, desde su fuente).
3. **Linux**: `build-essential` (también compila OpenH264), `pkg-config`, `libssl-dev` (TLS),
   `libasound2-dev` (sonido ALSA, para `cpal`) y las bibliotecas de ventana que
   pide egui (`libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev`, según
   https://github.com/emilk/egui). La compilación y las pruebas en Linux se han hecho sin abrir
   ventana; abrir la interfaz gráfica en Linux todavía no se ha comprobado.

No hace falta SQLite en el sistema: se compila dentro del ejecutable (`rusqlite` con `bundled`).
Las fuentes de los alfabetos no latinos también van dentro del ejecutable (`recursos/fuentes/`).

## Para ejecutar

- **Windows**: nada más. Si no hay dispositivo de sonido, el audio y el vídeo se reproducen sin
  sonido (el reloj avanza igual).
- **Linux**: `libasound2` (ALSA; en escritorios con PulseAudio o PipeWire funciona a través de su
  capa ALSA), OpenSSL 3 y las bibliotecas de ventana.
