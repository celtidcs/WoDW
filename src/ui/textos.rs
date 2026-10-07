//! Cadenas visibles por el usuario y colores, reunidos en un único sitio.

use crate::ids::eventos::{NivelSeveridad, VectorAmenaza};
use egui::Color32;

/// Nombre del producto.
pub const NOMBRE_PRODUCTO: &str = "WoDW — Waves on Dark Web";
/// Versión tomada de `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Título de la ventana.
pub fn titulo_ventana() -> String {
    format!("{NOMBRE_PRODUCTO} {VERSION} (modo Safest)")
}

/// Pie de la barra de estado.
pub fn pie_estado() -> String {
    format!("WoDW {VERSION} (modo Safest)")
}

/// Texto de pista de la barra de direcciones.
pub const PISTA_BARRA: &str = "Dirección .onion o términos de búsqueda…";
/// Botón para volver a la página anterior.
pub const BOTON_ATRAS: &str = "⏴";
/// Botón para avanzar a la página siguiente.
pub const BOTON_ADELANTE: &str = "⏵";
/// Botón para abrir una pestaña.
pub const BOTON_NUEVA_PESTANA: &str = "+";
/// Botón para cerrar una pestaña.
pub const BOTON_CERRAR_PESTANA: &str = "×";
/// Botón de navegación.
pub const BOTON_IR: &str = "Ir / Buscar";
/// Botón del pánico.
pub const BOTON_PANICO: &str = "⚠ PÁNICO";
/// Todos los textos de botón: la interfaz debe poder dibujar cada uno de sus caracteres.
pub const TEXTOS_DE_BOTONES: &[&str] = &[
    BOTON_ATRAS,
    BOTON_ADELANTE,
    BOTON_NUEVA_PESTANA,
    BOTON_CERRAR_PESTANA,
    BOTON_IR,
    BOTON_PANICO,
    BOTON_ROTAR,
    BOTON_REPRODUCIR,
    BOTON_PAUSA,
    BOTON_REANUDAR,
    BOTON_PARAR,
    BOTON_REGISTRO,
    BOTON_GUARDAR_REGISTRO,
    BOTON_BORRAR_REGISTRO,
    BOTON_CONFIRMAR,
    BOTON_CANCELAR,
];
/// Botón para empezar a reproducir un audio o vídeo.
pub const BOTON_REPRODUCIR: &str = "⏵ Reproducir";
/// Botón de pausa.
pub const BOTON_PAUSA: &str = "⏸ Pausa";
/// Botón para reanudar tras una pausa.
pub const BOTON_REANUDAR: &str = "⏵ Reanudar";
/// Botón para detener la reproducción.
pub const BOTON_PARAR: &str = "⏹ Parar";
/// Etiqueta del deslizador de volumen.
pub const ETIQUETA_VOLUMEN: &str = "Volumen";

/// Línea de estado de la reproducción.
pub fn estado_reproduccion(
    fase: &crate::maestro::medios::FaseReproduccion,
    reloj_ms: u64,
    con_sonido: bool,
) -> String {
    use crate::maestro::medios::FaseReproduccion;
    /// Milisegundos por segundo y segundos por minuto.
    const MS_POR_S: u64 = 1000;
    const S_POR_MIN: u64 = 60;
    let segundos = reloj_ms / MS_POR_S;
    let tiempo = format!("{}:{:02}", segundos / S_POR_MIN, segundos % S_POR_MIN);
    let sonido = if con_sonido {
        ""
    } else {
        " (sin dispositivo de sonido)"
    };
    match fase {
        FaseReproduccion::Abriendo => "Abriendo en un proceso aislado sin red…".to_string(),
        FaseReproduccion::Reproduciendo => format!("{tiempo}{sonido}"),
        FaseReproduccion::Terminada => format!("Terminado ({tiempo})."),
        FaseReproduccion::Error(e) => format!("No se pudo reproducir: {e}"),
    }
}
/// Viñeta del botón del pánico: avisa de lo que hace antes de pulsarlo.
pub fn ayuda_panico(pulsaciones_esc: u8) -> String {
    format!(
        "Borra al instante todo lo de esta sesión (pestañas, historial, contenido y \
         registro) y cierra WoDW sin dejar nada en el disco. No pide confirmación.\n\
         Atajo: pulsar Esc {pulsaciones_esc} veces seguidas."
    )
}
/// Botón de rotación manual.
pub const BOTON_ROTAR: &str = "🔄 Nuevo circuito para todas las pestañas";
/// Estado sin sesión de red (pruebas o fallo de arranque).
pub const SIN_SESION: &str = "Sin conexión: la sesión de red no está activa.";
/// Estado tras rotar.
pub const AISLAMIENTO_ROTADO: &str =
    "Circuitos renovados: las próximas peticiones usan circuitos nuevos.";
/// Estado tras purga por inactividad.
pub const PURGA_INACTIVIDAD: &str =
    "Purga automática por inactividad: pestañas e historial borrados.";
/// Aviso de cargando.
pub const CARGANDO: &str = "Descargando por Tor y procesando en un Worker aislado…";
/// Bienvenida.
pub const BIENVENIDA: &str = "Navegador de solo lectura sobre Tor: sin JavaScript, sin WebView. El contenido se procesa en un proceso aislado y desechable.";
/// Cabecera de la lista de motores.
pub const MOTORES_DISPONIBLES: &str = "Motores de búsqueda configurados:";
/// Cabecera de enlaces.
pub const ENLACES: &str = "Enlaces";
/// Cabecera de los medios incrustados en una página.
pub const MEDIOS_DE_LA_PAGINA: &str =
    "Imágenes, audios y vídeos de la página (no se descargan hasta que los pulsas)";

/// Texto de un medio incrustado: su tipo y su texto alternativo o su URL.
pub fn medio_enlazado(medio: &crate::ipc::mensajes::MedioEnlazado) -> String {
    use crate::ipc::mensajes::TipoMedioEnlazado;
    let tipo = match medio.tipo {
        TipoMedioEnlazado::Imagen => "Imagen",
        TipoMedioEnlazado::Audio => "Audio",
        TipoMedioEnlazado::Video => "Vídeo",
    };
    let nombre = if medio.texto.is_empty() {
        &medio.url
    } else {
        &medio.texto
    };
    format!("[{tipo}] {nombre}")
}

/// Aviso de texto recortado por el tope de caracteres.
pub const TEXTO_RECORTADO: &str =
    "(Texto recortado: la página supera el tope de caracteres configurado.)";

/// Aviso junto a un enlace sospechoso.
pub fn aviso_enlace(aviso: &crate::seguridad::enlaces::AvisoEnlace) -> String {
    use crate::seguridad::enlaces::AvisoEnlace;
    match aviso {
        AvisoEnlace::DestinoDistinto { host_real } => {
            format!("⚠ Engañoso: en realidad lleva a {host_real}")
        }
        AvisoEnlace::Punycode { host_real } => {
            format!("⚠ Nombre internacionalizado que puede imitar a otro: {host_real}")
        }
    }
}

/// Texto del estado de Tor.
pub fn estado_tor(estado: &crate::maestro::sesion::EstadoTor) -> String {
    use crate::maestro::sesion::EstadoTor;
    match estado {
        EstadoTor::Conectando(fraccion) => format!(
            "Conectando a Tor… {:.0} %",
            fraccion * crate::unidades::POR_CIENTO as f32
        ),
        EstadoTor::Listo => "Conectado a Tor.".to_string(),
        EstadoTor::Error(e) => format!("Tor no disponible: {e}"),
    }
}

/// Descripción de contenido no soportado.
pub fn no_soportado(tipo: &str) -> String {
    format!("Tipo de contenido «{tipo}» no soportado: no se procesa ni se guarda.")
}

/// Descripción de una imagen.
pub fn imagen(ancho: u32, alto: u32) -> String {
    format!("Imagen {ancho} × {alto} px, reconstruida desde píxeles (sin metadatos).")
}

/// Descripción de un audio o vídeo descargado y aún sin abrir.
pub fn medio(familia: crate::ipc::mensajes::FamiliaMedio, tipo: &str, bytes: usize) -> String {
    use crate::ipc::mensajes::FamiliaMedio;
    let nombre = match familia {
        FamiliaMedio::Audio => "Audio",
        FamiliaMedio::Video => "Vídeo",
    };
    format!(
        "{nombre} «{tipo}», {} KiB. Se abre en un proceso aislado sin red al reproducirlo.",
        bytes / crate::unidades::BYTES_POR_KIB
    )
}

/// Color de fondo del área de contenido.
pub const COLOR_FONDO_CONTENIDO: Color32 = Color32::from_rgb(26, 26, 26);
/// Color del nivel «Informativo» del IDS (verde).
pub const COLOR_INFORMATIVO: Color32 = Color32::from_rgb(46, 204, 113);
/// Color del nivel «Medio» del IDS (amarillo).
pub const COLOR_MEDIO: Color32 = Color32::from_rgb(241, 196, 15);
/// Color del nivel «Alto» del IDS (ámbar).
pub const COLOR_ALTO: Color32 = Color32::from_rgb(230, 126, 34);
/// Color del nivel «Crítico» del IDS (rojo).
pub const COLOR_CRITICO: Color32 = Color32::from_rgb(231, 76, 60);
/// Color de los avisos de enlace engañoso (el ámbar del nivel «Alto»).
pub const COLOR_AVISO_ENLACE: Color32 = COLOR_ALTO;
/// Color del aviso de versión nueva (el verde del nivel informativo).
pub const COLOR_AVISO_VERSION: Color32 = COLOR_INFORMATIVO;

/// Aviso de versión nueva.
pub fn aviso_version(version: &str) -> String {
    format!(
        "Hay una versión nueva de WoDW ({version}). Descárgala tú desde su página (WoDW no descarga nada solo):"
    )
}

/// Botón que abre el panel del registro de la sesión.
pub const BOTON_REGISTRO: &str = "Registro";
/// Título del panel del registro.
pub const TITULO_REGISTRO: &str = "Registro de la sesión";
/// Cabeceras de las dos elecciones.
pub const REGISTRO_QUE: &str = "Qué se registra";
/// Cabecera del guardado.
pub const REGISTRO_COMO: &str = "Cómo se guarda";
/// Nombres de las opciones.
pub const OPCION_SEGURIDAD: &str = "Seguridad";
/// Opción completa.
pub const OPCION_COMPLETO: &str = "Completo";
/// Opción manual.
pub const OPCION_MANUAL: &str = "Manual";
/// Opción automática.
pub const OPCION_AUTOMATICO: &str = "Automático";
/// Botones del panel.
pub const BOTON_GUARDAR_REGISTRO: &str = "Guardar registro ahora";
/// Borrar el registro de memoria.
pub const BOTON_BORRAR_REGISTRO: &str = "Borrar registro";
/// Confirmar una opción más reveladora.
pub const BOTON_CONFIRMAR: &str = "Sí, lo entiendo y quiero activarlo";
/// Cancelar.
pub const BOTON_CANCELAR: &str = "Cancelar";
/// Etiqueta de la ruta de guardado.
pub const ETIQUETA_RUTA_REGISTRO: &str = "Archivo:";
/// Registro vacío.
pub const REGISTRO_VACIO: &str = "(Sin sucesos todavía.)";

/// Explicación de cada contenido del registro: utilidad, qué guarda y consecuencias.
pub fn explicacion_contenido_registro(c: crate::configuracion::ContenidoRegistro) -> &'static str {
    use crate::configuracion::ContenidoRegistro;
    match c {
        ContenidoRegistro::Seguridad => {
            "Para qué sirve: revisar qué amenazas detectó y frenó WoDW. Qué apunta: el estado de \
             Tor, los ataques detectados, los sitios bloqueados (solo su nombre), los procesos \
             aislados que fallaron, los medios hostiles, el pánico y las purgas. NO apunta las \
             páginas que visitas. Consecuencias: si alguien lee el registro, sabe que usaste WoDW y \
             qué ataques recibiste, pero no qué visitaste. Es la opción recomendada."
        }
        ContenidoRegistro::Completo => {
            "Para qué sirve: una auditoría detallada de toda la sesión. Qué apunta: todo lo de \
             Seguridad y, además, las direcciones de cada página y archivo que abres y cada \
             reproducción (nunca el contenido de las páginas ni lo que escribas). Consecuencias: \
             quien lea el registro sabrá exactamente qué sitios visitaste y cuándo. Si se guarda en \
             disco y alguien accede a tu equipo (robo, incautación, un programa espía), queda \
             expuesto todo lo que hiciste. Actívalo solo si necesitas esa auditoría."
        }
    }
}

/// Explicación de cada modo de guardado: utilidad, qué hace y consecuencias.
pub fn explicacion_guardado_registro(g: crate::configuracion::GuardadoRegistro) -> &'static str {
    use crate::configuracion::GuardadoRegistro;
    match g {
        GuardadoRegistro::Manual => {
            "Para qué sirve: decidir tú si el registro sale de la memoria. Qué hace: el registro \
             solo existe mientras WoDW está abierto; se escribe en el disco únicamente cuando \
             pulsas «Guardar registro ahora». Consecuencias: si no lo guardas, al cerrar WoDW no \
             queda rastro. Es la opción recomendada."
        }
        GuardadoRegistro::Automatico => {
            "Para qué sirve: no perder nunca el registro. Qué hace: al cerrar WoDW con normalidad \
             se escribe solo en el archivo indicado (el botón del pánico nunca guarda nada). \
             Consecuencias: siempre queda un archivo en el disco con lo registrado, que cualquiera \
             con acceso a tu equipo puede leer, y borrarlo de forma segura es responsabilidad tuya. \
             En Tails, sin almacenamiento persistente, el archivo desaparece al apagar; con \
             almacenamiento persistente, se conserva."
        }
    }
}

/// Aviso de confirmación al activar la opción más reveladora: el riesgo
/// principal, en una frase, distinto de la explicación general.
pub fn aviso_confirmacion_registro(contenido_completo: bool) -> &'static str {
    if contenido_completo {
        "Atención: con «Completo» el registro apuntará cada dirección que visites. Si ese registro \
         llega al disco y alguien accede a tu equipo, sabrá exactamente qué sitios visitaste."
    } else {
        "Atención: con «Automático» quedará siempre un archivo en el disco al cerrar WoDW con \
         normalidad, aunque no lo pidas. Borrarlo de forma segura será cosa tuya."
    }
}

/// Nota junto a «Borrar registro».
pub const NOTA_BORRAR_REGISTRO: &str =
    "«Borrar registro» vacía el de la memoria; los archivos que ya guardaste no se tocan.";

/// Resultado de guardar el registro.
pub fn registro_guardado(resultado: &std::io::Result<()>, ruta: &str) -> String {
    match resultado {
        Ok(()) => format!("Registro guardado en {ruta}."),
        Err(e) => format!("No se pudo guardar el registro en {ruta}: {e}"),
    }
}

/// Color del botón del pánico.
pub const COLOR_PANICO: Color32 = Color32::from_rgb(192, 57, 43);

/// Color por severidad.
pub fn color_severidad(nivel: NivelSeveridad) -> Color32 {
    match nivel {
        NivelSeveridad::Informativo => COLOR_INFORMATIVO,
        NivelSeveridad::Medio => COLOR_MEDIO,
        NivelSeveridad::Alto => COLOR_ALTO,
        NivelSeveridad::Critico => COLOR_CRITICO,
    }
}

/// Etiqueta por severidad.
pub fn etiqueta_severidad(nivel: NivelSeveridad) -> &'static str {
    match nivel {
        NivelSeveridad::Informativo => "INFORMACIÓN",
        NivelSeveridad::Medio => "AVISO",
        NivelSeveridad::Alto => "AMENAZA CONTENIDA",
        NivelSeveridad::Critico => "CRÍTICO",
    }
}

/// Indicador compacto del IDS en la cabecera.
pub fn indicador_ids(nivel: NivelSeveridad) -> &'static str {
    match nivel {
        NivelSeveridad::Informativo => "🛡 IDS: sin incidentes",
        NivelSeveridad::Medio => "⚠ IDS: aviso",
        NivelSeveridad::Alto => "🚨 IDS: ataque bloqueado",
        NivelSeveridad::Critico => "☠ IDS: compromiso",
    }
}

/// Descripción legible de un vector de amenaza.
pub fn describir_vector(vector: &VectorAmenaza) -> String {
    match vector {
        VectorAmenaza::RespuestaHostil { host, motivo } => {
            format!("Respuesta hostil de {host}: {motivo}. Sitio bloqueado y circuitos renovados.")
        }
        VectorAmenaza::ContenidoHostil { host, motivo } => {
            format!("Contenido hostil de {host}: {motivo}. Sitio bloqueado y pestaña purgada.")
        }
        VectorAmenaza::CaidaSandbox { host, detalle } => {
            format!("El Worker aislado cayó procesando {host} ({detalle}). Sitio bloqueado.")
        }
        VectorAmenaza::ViolacionLlamadaSistema { host } => {
            format!(
                "Contenido de {host} intentó una llamada al sistema prohibida. Pánico automático."
            )
        }
        VectorAmenaza::CanarioAlterado(detalle) => format!("Archivo señuelo alterado: {detalle}."),
        VectorAmenaza::MemoriaTrampaAlterada => "Trampa de memoria alterada.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colores_distintos_y_textos_con_contexto() {
        let niveles = [
            NivelSeveridad::Informativo,
            NivelSeveridad::Medio,
            NivelSeveridad::Alto,
            NivelSeveridad::Critico,
        ];
        for par in niveles.windows(2) {
            assert_ne!(color_severidad(par[0]), color_severidad(par[1]));
        }
        let v = VectorAmenaza::RespuestaHostil {
            host: "x.onion".into(),
            motivo: "m".into(),
        };
        assert!(describir_vector(&v).contains("x.onion"));
        assert!(titulo_ventana().contains(VERSION));
    }
}
