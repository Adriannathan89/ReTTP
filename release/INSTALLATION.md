# Installation

## Download a release

Open the repository's **Releases** page, select `v0.1.1`, expand **Assets**, and
download the archive matching the operating system and CPU architecture.
Rust is not required to run a downloaded UTest binary.

## Verify the archive

Download `SHA256SUMS` from the same release. On Linux:

```bash
sha256sum --check SHA256SUMS --ignore-missing
```

On macOS:

```bash
grep 'aarch64-apple-darwin' SHA256SUMS | shasum -a 256 --check
```

On Windows PowerShell, verify the matching entry automatically:

```powershell
$asset = "utest-v0.1.1-x86_64-pc-windows-msvc.zip"
$line = Get-Content .\SHA256SUMS | Where-Object { $_ -match "  $([regex]::Escape($asset))$" }
if (-not $line) { throw "checksum entry not found" }
$expected = ($line -split '\s+')[0].ToUpperInvariant()
$actual = (Get-FileHash ".\$asset" -Algorithm SHA256).Hash
if ($actual -ne $expected) { throw "checksum mismatch" }
```

Do not install or execute an archive whose checksum does not match.

## Linux x86-64

```bash
tar -xzf utest-v0.1.1-x86_64-unknown-linux-gnu.tar.gz
chmod +x utest
./utest --version
sudo install -m 0755 utest /usr/local/bin/utest
```

The Linux build targets glibc-based x86-64 distributions. It is not a musl
static build.

## macOS Apple Silicon

```bash
tar -xzf utest-v0.1.1-aarch64-apple-darwin.tar.gz
chmod +x utest
./utest --version
sudo install -m 0755 utest /usr/local/bin/utest
```

Version 0.1.1 is not code-signed or notarized. macOS may require explicit
approval in **System Settings > Privacy & Security** before the first run.

## Windows x86-64

Extract `utest.exe` from the ZIP archive and verify it in PowerShell:

```powershell
.\utest.exe --version
.\utest.exe --help
```

Move the executable to a stable directory such as `C:\Tools\utest`, then add
that directory to the user or system `PATH` to invoke `utest` globally.

Version 0.1.1 is not Authenticode-signed, so Microsoft Defender SmartScreen may
show an unknown-publisher warning.

## Build from source

Install the stable Rust toolchain, clone the repository, and run:

```bash
cargo build --release --locked --package utest-cli
./target/release/utest --version
```

Building from source uses the exact dependency versions in `Cargo.lock`.
