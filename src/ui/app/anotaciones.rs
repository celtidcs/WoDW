//! Lo que la ventana pasa al registro de la sesión.

use super::VentanaPrincipal;
use crate::ids::motor::ResumenTelemetria;
use crate::maestro::{FalloNavegacion, ResultadoNavegacion};
use crate::registro::CategoriaRegistro;
use crate::ui::textos;

impl VentanaPrincipal {
    /// Anota en el registro el resultado de una navegación: un incidente
    /// siempre; una página abierta o un fallo ordinario, solo en Completo.
    pub(super) fn anotar_navegacion(
        &mut self,
        resultado: &Result<ResultadoNavegacion, FalloNavegacion>,
    ) {
        match resultado {
            Ok(r) => self.registro.anotar(
                CategoriaRegistro::Navegacion,
                textos::anotacion_pagina_abierta(&r.url, r.codigo_estado),
            ),
            Err(f) if f.purgar_pestana => self
                .registro
                .anotar(CategoriaRegistro::Incidente, f.mensaje.clone()),
            Err(f) => self.registro.anotar(
                CategoriaRegistro::Navegacion,
                textos::anotacion_fallo(&f.mensaje),
            ),
        }
    }

    /// Pasa al registro los eventos nuevos del IDS.
    pub(super) fn anotar_eventos_ids(&mut self, resumen: &ResumenTelemetria) {
        let nuevos = resumen
            .total_eventos
            .saturating_sub(self.eventos_ids_registrados);
        let tomar = usize::try_from(nuevos)
            .unwrap_or(usize::MAX)
            .min(resumen.historial_reciente.len());
        let inicio = resumen.historial_reciente.len() - tomar;
        for evento in &resumen.historial_reciente[inicio..] {
            self.registro.anotar(
                CategoriaRegistro::Incidente,
                textos::anotacion_ids(evento.severidad, &evento.vector),
            );
        }
        self.eventos_ids_registrados = resumen.total_eventos;
    }
}
