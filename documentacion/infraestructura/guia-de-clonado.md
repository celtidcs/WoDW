# Guía de clonado y arranque: WoDW

Pensada para alguien que no conoce el proyecto.

1. **Instalar Rust**: https://rustup.rs (canal estable).
2. **Herramientas del sistema**: ver [requisitos.md](requisitos.md). En Linux hacen falta, entre
   otras, `build-essential` (compila el decodificador H.264) y `libasound2-dev` (sonido).
3. **Obtener el código**: `git clone https://github.com/celtidcs/WoDW.git` y entrar en la carpeta.
4. **Preparar el entorno** (instala componentes, compila y ejecuta las pruebas):
   - Windows (PowerShell): `powershell -ExecutionPolicy Bypass -File documentacion/infraestructura/preparar-entorno.ps1`
   - Linux: `bash documentacion/infraestructura/preparar-entorno.sh`
5. **Compilar la versión final**: `cargo build --release`. Salen dos ejecutables en
   `target/release/`: `wodw(.exe)` y `wodw-worker(.exe)`. Tienen que ir juntos en la misma carpeta:
   el primero lanza al segundo para abrir cada página.
6. **Configurar (opcional)**: copiar `wodw.ejemplo.toml` como `wodw.toml` junto al ejecutable.
7. **Arrancar**: ejecutar el binario. La barra inferior indica cuándo Tor está conectado.
8. **Comprobaciones de calidad** antes de proponer cambios (todas deben terminar sin errores ni avisos):
   ```bash
   cargo test --all-features
   cargo clippy --all-targets --all-features -- -D warnings
   cargo fmt --check
   cargo doc --all-features --no-deps
   cargo audit
   ```
