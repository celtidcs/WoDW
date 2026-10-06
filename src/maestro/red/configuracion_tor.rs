//! Traducción de la sección `[tor]` de `wodw.toml` a la configuración de Arti.

use crate::configuracion::{ConfiguracionTor, ModoVanguardias, TransporteEnchufable};
use crate::error::{ErrorApp, Resultado};
use arti_client::config::pt::TransportConfigBuilder;
use arti_client::config::{BridgeConfigBuilder, CfgPath, PtTransportName};
use arti_client::TorClientConfig;
use tor_config::ExplicitOrAuto;
use tor_guardmgr::VanguardMode;

/// Construye la configuración de Arti: rutas, puentes, transportes y Vanguards.
///
/// # Errors
/// [`ErrorApp::CircuitoTor`] si una línea de puente o un transporte son
/// inválidos, o si Arti rechaza la configuración resultante.
pub fn construir_configuracion_tor(tor: &ConfiguracionTor) -> Resultado<TorClientConfig> {
    let mut constructor = TorClientConfig::builder();
    if let Some(ruta) = &tor.ruta_estado {
        constructor
            .storage()
            .state_dir(CfgPath::new_literal(ruta.clone()));
    }
    if let Some(ruta) = &tor.ruta_cache {
        constructor
            .storage()
            .cache_dir(CfgPath::new_literal(ruta.clone()));
    }
    for linea in &tor.puentes {
        let puente = linea.parse::<BridgeConfigBuilder>().map_err(|e| {
            ErrorApp::CircuitoTor(format!("línea de puente inválida «{linea}»: {e}"))
        })?;
        constructor.bridges().bridges().push(puente);
    }
    for transporte in &tor.transportes {
        constructor
            .bridges()
            .transports()
            .push(construir_transporte(transporte)?);
    }
    constructor
        .vanguards()
        .mode(ExplicitOrAuto::Explicit(modo_arti(tor.vanguardias)));
    constructor
        .build()
        .map_err(|e| ErrorApp::CircuitoTor(format!("configuración Tor rechazada: {e}")))
}

/// Equivalencia entre el modo de WoDW y el de Arti.
fn modo_arti(modo: ModoVanguardias) -> VanguardMode {
    match modo {
        ModoVanguardias::Lite => VanguardMode::Lite,
        ModoVanguardias::Full => VanguardMode::Full,
        ModoVanguardias::Disabled => VanguardMode::Disabled,
    }
}

/// Configura un binario de transporte enchufable gestionado por Arti.
fn construir_transporte(transporte: &TransporteEnchufable) -> Resultado<TransportConfigBuilder> {
    let protocolos = transporte
        .protocolos
        .iter()
        .map(|p| {
            p.parse::<PtTransportName>().map_err(|e| {
                ErrorApp::CircuitoTor(format!("nombre de transporte inválido «{p}»: {e}"))
            })
        })
        .collect::<Resultado<Vec<_>>>()?;
    let mut constructor = TransportConfigBuilder::default();
    constructor
        .protocols(protocolos)
        .path(CfgPath::new_literal(transporte.ruta.clone()))
        .arguments(transporte.argumentos.clone())
        .run_on_startup(false);
    Ok(constructor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tor_guardmgr::VanguardConfig;

    fn modo_resultante(modo: ModoVanguardias) -> VanguardMode {
        let tor = ConfiguracionTor {
            vanguardias: modo,
            ..Default::default()
        };
        let cfg = construir_configuracion_tor(&tor).unwrap();
        let vanguardias: &VanguardConfig = cfg.as_ref();
        vanguardias.mode()
    }

    #[test]
    fn vanguardias_se_aplican_a_la_configuracion_de_arti() {
        assert_eq!(modo_resultante(ModoVanguardias::Lite), VanguardMode::Lite);
        assert_eq!(modo_resultante(ModoVanguardias::Full), VanguardMode::Full);
        assert_eq!(
            modo_resultante(ModoVanguardias::Disabled),
            VanguardMode::Disabled
        );
    }

    #[test]
    fn puente_invalido_falla_temprano() {
        let tor = ConfiguracionTor {
            puentes: vec!["linea_invalida".to_string()],
            ..Default::default()
        };
        assert!(matches!(
            construir_configuracion_tor(&tor),
            Err(ErrorApp::CircuitoTor(_))
        ));
    }
}
