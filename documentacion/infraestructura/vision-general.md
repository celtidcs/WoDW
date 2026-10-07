# Visión general de la infraestructura: WoDW

## Componentes
- **Locales**: un único ejecutable (`wodw` / `wodw.exe`) que contiene la interfaz, el cliente Tor
  (Arti) y el modo Worker. Opcionalmente, `wodw.toml` junto al ejecutable y, si se usan puentes,
  los binarios de transporte (`lyrebird`, `snowflake-client`).
- **Datos locales de Arti**: estado y caché de directorio en los directorios predeterminados de
  Arti o en `[tor] ruta_estado` / `ruta_cache`.
- **Externos**: la red Tor (relés públicos o puentes) y, para el aviso de versión nueva, la API
  pública de GitHub consultada por Tor. No hay servidores propios ni cuentas.
- **Dispositivos**: la salida de sonido predeterminada (WASAPI en Windows, ALSA en Linux) para el
  audio y el vídeo. Sin ella, la reproducción sigue sin sonido.
- **Rastros en el sistema**: en Windows, un perfil de AppContainer vacío (`WoDW.Worker`) en el
  registro del usuario mientras la aplicación está abierta, y un permiso de lectura y ejecución
  para ese AppContainer en el propio ejecutable. El registro de la sesión solo se escribe en disco
  si el usuario lo pide o lo configura en Automático.

## Variables y autenticación
No hay secretos ni variables de entorno obligatorias. `RUST_LOG` controla el nivel de registro
(`tracing-subscriber`).

## Compilación
`cargo build --release` en cada plataforma. Scripts idempotentes de preparación:
`preparar-entorno.ps1` (Windows) y `preparar-entorno.sh` (Linux). Pasos completos en
[guia-de-clonado.md](guia-de-clonado.md).
