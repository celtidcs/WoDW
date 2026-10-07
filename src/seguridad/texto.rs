//! Higiene Unicode del texto que se muestra al usuario.
//!
//! La usan el Worker al sanitizar y el Maestro al revalidar lo que el Worker
//! devuelve (un Worker comprometido tampoco es de fiar).

use unicode_normalization::UnicodeNormalization;

/// Limpia `texto` para mostrarlo: elimina controles (salvo salto de línea y
/// tabulador), marcas bidireccionales (*Trojan Source*) y caracteres invisibles
/// (anchura cero, guion blando, etiquetas Unicode), y normaliza a NFC.
///
/// Los invisibles se usan para marcar texto y rastrear a quien lo copia, para
/// ocultar órdenes («ASCII smuggling») y para que dos cadenas que se ven
/// iguales sean distintas. NFC hace que lo que se ve igual sea igual.
pub fn limpiar_texto(texto: &str) -> String {
    texto
        .chars()
        .filter(|c| es_visible_y_seguro(*c))
        .nfc()
        .collect()
}

/// Recorta `texto` a `maximo` caracteres. Devuelve el texto y si se recortó.
pub fn recortar(texto: String, maximo: usize) -> (String, bool) {
    match texto.char_indices().nth(maximo) {
        Some((corte, _)) => (texto[..corte].to_string(), true),
        None => (texto, false),
    }
}

/// `true` si el carácter puede mostrarse.
fn es_visible_y_seguro(c: char) -> bool {
    !(es_bidi(c) || es_invisible(c) || (c.is_control() && !matches!(c, '\n' | '\t')))
}

/// Marcas de control bidireccional (embebidos, sobrescrituras, aislamientos y marcas).
fn es_bidi(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}')
}

/// Caracteres sin dibujo: anchura cero, unión de palabras e invisibles
/// matemáticos, BOM, separador vocálico mongol, guion blando y etiquetas.
fn es_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{200B}'..='\u{200D}'
            | '\u{2060}'..='\u{2064}'
            | '\u{FEFF}'
            | '\u{180E}'
            | '\u{00AD}'
            | '\u{E0000}'..='\u{E007F}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conserva_saltos_y_tabuladores() {
        assert_eq!(limpiar_texto("a\nb\tc\u{1b}"), "a\nb\tc");
    }

    #[test]
    fn recorta_por_caracteres_no_por_bytes() {
        assert_eq!(recortar("ñañaña".to_string(), 2), ("ña".to_string(), true));
        assert_eq!(recortar("ña".to_string(), 2), ("ña".to_string(), false));
    }
}
