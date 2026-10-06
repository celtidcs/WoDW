//! Pruebas de coherencia entre la documentación, la configuración de ejemplo
//! y el código, separadas del código que vigilan.

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    /// La arquitectura documentada nombra los componentes implementados y tiene
    /// una sección explícita de limitaciones.
    #[test]
    fn arquitectura_documentada_describe_componentes_clave() {
        let arch_path = Path::new("documentacion/arquitectura.md");
        let arch_content = fs::read_to_string(arch_path).expect("Lectura de arquitectura.md");
        let terminos_clave = [
            "arti-client",
            "Stream Isolation",
            "Maestro",
            "sub-Worker efímero",
            "Landlock",
            "seccomp",
            "Job Object",
            "integridad baja",
            "Vanguards",
            "paso bajo",
            "zeroize",
            "Honeypot",
            "canario",
            "Botón del Pánico",
            "respuesta automática",
            "wodw.toml",
            "## Limitaciones conocidas",
        ];
        for termino in terminos_clave {
            assert!(
                arch_content.contains(termino),
                "La arquitectura documentada debe describir: {}",
                termino
            );
        }
    }

    /// Cada motor por defecto debe estar documentado con su nombre en negrita.
    #[test]
    fn funcionalidades_documentan_motores_darkweb() {
        let func_content = fs::read_to_string("documentacion/funcionalidades.md")
            .expect("Lectura de funcionalidades.md");
        for motor in crate::configuracion::ConfiguracionMotores::default().lista {
            let marca = format!("**{}:**", motor.nombre);
            assert!(
                func_content.contains(&marca),
                "Las funcionalidades deben documentar el motor con formato '{}'",
                marca
            );
        }
    }

    /// El ejemplo documentado coincide exactamente con los valores por defecto.
    #[test]
    fn ejemplo_de_configuracion_coincide_con_los_valores_por_defecto() {
        let texto = fs::read_to_string("wodw.ejemplo.toml").expect("Lectura de wodw.ejemplo.toml");
        let cfg = crate::configuracion::ConfiguracionWodw::desde_texto(&texto)
            .expect("wodw.ejemplo.toml debe ser válido");
        assert_eq!(cfg, crate::configuracion::ConfiguracionWodw::default());
    }

    #[test]
    fn configuracion_audit_justifica_excepcion_rsa() {
        let audit_cfg = Path::new(".cargo/audit.toml");
        assert!(audit_cfg.exists(), ".cargo/audit.toml debe existir");

        let contenido = fs::read_to_string(audit_cfg).expect("Lectura de .cargo/audit.toml");
        assert!(
            contenido.contains("RUSTSEC-2023-0071"),
            "Debe registrarse RUSTSEC-2023-0071"
        );
        assert!(
            contenido.contains("arti-client"),
            "Debe justificarse el origen en arti-client"
        );
    }

    #[test]
    fn version_en_cargo_toml_coincide_con_historial_de_cambios() {
        let cargo_str = fs::read_to_string("Cargo.toml").expect("Lectura de Cargo.toml");
        let historial_str = fs::read_to_string("documentacion/historial-de-cambios.md")
            .expect("Lectura de historial-de-cambios.md");

        let version_line = cargo_str
            .lines()
            .find(|l| l.starts_with("version = "))
            .expect("Línea de versión en Cargo.toml");

        let version = version_line
            .trim_start_matches("version = ")
            .trim_matches('"');

        assert!(
            historial_str.contains(&format!("Versión {}", version)),
            "El historial de cambios debe documentar la versión {} de Cargo.toml",
            version
        );
    }
}
