# Servicios y protocolos externos: WoDW

| Servicio | Uso | Protocolo |
|---|---|---|
| Red Tor (autoridades de directorio y relés) | Consenso y circuitos | Protocolo Tor, vía `arti-client` |
| Puentes Tor (opcional) | Entrar en Tor sin que el proveedor lo detecte | obfs4 / Snowflake mediante transportes enchufables |
| Servicios onion v3 | Destinos de navegación | HTTP/1.1 (y TLS si `https`) sobre flujos Tor |
| Motores de búsqueda configurados | Búsquedas | HTTP(S) sobre Tor; plantillas en `[motores]` |

WoDW no abre puertos locales ni expone servicios. No usa MCP ni otros conectores.
