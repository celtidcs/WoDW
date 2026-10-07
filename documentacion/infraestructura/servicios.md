# Servicios y protocolos externos: WoDW

| Servicio | Uso | Protocolo |
|---|---|---|
| Red Tor (autoridades de directorio y relés) | Consenso y circuitos | Protocolo Tor, vía `arti-client` |
| Puentes Tor (opcional) | Entrar en Tor sin que el proveedor lo detecte | obfs4 / Snowflake mediante transportes enchufables |
| Servicios onion v3 | Destinos de navegación | HTTP/1.1 (y TLS si `https`) sobre flujos Tor |
| Motores de búsqueda configurados | Búsquedas | HTTP(S) sobre Tor; plantillas en `[motores]`, cada una con su fiabilidad |
| Accesos directos configurados | Sitios que se abren desde el botón «Accesos» (dark.fail, tor.taxi y Tor Project de serie) | HTTP(S) sobre Tor; direcciones en `[accesos]`, cada una con su fiabilidad |
| API de publicaciones de GitHub (`api.github.com/repos/celtidcs/WoDW/releases/latest`) | Aviso de versión nueva al conectar (desactivable en `[actualizaciones]`) | HTTPS sobre Tor, con un aislamiento propio; solo se lee el número de versión, nunca se descarga nada |

WoDW no abre puertos locales ni expone servicios. No usa MCP ni otros conectores.
