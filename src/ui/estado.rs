//! Estado del navegador independiente de `egui`: pestañas, historial, barra de
//! direcciones, botón del pánico por teclado e inactividad.
//!
//! Toda mutación pasa por métodos con significado que preservan sus
//! invariantes: siempre hay al menos una pestaña y el índice activo es
//! válido. La purga sobrescribe con ceros las cadenas antes de soltarlas.

use crate::maestro::navegacion::{ContenidoPagina, FalloNavegacion, ResultadoNavegacion};
use crate::seguridad::enlaces::AvisoEnlace;
use crate::seguridad::purgar_cadenas;
use std::time::{Duration, Instant};

/// Puntos suspensivos añadidos a los títulos truncados.
const ELIPSIS: &str = "…";

/// Lo que muestra una pestaña.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstadoContenido {
    /// Pantalla de bienvenida.
    Bienvenida,
    /// Esperando respuesta de la sesión.
    Cargando,
    /// Contenido recibido.
    Pagina(ContenidoPagina),
    /// Error mostrado al usuario.
    Error(String),
}

/// Pestaña de navegación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pestana {
    id: u64,
    titulo: String,
    url: String,
    atras: Vec<String>,
    adelante: Vec<String>,
    contenido: EstadoContenido,
    solicitud: u64,
}

impl Pestana {
    fn nueva(id: u64) -> Self {
        Self {
            id,
            titulo: String::new(),
            url: String::new(),
            atras: Vec::new(),
            adelante: Vec::new(),
            contenido: EstadoContenido::Bienvenida,
            solicitud: 0,
        }
    }

    /// Identificador estable de la pestaña.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// URL actual (vacía en la bienvenida).
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Contenido actual.
    pub fn contenido(&self) -> &EstadoContenido {
        &self.contenido
    }

    /// Número de la última solicitud de navegación de la pestaña.
    pub fn solicitud(&self) -> u64 {
        self.solicitud
    }

    /// Historial hacia atrás (más reciente al final).
    pub fn historial_atras(&self) -> &[String] {
        &self.atras
    }

    /// Título completo de la pestaña.
    pub fn titulo(&self) -> &str {
        &self.titulo
    }

    /// Título truncado a `maximo` caracteres (nunca corta un carácter por la mitad).
    pub fn titulo_corto(&self, maximo: usize) -> String {
        let base = if self.titulo.is_empty() {
            "Nueva pestaña"
        } else {
            &self.titulo
        };
        truncar(base, maximo)
    }

    /// Sobrescribe con ceros todo el contenido textual.
    fn purgar(&mut self) {
        purgar_cadenas(
            [&mut self.titulo, &mut self.url]
                .into_iter()
                .chain(self.atras.iter_mut())
                .chain(self.adelante.iter_mut()),
        );
        if let EstadoContenido::Pagina(ContenidoPagina::Documento {
            titulo,
            texto,
            enlaces,
            medios,
            ..
        }) = &mut self.contenido
        {
            purgar_cadenas(medios.iter_mut().flat_map(|m| [&mut m.url, &mut m.texto]));
            purgar_cadenas(
                [titulo, texto]
                    .into_iter()
                    .chain(enlaces.iter_mut().flat_map(|e| {
                        [&mut e.texto, &mut e.url]
                            .into_iter()
                            .chain(e.aviso.as_mut().map(AvisoEnlace::host_real_mut))
                    })),
            );
        }
        if let EstadoContenido::Pagina(ContenidoPagina::Medio { datos, .. }) = &mut self.contenido {
            datos.fill(0);
        }
        self.atras.clear();
        self.adelante.clear();
        self.contenido = EstadoContenido::Bienvenida;
        self.solicitud += 1;
    }
}

/// Trunca `texto` a `maximo` caracteres añadiendo una elipsis.
pub fn truncar(texto: &str, maximo: usize) -> String {
    if texto.chars().count() <= maximo {
        return texto.to_string();
    }
    let corte: String = texto.chars().take(maximo.saturating_sub(1)).collect();
    format!("{corte}{ELIPSIS}")
}

/// Navegación pendiente de enviar a la sesión.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolicitudNavegacion {
    /// Pestaña de origen.
    pub id_pestana: u64,
    /// Número de solicitud de esa pestaña.
    pub solicitud: u64,
    /// URL absoluta.
    pub direccion: String,
}

/// Parámetros de comportamiento del estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParametrosEstado {
    /// Pulsaciones de `Esc` que disparan el pánico.
    pub pulsaciones_panico: u8,
    /// Ventana máxima entre pulsaciones.
    pub ventana_panico: Duration,
    /// Inactividad tras la que se purga todo (`None` = nunca).
    pub inactividad_purga: Option<Duration>,
}

/// Estado completo del navegador.
#[derive(Debug, Clone)]
pub struct EstadoNavegador {
    pestanas: Vec<Pestana>,
    activa: usize,
    siguiente_id: u64,
    barra: String,
    parametros: ParametrosEstado,
    pulsaciones_esc: u8,
    ultimo_esc: Option<Instant>,
    ultima_actividad: Instant,
}

impl EstadoNavegador {
    /// Estado inicial con una pestaña de bienvenida.
    pub fn nuevo(parametros: ParametrosEstado, ahora: Instant) -> Self {
        Self {
            pestanas: vec![Pestana::nueva(1)],
            activa: 0,
            siguiente_id: 2,
            barra: String::new(),
            parametros,
            pulsaciones_esc: 0,
            ultimo_esc: None,
            ultima_actividad: ahora,
        }
    }

    /// Pestañas abiertas.
    pub fn pestanas(&self) -> &[Pestana] {
        &self.pestanas
    }

    /// Índice de la pestaña activa.
    pub fn indice_activo(&self) -> usize {
        self.activa
    }

    /// Pestaña activa.
    pub fn pestana_activa(&self) -> &Pestana {
        &self.pestanas[self.activa]
    }

    /// Texto de la barra de direcciones (editable por la interfaz).
    pub fn barra_mut(&mut self) -> &mut String {
        &mut self.barra
    }

    /// Texto de la barra de direcciones.
    pub fn barra(&self) -> &str {
        &self.barra
    }

    /// Abre una pestaña nueva y la activa.
    pub fn abrir_pestana(&mut self) {
        self.pestanas.push(Pestana::nueva(self.siguiente_id));
        self.siguiente_id += 1;
        self.activar(self.pestanas.len() - 1);
    }

    /// Activa la pestaña `indice` si existe.
    pub fn activar(&mut self, indice: usize) {
        if indice < self.pestanas.len() {
            self.activa = indice;
            self.barra = self.pestanas[indice].url.clone();
        }
    }

    /// Cierra la pestaña `indice` (nunca la última) y devuelve su id.
    pub fn cerrar_pestana(&mut self, indice: usize) -> Option<u64> {
        if self.pestanas.len() <= 1 || indice >= self.pestanas.len() {
            return None;
        }
        let mut cerrada = self.pestanas.remove(indice);
        cerrada.purgar();
        if self.activa >= self.pestanas.len() || self.activa > indice {
            self.activa = self.activa.saturating_sub(1).min(self.pestanas.len() - 1);
        }
        self.activar(self.activa);
        Some(cerrada.id)
    }

    /// Inicia la navegación de la pestaña activa a `direccion` (ya resuelta).
    pub fn navegar(&mut self, direccion: String) -> SolicitudNavegacion {
        let pestana = &mut self.pestanas[self.activa];
        if !pestana.url.is_empty() && pestana.url != direccion {
            pestana.atras.push(std::mem::take(&mut pestana.url));
            pestana.adelante.clear();
        }
        Self::cargar(pestana, direccion, &mut self.barra)
    }

    /// Retrocede en el historial de la pestaña activa.
    pub fn retroceder(&mut self) -> Option<SolicitudNavegacion> {
        let pestana = &mut self.pestanas[self.activa];
        let anterior = pestana.atras.pop()?;
        pestana.adelante.push(std::mem::take(&mut pestana.url));
        Some(Self::cargar(pestana, anterior, &mut self.barra))
    }

    /// Avanza en el historial de la pestaña activa.
    pub fn avanzar(&mut self) -> Option<SolicitudNavegacion> {
        let pestana = &mut self.pestanas[self.activa];
        let siguiente = pestana.adelante.pop()?;
        pestana.atras.push(std::mem::take(&mut pestana.url));
        Some(Self::cargar(pestana, siguiente, &mut self.barra))
    }

    fn cargar(pestana: &mut Pestana, direccion: String, barra: &mut String) -> SolicitudNavegacion {
        pestana.solicitud += 1;
        pestana.url.clone_from(&direccion);
        pestana.titulo.clone_from(&direccion);
        pestana.contenido = EstadoContenido::Cargando;
        barra.clone_from(&direccion);
        SolicitudNavegacion {
            id_pestana: pestana.id,
            solicitud: pestana.solicitud,
            direccion,
        }
    }

    /// Purga la pestaña `id_pestana` tras un incidente ajeno a una navegación
    /// (por ejemplo, un medio hostil al reproducirlo) y muestra el mensaje.
    pub fn purgar_pestana_por_incidente(&mut self, id_pestana: u64, fallo: FalloNavegacion) {
        let Some(solicitud) = self
            .pestanas
            .iter()
            .find(|p| p.id == id_pestana)
            .map(|p| p.solicitud)
        else {
            return;
        };
        self.aplicar_resultado(id_pestana, solicitud, Err(fallo));
    }

    /// Aplica el resultado de una navegación; descarta respuestas obsoletas.
    pub fn aplicar_resultado(
        &mut self,
        id_pestana: u64,
        solicitud: u64,
        resultado: Result<ResultadoNavegacion, FalloNavegacion>,
    ) {
        let Some(posicion) = self.pestanas.iter().position(|p| p.id == id_pestana) else {
            return;
        };
        let pestana = &mut self.pestanas[posicion];
        if pestana.solicitud != solicitud {
            return;
        }
        match resultado {
            Ok(r) => {
                if let ContenidoPagina::Documento { titulo, .. } = &r.contenido {
                    if !titulo.is_empty() {
                        pestana.titulo.clone_from(titulo);
                    }
                }
                pestana.url = r.url;
                pestana.contenido = EstadoContenido::Pagina(r.contenido);
            }
            Err(fallo) if fallo.purgar_pestana => {
                pestana.purgar();
                pestana.contenido = EstadoContenido::Error(fallo.mensaje);
            }
            Err(fallo) => pestana.contenido = EstadoContenido::Error(fallo.mensaje),
        }
        if posicion == self.activa {
            self.barra = self.pestanas[posicion].url.clone();
        }
    }

    /// Purga todas las pestañas, el historial y la barra; deja una pestaña vacía.
    pub fn purgar_todo(&mut self) {
        for pestana in &mut self.pestanas {
            pestana.purgar();
        }
        purgar_cadenas([&mut self.barra]);
        self.pestanas.truncate(1);
        self.activa = 0;
        self.pulsaciones_esc = 0;
        self.ultimo_esc = None;
    }

    /// Registra una pulsación de `Esc`; devuelve `true` si dispara el pánico.
    pub fn registrar_escape(&mut self, ahora: Instant) -> bool {
        let consecutiva = self
            .ultimo_esc
            .is_some_and(|previo| ahora.duration_since(previo) <= self.parametros.ventana_panico);
        self.pulsaciones_esc = if consecutiva {
            self.pulsaciones_esc.saturating_add(1)
        } else {
            1
        };
        self.ultimo_esc = Some(ahora);
        self.pulsaciones_esc >= self.parametros.pulsaciones_panico
    }

    /// Anota actividad del usuario.
    pub fn registrar_actividad(&mut self, ahora: Instant) {
        self.ultima_actividad = ahora;
    }

    /// `true` si ha vencido el plazo de inactividad (y reinicia el cómputo).
    pub fn inactividad_vencida(&mut self, ahora: Instant) -> bool {
        let vencida = self
            .parametros
            .inactividad_purga
            .is_some_and(|plazo| ahora.duration_since(self.ultima_actividad) >= plazo);
        if vencida {
            self.ultima_actividad = ahora;
        }
        vencida
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maestro::EnlaceRevisado;

    fn parametros() -> ParametrosEstado {
        ParametrosEstado {
            pulsaciones_panico: 3,
            ventana_panico: Duration::from_millis(1500),
            inactividad_purga: Some(Duration::from_secs(60)),
        }
    }

    fn documento(titulo: &str) -> ResultadoNavegacion {
        ResultadoNavegacion {
            url: "http://a.onion/".to_string(),
            codigo_estado: 200,
            contenido: ContenidoPagina::Documento {
                titulo: titulo.to_string(),
                texto: "DATO SENSIBLE".to_string(),
                enlaces: vec![EnlaceRevisado {
                    texto: "x".into(),
                    url: "http://b.onion/".into(),
                    aviso: Some(AvisoEnlace::Punycode {
                        host_real: "xn--b.onion".into(),
                    }),
                }],
                medios: vec![],
                recortado: false,
            },
        }
    }

    #[test]
    fn titulo_no_ascii_se_trunca_por_caracteres() {
        assert_eq!(
            truncar("http://aaaaaaaaañññ.onion", 17),
            "http://aaaaaaaaa…"
        );
        assert_eq!(truncar("ñandú", 20), "ñandú");
        for maximo in 0..30 {
            let _ = truncar("ññññññññññññññññññññññññ", maximo);
        }
    }

    #[test]
    fn navegacion_historial_y_respuestas_obsoletas() {
        let mut e = EstadoNavegador::nuevo(parametros(), Instant::now());
        let s1 = e.navegar("http://uno.onion/".into());
        let s2 = e.navegar("http://dos.onion/".into());
        e.aplicar_resultado(s1.id_pestana, s1.solicitud, Ok(documento("viejo")));
        assert_eq!(e.pestana_activa().contenido(), &EstadoContenido::Cargando);
        e.aplicar_resultado(s2.id_pestana, s2.solicitud, Ok(documento("Título")));
        assert_eq!(e.pestana_activa().titulo(), "Título");
        let atras = e.retroceder().unwrap();
        assert_eq!(atras.direccion, "http://uno.onion/");
        assert_eq!(e.avanzar().unwrap().direccion, "http://a.onion/");
    }

    #[test]
    fn purga_total_borra_todo_rastro() {
        let mut e = EstadoNavegador::nuevo(parametros(), Instant::now());
        let s = e.navegar("http://secreto.onion/".into());
        e.aplicar_resultado(s.id_pestana, s.solicitud, Ok(documento("Secreto")));
        e.navegar("http://otro.onion/".into());
        e.abrir_pestana();
        e.navegar("http://tercero.onion/".into());
        e.purgar_todo();
        assert_eq!(e.pestanas().len(), 1);
        let p = e.pestana_activa();
        assert!(p.url().is_empty() && p.titulo().is_empty() && p.historial_atras().is_empty());
        assert_eq!(p.contenido(), &EstadoContenido::Bienvenida);
        assert!(e.barra().is_empty());
    }

    #[test]
    fn fallo_con_purga_vacia_la_pestana() {
        let mut e = EstadoNavegador::nuevo(parametros(), Instant::now());
        let s = e.navegar("http://malo.onion/".into());
        let fallo = FalloNavegacion {
            mensaje: "hostil".into(),
            purgar_pestana: true,
        };
        e.aplicar_resultado(s.id_pestana, s.solicitud, Err(fallo));
        assert!(e.pestana_activa().url().is_empty());
        assert_eq!(
            e.pestana_activa().contenido(),
            &EstadoContenido::Error("hostil".into())
        );
    }

    #[test]
    fn panico_por_teclado_e_inactividad() {
        let t0 = Instant::now();
        let mut e = EstadoNavegador::nuevo(parametros(), t0);
        assert!(!e.registrar_escape(t0));
        assert!(!e.registrar_escape(t0 + Duration::from_secs(2)));
        assert!(!e.registrar_escape(t0 + Duration::from_millis(2500)));
        assert!(e.registrar_escape(t0 + Duration::from_millis(3000)));
        assert!(!e.inactividad_vencida(t0 + Duration::from_secs(59)));
        assert!(e.inactividad_vencida(t0 + Duration::from_secs(60)));
        assert!(!e.inactividad_vencida(t0 + Duration::from_secs(61)));
    }

    #[test]
    fn nunca_se_cierra_la_ultima_pestana_y_el_indice_sigue_valido() {
        let mut e = EstadoNavegador::nuevo(parametros(), Instant::now());
        assert_eq!(e.cerrar_pestana(0), None);
        e.abrir_pestana();
        e.abrir_pestana();
        e.activar(2);
        assert_eq!(e.cerrar_pestana(0), Some(1));
        assert_eq!(e.indice_activo(), 1);
        assert_eq!(e.cerrar_pestana(1), Some(3));
        assert_eq!(e.indice_activo(), 0);
    }
}
