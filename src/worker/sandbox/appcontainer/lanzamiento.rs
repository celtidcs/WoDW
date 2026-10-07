//! Creación del proceso del Worker dentro del AppContainer y del Job Object.

use super::{ancho, error_windows, linea_de_ordenes, Descriptor, SidAppContainer};
use crate::error::Resultado;
use crate::worker::sandbox::windows::JobObjectGuardian;
use std::ffi::{c_void, OsStr};
use std::fs::File;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{
    SetHandleInformation, BOOL, HANDLE, HANDLE_FLAG_INHERIT, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::{SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
    InitializeProcThreadAttributeList, ResumeThread, TerminateProcess, UpdateProcThreadAttribute,
    WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED, EXTENDED_STARTUPINFO_PRESENT,
    LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
    PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, STARTF_USESTDHANDLES, STARTUPINFOEXW,
};

/// Atributos del proceso: capacidades de seguridad y lista de descriptores heredables.
const ATRIBUTOS_PROCESO: u32 = 2;
/// Código de salida que `TerminateProcess` asigna al Worker destruido.
const CODIGO_DESTRUIDO: u32 = 1;

/// Proceso en AppContainer con su entrada y salida estándar.
pub struct ProcesoAppContainer {
    proceso: Descriptor,
    entrada: Option<File>,
    salida: Option<File>,
}

impl ProcesoAppContainer {
    /// Toma la tubería de la entrada estándar del hijo (para escribirle).
    pub fn tomar_entrada(&mut self) -> Option<File> {
        self.entrada.take()
    }

    /// Toma la tubería de la salida estándar del hijo (para leerle).
    pub fn tomar_salida(&mut self) -> Option<File> {
        self.salida.take()
    }

    /// Descriptor del proceso.
    pub fn manejador(&self) -> HANDLE {
        self.proceso.0
    }

    /// Espera como mucho `milisegundos` a que termine y devuelve su código.
    pub fn esperar(&self, milisegundos: u32) -> Option<u32> {
        // SAFETY: descriptor de proceso válido mientras viva `self`.
        if unsafe { WaitForSingleObject(self.proceso.0, milisegundos) } != WAIT_OBJECT_0 {
            return None;
        }
        let mut codigo = 0u32;
        // SAFETY: descriptor válido y puntero de salida válido.
        (unsafe { GetExitCodeProcess(self.proceso.0, &mut codigo) } != 0).then_some(codigo)
    }

    /// Destruye el proceso (sin efecto si ya terminó).
    pub fn destruir(&self) {
        // SAFETY: descriptor de proceso válido.
        unsafe { TerminateProcess(self.proceso.0, CODIGO_DESTRUIDO) };
    }
}

impl Drop for ProcesoAppContainer {
    fn drop(&mut self) {
        self.destruir();
    }
}

/// Extremos de las tuberías estándar que hereda el hijo.
struct EstandarHijo<'a> {
    entrada: &'a Descriptor,
    salida: &'a Descriptor,
}

/// Lanza `ejecutable argumentos…` en el AppContainer, dentro de `job`, con
/// tuberías propias para la entrada y la salida estándar. La salida de
/// errores no se conecta.
///
/// # Errors
/// [`crate::error::ErrorApp::Sandbox`] o [`crate::error::ErrorApp::Proceso`]
/// con el código de Windows.
pub fn lanzar(
    ejecutable: &Path,
    argumentos: &[&str],
    job: &JobObjectGuardian,
) -> Resultado<ProcesoAppContainer> {
    let (entrada_hijo, entrada_padre) = tuberia()?;
    let (salida_padre, salida_hijo) = tuberia()?;
    no_heredable(&entrada_padre)?;
    no_heredable(&salida_padre)?;
    let estandar = EstandarHijo {
        entrada: &entrada_hijo,
        salida: &salida_hijo,
    };
    let info = crear_suspendido(ejecutable, argumentos, &estandar)?;
    let hilo = Descriptor(info.hThread);
    let lanzado = ProcesoAppContainer {
        proceso: Descriptor(info.hProcess),
        // SAFETY: descriptores de tubería propios, cedidos a `File`.
        entrada: Some(unsafe { File::from_raw_handle(entrada_padre.ceder() as _) }),
        // SAFETY: ídem.
        salida: Some(unsafe { File::from_raw_handle(salida_padre.ceder() as _) }),
    };
    job.asignar_proceso(lanzado.manejador())?;
    // SAFETY: hilo principal suspendido del proceso recién creado.
    if unsafe { ResumeThread(hilo.0) } == u32::MAX {
        return Err(error_windows("ResumeThread"));
    }
    Ok(lanzado)
}

/// Crea el proceso suspendido en el AppContainer sin capacidades, heredando
/// solo los dos extremos de `estandar`.
fn crear_suspendido(
    ejecutable: &Path,
    argumentos: &[&str],
    estandar: &EstandarHijo<'_>,
) -> Resultado<PROCESS_INFORMATION> {
    let sid = SidAppContainer::derivar()?;
    let heredables = [estandar.entrada.0, estandar.salida.0];
    let mut capacidades = SECURITY_CAPABILITIES {
        AppContainerSid: sid.0,
        Capabilities: null_mut(),
        CapabilityCount: 0,
        Reserved: 0,
    };
    let mut lista = ListaAtributos::nueva(ATRIBUTOS_PROCESO)?;
    lista.fijar(
        PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
        &mut capacidades,
    )?;
    lista.fijar_lista(PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize, &heredables)?;
    let mut linea = ancho(OsStr::new(&linea_de_ordenes(ejecutable, argumentos)));
    let aplicacion = ancho(ejecutable.as_os_str());
    // SAFETY: estructura POD cuyo valor cero es válido.
    let mut inicio: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    inicio.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    inicio.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    inicio.StartupInfo.hStdInput = estandar.entrada.0;
    inicio.StartupInfo.hStdOutput = estandar.salida.0;
    inicio.lpAttributeList = lista.puntero();
    // SAFETY: estructura POD de salida.
    let mut info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: cadenas terminadas en nulo; línea de órdenes mutable propia; la
    // lista de atributos, las capacidades y los descriptores heredables viven
    // hasta el retorno.
    let creado = unsafe {
        CreateProcessW(
            aplicacion.as_ptr(),
            linea.as_mut_ptr(),
            null(),
            null(),
            BOOL::from(true),
            EXTENDED_STARTUPINFO_PRESENT | CREATE_SUSPENDED | CREATE_NO_WINDOW,
            null(),
            null(),
            &inicio.StartupInfo,
            &mut info,
        )
    };
    if creado == 0 {
        return Err(error_windows("CreateProcessW en AppContainer"));
    }
    Ok(info)
}

/// Lista de atributos de creación de proceso.
struct ListaAtributos {
    memoria: Vec<usize>,
}

impl ListaAtributos {
    fn nueva(atributos: u32) -> Resultado<Self> {
        let mut tamano = 0usize;
        // SAFETY: consulta de tamaño con lista nula (falla a propósito).
        unsafe { InitializeProcThreadAttributeList(null_mut(), atributos, 0, &mut tamano) };
        let palabras = tamano.div_ceil(std::mem::size_of::<usize>());
        let mut lista = Self {
            memoria: vec![0; palabras],
        };
        // SAFETY: búfer alineado a `usize` del tamaño pedido por Windows.
        if unsafe { InitializeProcThreadAttributeList(lista.puntero(), atributos, 0, &mut tamano) }
            == 0
        {
            return Err(error_windows("InitializeProcThreadAttributeList"));
        }
        Ok(lista)
    }

    fn puntero(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.memoria.as_mut_ptr().cast()
    }

    fn fijar<T>(&mut self, atributo: usize, valor: &mut T) -> Resultado<()> {
        self.fijar_crudo(atributo, (valor as *mut T).cast(), std::mem::size_of::<T>())
    }

    fn fijar_lista(&mut self, atributo: usize, valores: &[HANDLE]) -> Resultado<()> {
        self.fijar_crudo(
            atributo,
            valores.as_ptr() as *mut c_void,
            std::mem::size_of_val(valores),
        )
    }

    fn fijar_crudo(&mut self, atributo: usize, valor: *mut c_void, tamano: usize) -> Resultado<()> {
        // SAFETY: lista inicializada; `valor` vive hasta CreateProcessW (lo
        // garantiza el llamador manteniendo el préstamo en su marco).
        let fijado = unsafe {
            UpdateProcThreadAttribute(
                self.puntero(),
                0,
                atributo,
                valor,
                tamano,
                null_mut(),
                null(),
            )
        };
        if fijado == 0 {
            return Err(error_windows("UpdateProcThreadAttribute"));
        }
        Ok(())
    }
}

impl Drop for ListaAtributos {
    fn drop(&mut self) {
        // SAFETY: lista inicializada en `nueva`.
        unsafe { DeleteProcThreadAttributeList(self.puntero()) };
    }
}

/// Crea una tubería anónima con ambos extremos heredables: (lectura, escritura).
fn tuberia() -> Resultado<(Descriptor, Descriptor)> {
    let atributos = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: BOOL::from(true),
    };
    let (mut lectura, mut escritura): (HANDLE, HANDLE) = (0, 0);
    // SAFETY: punteros de salida y atributos válidos.
    if unsafe { CreatePipe(&mut lectura, &mut escritura, &atributos, 0) } == 0 {
        return Err(error_windows("CreatePipe"));
    }
    Ok((Descriptor(lectura), Descriptor(escritura)))
}

/// Impide que el hijo herede un extremo que es del padre.
fn no_heredable(descriptor: &Descriptor) -> Resultado<()> {
    // SAFETY: descriptor propio válido.
    if unsafe { SetHandleInformation(descriptor.0, HANDLE_FLAG_INHERIT, 0) } == 0 {
        return Err(error_windows("SetHandleInformation"));
    }
    Ok(())
}
