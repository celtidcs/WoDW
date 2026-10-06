//! Sanitización de documentos HTML en modo «Safest»: solo texto, título y enlaces.
//!
//! No se construye DOM ni se ejecuta nada: el documento se recorre una vez,
//! se descartan los elementos activos o incrustados y se extraen el texto
//! visible y los hipervínculos `http`/`https` resueltos contra la URL de origen.

use crate::ipc::mensajes::Enlace;
use url::Url;

/// Elementos cuyo contenido se descarta por completo.
const ELEMENTOS_IGNORADOS: &[&str] = &[
    "script", "style", "noscript", "template", "iframe", "object", "embed", "svg", "math",
];

/// Elementos que separan bloques de texto.
const ELEMENTOS_DE_BLOQUE: &[&str] = &[
    "p",
    "br",
    "div",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "li",
    "tr",
    "section",
    "article",
    "header",
    "footer",
    "ul",
    "ol",
    "table",
    "blockquote",
    "pre",
    "hr",
];

/// Esquemas de enlace admitidos.
const ESQUEMAS_PERMITIDOS: &[&str] = &["http", "https"];

/// Documento sanitizado.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocumentoSanitizado {
    /// Título del documento.
    pub titulo: String,
    /// Texto visible, un bloque por línea.
    pub texto: String,
    /// Enlaces http/https absolutos.
    pub enlaces: Vec<Enlace>,
}

/// Etiqueta interpretada.
struct Etiqueta {
    nombre: String,
    cierre: bool,
    atributos: Vec<(String, String)>,
}

/// Estado del recorrido.
#[derive(Default)]
struct Recorrido {
    documento: DocumentoSanitizado,
    ignorando: Option<String>,
    en_titulo: bool,
    enlace_abierto: Option<(String, String)>,
}

/// Sanitiza `html` resolviendo enlaces relativos contra `url_origen`.
pub fn sanitizar_html(html: &[u8], url_origen: &str) -> DocumentoSanitizado {
    let texto = String::from_utf8_lossy(html);
    // Minúsculas ASCII: mismos desplazamientos en bytes que `texto`.
    let minusculas = texto.to_ascii_lowercase();
    let base = Url::parse(url_origen).ok();
    let mut recorrido = Recorrido::default();
    let mut posicion = 0usize;
    loop {
        if let Some(nombre) = &recorrido.ignorando {
            // Dentro de un elemento ignorado (script, style…) solo cuenta su cierre,
            // como en el estado «script data» de HTML5: un `<` del código no abre etiqueta.
            match minusculas[posicion..].find(&format!("</{nombre}")) {
                Some(relativa) => posicion += relativa,
                None => break,
            }
        }
        let Some(relativa) = texto[posicion..].find('<') else {
            recorrido.texto(&texto[posicion..]);
            break;
        };
        let inicio = posicion + relativa;
        recorrido.texto(&texto[posicion..inicio]);
        let (etiqueta, consumido) = leer_etiqueta(&texto[inicio..]);
        if let Some(etiqueta) = etiqueta {
            recorrido.etiqueta(etiqueta, base.as_ref());
        }
        posicion = inicio + consumido;
    }
    recorrido.cerrar()
}

impl Recorrido {
    fn texto(&mut self, crudo: &str) {
        if self.ignorando.is_some() || crudo.is_empty() {
            return;
        }
        let limpio = limpiar_texto(&decodificar_entidades(crudo));
        if self.en_titulo {
            self.documento.titulo.push_str(&limpio);
            return;
        }
        if let Some((texto, _)) = &mut self.enlace_abierto {
            texto.push_str(&limpio);
        }
        self.documento.texto.push_str(&limpio);
    }

    fn etiqueta(&mut self, etiqueta: Etiqueta, base: Option<&Url>) {
        if let Some(ignorado) = &self.ignorando {
            if etiqueta.cierre && &etiqueta.nombre == ignorado {
                self.ignorando = None;
            }
            return;
        }
        if ELEMENTOS_IGNORADOS.contains(&etiqueta.nombre.as_str()) && !etiqueta.cierre {
            self.ignorando = Some(etiqueta.nombre);
            return;
        }
        if ELEMENTOS_DE_BLOQUE.contains(&etiqueta.nombre.as_str()) {
            self.documento.texto.push('\n');
        }
        if etiqueta.nombre == "title" {
            self.en_titulo = !etiqueta.cierre;
        } else if etiqueta.nombre == "a" {
            self.enlace(etiqueta, base);
        }
    }

    fn enlace(&mut self, etiqueta: Etiqueta, base: Option<&Url>) {
        self.finalizar_enlace();
        if etiqueta.cierre {
            return;
        }
        let href = etiqueta
            .atributos
            .into_iter()
            .find(|(nombre, _)| nombre == "href")
            .map(|(_, valor)| valor);
        if let Some(url) = href.and_then(|h| resolver_enlace(&h, base)) {
            self.enlace_abierto = Some((String::new(), url));
        }
    }

    fn finalizar_enlace(&mut self) {
        if let Some((texto, url)) = self.enlace_abierto.take() {
            let texto = normalizar_espacios(&texto);
            let texto = if texto.is_empty() { url.clone() } else { texto };
            self.documento.enlaces.push(Enlace { texto, url });
        }
    }

    fn cerrar(mut self) -> DocumentoSanitizado {
        self.finalizar_enlace();
        let lineas: Vec<String> = self
            .documento
            .texto
            .lines()
            .map(normalizar_espacios)
            .filter(|l| !l.is_empty())
            .collect();
        self.documento.texto = lineas.join("\n");
        self.documento.titulo = normalizar_espacios(&self.documento.titulo);
        self.documento
    }
}

/// Lee una etiqueta que empieza en `<`. Devuelve la etiqueta (si es un
/// elemento) y los bytes consumidos. Los comentarios y declaraciones se consumen
/// sin producir etiqueta.
fn leer_etiqueta(fuente: &str) -> (Option<Etiqueta>, usize) {
    if let Some(cuerpo) = fuente.strip_prefix("<!--") {
        let fin = cuerpo.find("-->").map_or(fuente.len(), |p| 4 + p + 3);
        return (None, fin);
    }
    let fin = fin_de_etiqueta(fuente);
    let fin_interior = if fuente[..fin].ends_with('>') {
        fin - 1
    } else {
        fin
    };
    let interior = fuente.get(1..fin_interior).unwrap_or_default();
    if interior.starts_with(['!', '?']) {
        return (None, fin);
    }
    (interpretar_etiqueta(interior), fin)
}

/// Posición tras el `>` que cierra la etiqueta, respetando comillas.
fn fin_de_etiqueta(fuente: &str) -> usize {
    let mut comilla: Option<char> = None;
    for (posicion, caracter) in fuente.char_indices().skip(1) {
        match (comilla, caracter) {
            (Some(c), x) if x == c => comilla = None,
            (None, '"' | '\'') => comilla = Some(caracter),
            (None, '>') => return posicion + 1,
            _ => {}
        }
    }
    fuente.len()
}

/// Interpreta `nombre atr="v" atr2=v2` (o `/nombre`).
fn interpretar_etiqueta(interior: &str) -> Option<Etiqueta> {
    let (cierre, interior) = match interior.strip_prefix('/') {
        Some(resto) => (true, resto),
        None => (false, interior),
    };
    let fin_nombre = interior
        .find(|c: char| c.is_whitespace() || c == '/')
        .unwrap_or(interior.len());
    let nombre = interior[..fin_nombre].to_ascii_lowercase();
    if nombre.is_empty() || !nombre.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    Some(Etiqueta {
        nombre,
        cierre,
        atributos: interpretar_atributos(&interior[fin_nombre..]),
    })
}

/// Interpreta la lista de atributos de una etiqueta.
fn interpretar_atributos(mut resto: &str) -> Vec<(String, String)> {
    let mut atributos = Vec::new();
    loop {
        resto = resto.trim_start_matches(|c: char| c.is_whitespace() || c == '/');
        let fin_nombre = resto
            .find(|c: char| c.is_whitespace() || c == '=' || c == '/')
            .unwrap_or(resto.len());
        if fin_nombre == 0 {
            return atributos;
        }
        let nombre = resto[..fin_nombre].to_ascii_lowercase();
        resto = resto[fin_nombre..].trim_start();
        let valor = match resto.strip_prefix('=') {
            Some(tras_igual) => {
                let (valor, siguiente) = leer_valor_atributo(tras_igual.trim_start());
                resto = siguiente;
                decodificar_entidades(valor)
            }
            None => String::new(),
        };
        atributos.push((nombre, valor));
    }
}

/// Lee un valor de atributo entre comillas o sin ellas.
fn leer_valor_atributo(fuente: &str) -> (&str, &str) {
    for comilla in ['"', '\''] {
        if let Some(interior) = fuente.strip_prefix(comilla) {
            let fin = interior.find(comilla).unwrap_or(interior.len());
            let siguiente = interior.get(fin + 1..).unwrap_or_default();
            return (&interior[..fin], siguiente);
        }
    }
    let fin = fuente.find(char::is_whitespace).unwrap_or(fuente.len());
    (&fuente[..fin], &fuente[fin..])
}

/// Resuelve `href` contra `base` y solo acepta http/https.
fn resolver_enlace(href: &str, base: Option<&Url>) -> Option<String> {
    let href = href.trim();
    let url = match base {
        Some(base) => base.join(href).ok()?,
        None => Url::parse(href).ok()?,
    };
    ESQUEMAS_PERMITIDOS
        .contains(&url.scheme())
        .then(|| url.to_string())
}

/// Decodifica las entidades HTML más comunes y las numéricas.
fn decodificar_entidades(texto: &str) -> String {
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
        _ => {
            let numero = nombre.strip_prefix('#')?;
            let valor = match numero.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => numero.parse().ok()?,
            };
            char::from_u32(valor)
        }
    }
}

/// Elimina controles (salvo salto de línea y tabulador) y marcas bidireccionales
/// usadas en ataques *Trojan Source*.
fn limpiar_texto(texto: &str) -> String {
    texto
        .chars()
        .filter(|c| !es_bidi(*c))
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect()
}

/// Marcas de control bidireccional (embebidos, sobrescrituras, aislamientos y marcas).
fn es_bidi(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}')
}

/// Colapsa espacios consecutivos y recorta extremos.
fn normalizar_espacios(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGEN: &str = "http://ejemplo.onion/dir/pagina.html";

    #[test]
    fn extrae_titulo_texto_y_descarta_scripts() {
        let d = sanitizar_html(
            b"<html><head><title> Mi &amp; Sitio </title><script>alert('x')</script></head><body><h1>Hola</h1><p>Mundo</p></body></html>",
            ORIGEN,
        );
        assert_eq!(d.titulo, "Mi & Sitio");
        assert_eq!(d.texto, "Hola\nMundo");
    }

    #[test]
    fn enlaces_se_resuelven_y_filtran_por_esquema() {
        let d = sanitizar_html(
            b"<a href=\"javascript:alert(1)\">a</a><a href='file:///C:/x'>b</a><a data-href=\"http://trampa.onion\" href=\"otra.html\">Otra <b>pagina</b></a><a href=data:text/html,x>d</a><a href=\"https://b.onion/\"></a>",
            ORIGEN,
        );
        let urls: Vec<&str> = d.enlaces.iter().map(|e| e.url.as_str()).collect();
        assert_eq!(
            urls,
            ["http://ejemplo.onion/dir/otra.html", "https://b.onion/"]
        );
        assert_eq!(d.enlaces[0].texto, "Otra pagina");
        assert_eq!(d.enlaces[1].texto, "https://b.onion/");
    }

    #[test]
    fn filtra_bidi_y_controles() {
        let d = sanitizar_html("<p>A\u{202E}txt.exe\u{1b}[31m</p>".as_bytes(), ORIGEN);
        assert_eq!(d.texto, "Atxt.exe[31m");
    }

    #[test]
    fn comentario_con_mayor_que_no_filtra_texto_oculto() {
        let d = sanitizar_html(
            b"<!-- <a href=x> --> visible <script>if(a<b)x()</script>fin",
            ORIGEN,
        );
        assert_eq!(d.texto, "visible fin");
        assert!(d.enlaces.is_empty());
    }

    #[test]
    fn entrada_truncada_no_entra_en_panico() {
        for entrada in [
            "<",
            "<a href=\"",
            "&#xFFFFFFFF;",
            "<a href='x' ",
            "&",
            "<!--",
            "<añ",
            "<a b=ñ",
        ] {
            let _ = sanitizar_html(entrada.as_bytes(), ORIGEN);
        }
    }
}
