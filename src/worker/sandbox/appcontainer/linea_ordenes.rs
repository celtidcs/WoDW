//! Línea de órdenes del Worker, citada para que el hijo lea exactamente los
//! mismos argumentos.

use std::path::Path;

/// Línea de órdenes con cada elemento citado según las reglas del entorno de
/// ejecución de C de Microsoft (las que usa `std::env::args` en el hijo).
pub fn linea_de_ordenes(ejecutable: &Path, argumentos: &[&str]) -> String {
    std::iter::once(citar(&ejecutable.to_string_lossy()))
        .chain(argumentos.iter().map(|a| citar(a)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Cita un argumento: comillas dobles alrededor, comillas internas escapadas y
/// las barras invertidas que preceden a una comilla, duplicadas.
fn citar(argumento: &str) -> String {
    if !argumento.is_empty() && !argumento.contains([' ', '\t', '\n', '\r', '"']) {
        return argumento.to_string();
    }
    let mut citado = String::from('"');
    let mut barras = 0usize;
    for c in argumento.chars() {
        match c {
            '\\' => barras += 1,
            '"' => {
                citado.push_str(&"\\".repeat(barras * 2 + 1));
                citado.push('"');
                barras = 0;
            }
            _ => {
                citado.push_str(&"\\".repeat(barras));
                citado.push(c);
                barras = 0;
            }
        }
    }
    citado.push_str(&"\\".repeat(barras * 2));
    citado.push('"');
    citado
}

#[cfg(test)]
mod tests {
    use super::super::ancho;
    use super::*;
    use std::ffi::OsStr;
    use windows_sys::Win32::Foundation::LocalFree;

    /// La cita se comprueba contra el intérprete real de Windows.
    #[test]
    fn citas_sobreviven_a_command_line_to_argv() {
        use windows_sys::Win32::UI::Shell::CommandLineToArgvW;
        let argumentos = [
            "simple",
            "con espacio",
            "comilla\"dentro",
            "barra\\\"x",
            "fin\\",
            "",
            "a\nb",
        ];
        let linea = ancho(OsStr::new(&linea_de_ordenes(
            Path::new("C:\\x y\\w.exe"),
            &argumentos,
        )));
        let mut n = 0i32;
        // SAFETY: cadena terminada en nulo; el resultado se libera con LocalFree.
        let vector = unsafe { CommandLineToArgvW(linea.as_ptr(), &mut n) };
        assert!(!vector.is_null());
        let leidos: Vec<String> = (0..n as usize)
            .map(|i| {
                // SAFETY: `n` punteros a cadenas anchas terminadas en nulo.
                let p = unsafe { *vector.add(i) };
                let largo = (0..).take_while(|&j| unsafe { *p.add(j) } != 0).count();
                String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, largo) })
            })
            .collect();
        // SAFETY: memoria reservada por CommandLineToArgvW.
        unsafe { LocalFree(vector as _) };
        assert_eq!(leidos[0], "C:\\x y\\w.exe");
        assert_eq!(&leidos[1..], &argumentos);
    }
}
