# Punch Installer for Windows
# Run with: .\install.ps1

Write-Host ""
Write-Host "👊 Installing Punch..." -ForegroundColor Cyan
Write-Host ""

# Check Rust
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Rust not found. Please install it from https://rustup.rs" -ForegroundColor Yellow
    Write-Host "Then re-run this script."
    Start-Process "https://rustup.rs"
    exit 1
}

# Build and install CLI
Write-Host "Building punch CLI (this takes ~2 minutes first time)..."
Set-Location core
cargo install --path . --quiet
Set-Location ..

Write-Host ""
Write-Host "✅ Punch installed!" -ForegroundColor Green
Write-Host ""
Write-Host "Try it:"
Write-Host "  punch generate         -> get a code"
Write-Host "  punch connect <code>   -> connect to a peer"
Write-Host ""
Write-Host "Note: Punch works best on WiFi." -ForegroundColor Yellow
Write-Host "Mobile/corporate networks may fall back to relay."
Write-Host ""
