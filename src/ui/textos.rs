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
];
/// Viñeta del botón del pánico: avisa de lo que hace antes de pulsarlo.
pub fn ayuda_panico(pulsaciones_esc: u8) -> String {
    format!(
        "Borra al instante todo lo de esta sesión (pestañas, historial y contenido)          y cierra WoDW. No pide confirmación.
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

/// Texto del estado de Tor.
pub fn estado_tor(estado: &crate::maestro::sesion::EstadoTor) -> String {
    use crate::maestro::sesion::EstadoTor;
    match estado {
        EstadoTor::Conectando(fraccion) => format!("Conectando a Tor… {:.0} %", fraccion * 100.0),
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

/// Descripción de un audio.
pub fn audio(frecuencia: u32, canales: u16, muestras: usize, corte: u32) -> String {
    format!(
        "Audio WAV {frecuencia} Hz, {canales} canal(es), {muestras} muestras por canal; filtro paso bajo a {corte} Hz aplicado. (Sin reproducción.)"
    )
}

/// Color de fondo del área de contenido.
pub const COLOR_FONDO_CONTENIDO: Color32 = Color32::from_rgb(26, 26, 26);
/// Color del botón del pánico.
pub const COLOR_PANICO: Color32 = Color32::from_rgb(192, 57, 43);

/// Color por severidad.
pub fn color_severidad(nivel: NivelSeveridad) -> Color32 {
    match nivel {
        NivelSeveridad::Informativo => Color32::from_rgb(46, 204, 113),
        NivelSeveridad::Medio => Color32::from_rgb(241, 196, 15),
        NivelSeveridad::Alto => Color32::from_rgb(230, 126, 34),
        NivelSeveridad::Critico => Color32::from_rgb(231, 76, 60),
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
