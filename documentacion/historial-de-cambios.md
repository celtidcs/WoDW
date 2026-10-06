# Historial de cambios: WoDW (Waves on Dark Web)

## Versión 0.1.0

Primera versión publicada.

### Navegación
- Navegador de solo lectura sobre Tor con Tor incluido (`arti-client` 0.47): sin JavaScript, sin
  WebView y sin instalar nada más.
- Pestañas con circuitos de Tor separados, Vanguards-Lite, puentes obfs4/Snowflake y HTTPS sobre Tor.
- Búsqueda con Ahmia, Torch y DuckDuckGo Onion; se pueden añadir otros motores.

### Seguridad
- Cada página, imagen o audio se procesa en un proceso aparte, encerrado y de usar y tirar.
  Windows: mitigaciones de proceso, integridad baja y Job Object. Linux: `prctl`, Landlock y
  seccomp, incluido el bloqueo de creación de procesos.
- Cliente HTTP desconfiado: plazos, límites y rechazo de respuestas ambiguas o inyectadas.
- Imágenes reconstruidas a píxeles sin metadatos y con límites contra bombas de descompresión;
  audio con filtro contra ultrasonidos.
- Defensa automática: bloqueo de sitios hostiles, cambio de circuitos, borrado de pestañas, pánico
  automático ante compromiso, borrado por inactividad y al cerrar.
- Botón del pánico con viñeta explicativa y atajo `Esc` ×3.
- Señuelos en disco (opcionales) y en memoria.

### Interfaz y configuración
- Interfaz nativa con `egui`, letterboxing e icono propio.
- Toda la configuración en `wodw.toml`, con `wodw.ejemplo.toml` comentado.

### Conocido
- En Windows se usa temporalmente una copia corregida de la dependencia `saturating-time` hasta
  que Arti publique la corrección de su arranque en Windows. Detalles en
  [arquitectura.md](arquitectura.md) y [defectos-conocidos.md](defectos-conocidos.md).
