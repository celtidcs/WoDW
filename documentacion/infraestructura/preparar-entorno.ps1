<#
.SYNOPSIS
    Script idempotente de preparación de entorno para WoDW en Windows.
.DESCRIPTION
    Verifica herramientas del sistema (Rust, Cargo, Git), componentes de compilación
    (clippy, rustfmt, cargo-audit), compila el proyecto y valida las puertas de calidad.
#>

[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

Write-Host "=== [WoDW] Preparación de Entorno en Windows ===" -ForegroundColor Cyan

# 1. Verificar Git
if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Error "Git no está instalado o no se encuentra en el PATH."
}
Write-Host "[OK] Git detectado: $((git --version).Trim())" -ForegroundColor Green

# 2. Verificar Rust y Cargo
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "Cargo no está instalado. Instale Rust desde https://rustup.rs"
}
Write-Host "[OK] Rustc detectado: $((rustc --version).Trim())" -ForegroundColor Green
Write-Host "[OK] Cargo detectado: $((cargo --version).Trim())" -ForegroundColor Green

# 3. Verificar componentes rustfmt y clippy
Write-Host "Verificando componentes rustfmt y clippy..." -ForegroundColor Yellow
rustup component add rustfmt clippy

# 4. Verificar cargo-audit
if (-not (Get-Command cargo-audit -ErrorAction SilentlyContinue)) {
    Write-Host "Instalando cargo-audit para auditorías de seguridad..." -ForegroundColor Yellow
    cargo install cargo-audit --locked
} else {
    Write-Host "[OK] cargo-audit ya instalado." -ForegroundColor Green
}

# 5. Compilación del proyecto
Write-Host "Compilando el proyecto WoDW..." -ForegroundColor Yellow
cargo check
if ($LASTEXITCODE -ne 0) { Write-Error "La compilación falló." }

# 6. Comprobación final
Write-Host "Ejecutando pruebas del proyecto..." -ForegroundColor Yellow
cargo test --all-features
if ($LASTEXITCODE -ne 0) { Write-Error "Las pruebas fallaron." }

# 7. Configuración opcional (sin secretos): la aplicación funciona sin ella.
Write-Host "Configuración opcional: copie wodw.ejemplo.toml como wodw.toml junto al ejecutable." -ForegroundColor White

Write-Host "=== Entorno WoDW preparado exitosamente en Windows ===" -ForegroundColor Cyan
Write-Host "Para arrancar la aplicación: cargo build --release (compila wodw.exe y wodw-worker.exe) y después target\release\wodw.exe" -ForegroundColor White
