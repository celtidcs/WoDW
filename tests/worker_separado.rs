//! Barreras del Worker en su propio ejecutable (CA-21-7 a CA-21-10): no carga
//! nada gráfico, funciona en cualquier escritorio, tiene Win32k cortado y el
//! Maestro solo lanza el que tiene al lado.

use std::path::{Path, PathBuf};
use wodw::maestro::proceso_worker::{localizar_worker, ruta_worker_junto_a};

/// CA-21-10: el Worker se busca solo en la carpeta del Maestro.
#[test]
fn el_worker_se_busca_junto_al_maestro() {
    let maestro = Path::new("carpeta").join(format!("wodw{}", std::env::consts::EXE_SUFFIX));
    assert_eq!(
        ruta_worker_junto_a(&maestro),
        Path::new("carpeta").join(format!("wodw-worker{}", std::env::consts::EXE_SUFFIX))
    );
    let real = localizar_worker(Path::new(env!("CARGO_BIN_EXE_wodw"))).expect("junto a wodw");
    assert_eq!(real, PathBuf::from(env!("CARGO_BIN_EXE_wodw-worker")));
    let falta = localizar_worker(&std::env::temp_dir().join("no-existe").join("wodw"))
        .expect_err("sin Worker al lado");
    assert!(falta.to_string().contains("wodw-worker"), "{falta}");
}

#[cfg(windows)]
mod windows {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use wodw::configuracion::ConfiguracionWorker;
    use wodw::ipc::mensajes::{OrdenWorker, RespuestaWorker};
    use wodw::maestro::ProcesadorSubworker;
    use wodw::worker::sandbox::appcontainer;
    use wodw::worker::sandbox::windows::JobObjectGuardian;
    use wodw::worker::{ARGUMENTO_MODO_WORKER, ARGUMENTO_PARAMETROS_WORKER};

    /// Bibliotecas de interfaz que el Worker no debe cargar.
    const BIBLIOTECAS_GRAFICAS: &[&str] = &[
        "user32.dll",
        "gdi32.dll",
        "opengl32.dll",
        "dxgi.dll",
        "dwmapi.dll",
        "uxtheme.dll",
        "imm32.dll",
        "shell32.dll",
        "uiautomationcore.dll",
        "comctl32.dll",
        "comdlg32.dll",
    ];

    /// DLL importadas por un ejecutable PE32+ (directorio de importaciones).
    fn importaciones(ruta: &str) -> Vec<String> {
        let b = std::fs::read(ruta).expect("ejecutable");
        let u16_en = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]) as usize;
        let u32_en = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().expect("u32")) as usize;
        let pe = u32_en(0x3C);
        let (secciones, opcional) = (u16_en(pe + 6), pe + 24);
        let tabla = opcional + u16_en(pe + 20);
        let a_posicion = |rva: usize| {
            (0..secciones)
                .map(|i| tabla + 40 * i)
                .find_map(|s| {
                    let (va, tam, datos) = (u32_en(s + 12), u32_en(s + 8), u32_en(s + 20));
                    (va <= rva && rva < va + tam.max(1)).then(|| rva - va + datos)
                })
                .expect("RVA dentro de una sección")
        };
        // Directorio de datos n.º 1 (importaciones) en la cabecera opcional PE32+.
        let mut o = a_posicion(u32_en(opcional + 112 + 8));
        let mut nombres = Vec::new();
        while u32_en(o + 12) != 0 {
            let n = a_posicion(u32_en(o + 12));
            let fin = b[n..].iter().position(|&c| c == 0).expect("nombre");
            nombres.push(String::from_utf8_lossy(&b[n..n + fin]).to_lowercase());
            o += 20;
        }
        nombres
    }

    /// CA-21-7: el Worker no importa ninguna biblioteca de interfaz.
    #[test]
    fn el_worker_no_carga_nada_grafico() {
        // Calibración: la interfaz sí las importa.
        let interfaz = importaciones(env!("CARGO_BIN_EXE_wodw"));
        assert!(interfaz.iter().any(|d| d == "user32.dll"), "{interfaz:?}");
        let worker = importaciones(env!("CARGO_BIN_EXE_wodw-worker"));
        let graficas: Vec<_> = worker
            .iter()
            .filter(|d| BIBLIOTECAS_GRAFICAS.contains(&d.as_str()))
            .collect();
        assert!(graficas.is_empty(), "el Worker importa {graficas:?}");
    }

    /// Trabajo real del Worker (HTML) en el escritorio en el que corre esta
    /// prueba. Lo usa también la prueba del escritorio nuevo.
    #[tokio::test]
    async fn trabajo_real_del_worker_en_este_escritorio() {
        let procesador = ProcesadorSubworker::nuevo(
            PathBuf::from(env!("CARGO_BIN_EXE_wodw-worker")),
            ConfiguracionWorker::default(),
        )
        .expect("procesador");
        let r = procesador
            .procesar(&OrdenWorker::ProcesarHtml {
                id_tarea: 1,
                url_origen: "http://a.onion/".into(),
                contenido_html: b"<title>Hola</title><p>mundo</p>".to_vec(),
            })
            .await
            .expect("el Worker responde");
        assert!(matches!(r, RespuestaWorker::HtmlProcesado { .. }), "{r:?}");
    }

    /// Ejecuta solo `prueba` de este mismo archivo en el escritorio `escritorio`
    /// y devuelve su código de salida.
    fn ejecutar_prueba_en(escritorio: &str, prueba: &str) -> u32 {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            CreateProcessW, GetExitCodeProcess, WaitForSingleObject, CREATE_NO_WINDOW, INFINITE,
            PROCESS_INFORMATION, STARTUPINFOW,
        };
        let ancho = |t: &str| -> Vec<u16> { OsStr::new(t).encode_wide().chain([0]).collect() };
        let exe = std::env::current_exe().expect("prueba");
        let mut linea = ancho(&format!(
            "\"{}\" {prueba} --exact --test-threads=1",
            exe.display()
        ));
        let mut escritorio = ancho(escritorio);
        // SAFETY: estructuras POD cuyo valor cero es válido.
        let mut inicio: STARTUPINFOW = unsafe { std::mem::zeroed() };
        inicio.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        inicio.lpDesktop = escritorio.as_mut_ptr();
        // SAFETY: ídem.
        let mut info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: cadenas anchas terminadas en nulo que viven durante la llamada.
        let creado = unsafe {
            CreateProcessW(
                std::ptr::null(),
                linea.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                CREATE_NO_WINDOW,
                std::ptr::null(),
                std::ptr::null(),
                &inicio,
                &mut info,
            )
        };
        assert_ne!(creado, 0, "no se pudo lanzar la prueba");
        let mut codigo = 0u32;
        // SAFETY: descriptores devueltos por CreateProcessW.
        unsafe {
            WaitForSingleObject(info.hProcess, INFINITE);
            GetExitCodeProcess(info.hProcess, &mut codigo);
            CloseHandle(info.hProcess);
            CloseHandle(info.hThread);
        }
        codigo
    }

    /// CA-21-8: en un escritorio nuevo (sin los permisos que Windows da a las
    /// aplicaciones aisladas en el escritorio normal), el Worker funciona. Es la
    /// situación en la que la 0.2.0 moría con 0xc0000142.
    #[test]
    fn el_worker_funciona_en_un_escritorio_nuevo() {
        use windows_sys::Win32::System::StationsAndDesktops::{CloseDesktop, CreateDesktopW};
        /// Todos los derechos sobre el escritorio creado.
        const DERECHOS_ESCRITORIO: u32 = 0x01FF;
        let nombre = format!("WoDWPrueba{}", std::process::id());
        let ancho: Vec<u16> = OsStr::new(&nombre).encode_wide().chain([0]).collect();
        // SAFETY: nombre terminado en nulo; sin dispositivo ni modo; seguridad por defecto.
        let escritorio = unsafe {
            CreateDesktopW(
                ancho.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                DERECHOS_ESCRITORIO,
                std::ptr::null(),
            )
        };
        assert_ne!(escritorio, 0, "no se pudo crear el escritorio de prueba");
        let prueba = "windows::trabajo_real_del_worker_en_este_escritorio";
        let normal = ejecutar_prueba_en("WinSta0\\Default", prueba);
        let nuevo = ejecutar_prueba_en(&format!("WinSta0\\{nombre}"), prueba);
        // SAFETY: escritorio creado arriba.
        unsafe { CloseDesktop(escritorio) };
        assert_eq!(normal, 0, "control: en el escritorio normal debe funcionar");
        assert_eq!(nuevo, 0, "en un escritorio nuevo el Worker falla");
    }

    /// CA-21-9: el Worker se encierra con Win32k cortado.
    #[test]
    fn el_worker_tiene_win32k_cortado() {
        use windows_sys::Win32::System::Threading::{
            GetProcessMitigationPolicy, ProcessSystemCallDisablePolicy,
        };
        let ejecutable = PathBuf::from(env!("CARGO_BIN_EXE_wodw-worker"));
        appcontainer::preparar(&ejecutable).expect("AppContainer");
        let parametros = toml::to_string(&ConfiguracionWorker::default()).expect("parámetros");
        let job =
            JobObjectGuardian::nuevo(ConfiguracionWorker::default().memoria_maxima_worker_bytes)
                .expect("job");
        let proceso = appcontainer::lanzar(
            &ejecutable,
            &[
                ARGUMENTO_MODO_WORKER,
                ARGUMENTO_PARAMETROS_WORKER,
                &parametros,
            ],
            &job,
        )
        .expect("Worker lanzado");
        // El Worker aplica sus políticas al arrancar, antes de leer órdenes.
        let limite = Instant::now() + Duration::from_secs(10);
        let mut activa = false;
        while !activa && Instant::now() < limite {
            let mut banderas = 0u32;
            // SAFETY: descriptor del proceso con todos los derechos; búfer de 4 bytes.
            let ok = unsafe {
                GetProcessMitigationPolicy(
                    proceso.manejador(),
                    ProcessSystemCallDisablePolicy,
                    (&mut banderas as *mut u32).cast(),
                    std::mem::size_of::<u32>(),
                )
            };
            activa = ok != 0 && banderas & 1 != 0;
            std::thread::sleep(Duration::from_millis(50));
        }
        proceso.destruir();
        assert!(
            activa,
            "DisallowWin32kSystemCalls no está activo en el Worker"
        );
    }
}
