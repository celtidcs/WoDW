//! Decodificación de las entidades HTML: las con nombre habituales y las
//! numéricas, decimales y hexadecimales. Lo que no se reconoce se deja tal
//! cual, sin interpretarlo.

/// Base de las referencias numéricas hexadecimales (`&#xE9;`).
const BASE_HEXADECIMAL: u32 = 16;

/// Decodifica las entidades HTML más comunes y las numéricas.
pub(crate) fn decodificar_entidades(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(inicio) = resto.find('&') {
        salida.push_str(&resto[..inicio]);
        let tras = &resto[inicio + 1..];
        match tras.find(';').filter(|fin| *fin <= LONGITUD_MAXIMA_ENTIDAD) {
            Some(fin) => match entidad(&tras[..fin]) {
                Some(caracter) => {
                    salida.push(caracter);
                    resto = &tras[fin + 1..];
                }
                None => {
                    salida.push('&');
                    resto = tras;
                }
            },
            None => {
                salida.push('&');
                resto = tras;
            }
        }
    }
    salida.push_str(resto);
    salida
}

/// Longitud máxima del nombre de una entidad reconocida (`#x10FFFF` = 8).
const LONGITUD_MAXIMA_ENTIDAD: usize = 8;

/// Carácter de una entidad sin `&` ni `;`.
fn entidad(nombre: &str) -> Option<char> {
    match nombre {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => entidad_con_nombre(nombre).or_else(|| entidad_numerica(nombre)),
    }
}

/// Entidades con nombre habituales en páginas en español e inglés.
const ENTIDADES: &[(&str, char)] = &[
    ("mdash", '—'),
    ("ndash", '–'),
    ("hellip", '…'),
    ("laquo", '«'),
    ("raquo", '»'),
    ("lsquo", '‘'),
    ("rsquo", '’'),
    ("ldquo", '“'),
    ("rdquo", '”'),
    ("bull", '•'),
    ("middot", '·'),
    ("copy", '©'),
    ("reg", '®'),
    ("trade", '™'),
    ("euro", '€'),
    ("pound", '£'),
    ("yen", '¥'),
    ("cent", '¢'),
    ("deg", '°'),
    ("times", '×'),
    ("divide", '÷'),
    ("plusmn", '±'),
    ("para", '¶'),
    ("sect", '§'),
    ("iexcl", '¡'),
    ("iquest", '¿'),
    ("ordf", 'ª'),
    ("ordm", 'º'),
    ("aacute", 'á'),
    ("eacute", 'é'),
    ("iacute", 'í'),
    ("oacute", 'ó'),
    ("uacute", 'ú'),
    ("Aacute", 'Á'),
    ("Eacute", 'É'),
    ("Iacute", 'Í'),
    ("Oacute", 'Ó'),
    ("Uacute", 'Ú'),
    ("ntilde", 'ñ'),
    ("Ntilde", 'Ñ'),
    ("uuml", 'ü'),
    ("Uuml", 'Ü'),
    ("agrave", 'à'),
    ("egrave", 'è'),
    ("ccedil", 'ç'),
    ("Ccedil", 'Ç'),
    ("ouml", 'ö'),
    ("auml", 'ä'),
    ("szlig", 'ß'),
    ("rarr", '→'),
    ("larr", '←'),
    ("uarr", '↑'),
    ("darr", '↓'),
];

/// Entidad con nombre de la tabla [`ENTIDADES`].
fn entidad_con_nombre(nombre: &str) -> Option<char> {
    ENTIDADES
        .iter()
        .find(|(n, _)| *n == nombre)
        .map(|(_, c)| *c)
}

/// Entidad numérica decimal (`#233`) o hexadecimal (`#xE9`).
fn entidad_numerica(nombre: &str) -> Option<char> {
    {
        let numero = nombre.strip_prefix('#')?;
        let valor = match numero.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, BASE_HEXADECIMAL).ok()?,
            None => numero.parse().ok()?,
        };
        char::from_u32(valor)
    }
}
