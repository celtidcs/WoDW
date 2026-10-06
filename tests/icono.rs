//! La aplicación tiene icono propio en la ventana y en la barra de tareas.

use wodw::configuracion::ConfiguracionWodw;

/// Lado mínimo del icono: Windows usa hasta 256 px en el Explorador.
const LADO_MINIMO: u32 = 256;

#[test]
fn el_icono_se_decodifica_y_es_cuadrado() {
    let icono = wodw::ui::icono::icono_aplicacion().expect("el icono debe decodificarse");
    assert_eq!(icono.width, icono.height, "el icono no es cuadrado");
    assert!(
        icono.width >= LADO_MINIMO,
        "icono demasiado pequeño: {}",
        icono.width
    );
    assert_eq!(
        icono.rgba.len(),
        (icono.width * icono.height * 4) as usize,
        "el búfer RGBA no corresponde a las dimensiones"
    );
}

#[test]
fn la_ventana_lleva_el_icono() {
    let cfg = ConfiguracionWodw::default();
    let opciones = wodw::ui::crear_opciones_nativas_seguras(&cfg.interfaz, "WoDW");
    assert!(
        opciones.viewport.icon.is_some(),
        "la ventana no lleva icono"
    );
}
