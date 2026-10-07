#!/usr/bin/env bash
# Script idempotente de preparación de entorno para WoDW en Linux / Tails OS.
# Verifica herramientas del sistema (Rust, Cargo, Git, paquetes del sistema),
# instala componentes de compilación y ejecuta la verificación inicial.

set -euo pipefail

echo "=== [WoDW] Preparación de Entorno en Linux / Tails OS ==="

# 1. Verificar Git
if ! command -v git &> /dev/null; then
    echo "[ERROR] Git no está instalado. Ejecute: sudo apt update && sudo apt install -y git"
    exit 1
fi
echo "[OK] Git detectado: $(git --version)"

# 2. Verificar Rust y Cargo
if ! command -v cargo &> /dev/null; then
    echo "[ERROR] Rust/Cargo no detectado. Instale Rust desde https://rustup.rs"
    exit 1
fi
echo "[OK] Rustc detectado: $(rustc --version)"
echo "[OK] Cargo detectado: $(cargo --version)"

# 3. Componentes rustfmt y clippy
echo "Verificando componentes rustfmt y clippy..."
rustup component add rustfmt clippy || true

# 4. Paquetes del sistema (solo Debian/Ubuntu/Tails con apt).
#    native-tls necesita OpenSSL; la interfaz egui necesita las bibliotecas de X11/Wayland
#    (lista tomada del README de egui; no comprobada todavía en esta máquina).
PAQUETES="build-essential pkg-config libssl-dev libasound2-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev"
if command -v dpkg-query &> /dev/null; then
    FALTAN=""
    for paquete in $PAQUETES; do
        dpkg-query -W -f='${Status}' "$paquete" 2>/dev/null | grep -q "install ok installed" || FALTAN="$FALTAN $paquete"
    done
    if [ -n "$FALTAN" ]; then
        echo "Instalando paquetes del sistema:$FALTAN"
        sudo apt-get update && sudo apt-get install -y $FALTAN
    else
        echo "[OK] Paquetes del sistema presentes."
    fi
else
    echo "[AVISO] Sin apt: instale manualmente equivalentes de: $PAQUETES"
fi

# 5. Compilación y comprobación del proyecto
echo "Compilando proyecto WoDW..."
cargo check

echo "Ejecutando pruebas del proyecto..."
cargo test

echo "=== Entorno WoDW preparado exitosamente en Linux ==="
echo "Configuración opcional: copie wodw.ejemplo.toml como wodw.toml junto al ejecutable."
echo "Para arrancar la aplicación: cargo build --release (compila wodw y wodw-worker) y después ./target/release/wodw"
