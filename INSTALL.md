# Install Dutchman Electronics 0.1.0

We need [Dutchman 0.4.0 or later](https://github.com/dutchman-cad/dutchman/releases). We install this module separately.

Download the archive for the computer and its matching `.sha256` file from the [0.1.0 release](https://github.com/dutchman-cad/dutchman-electronics/releases/tag/v0.1.0). Each archive contains one folder with the program, README.md, INSTALL.md and the licenses. We ship unsigned downloads; the macOS program has only an ad-hoc signature.

## Windows

1. Download `dutchman-electronics-0.1.0-x86_64-pc-windows-msvc.zip` and its `.sha256` file. In PowerShell, open the download folder and compare these two hashes. They must match; stop if they differ.

   ```powershell
   (Get-FileHash .\dutchman-electronics-0.1.0-x86_64-pc-windows-msvc.zip -Algorithm SHA256).Hash
   Get-Content .\dutchman-electronics-0.1.0-x86_64-pc-windows-msvc.zip.sha256
   ```

2. Extract the ZIP. Open PowerShell inside the extracted folder containing `dutchman-electronics.exe`. Put the program directly in `%LOCALAPPDATA%\Dutchman\modules\`, creating the folder if needed:

   ```powershell
   $modules = Join-Path $env:LOCALAPPDATA 'Dutchman\modules'
   New-Item -ItemType Directory -Force -Path $modules | Out-Null
   Move-Item -Force .\dutchman-electronics.exe $modules
   & "$modules\dutchman-electronics.exe" --version
   ```

## macOS

1. Download `dutchman-electronics-0.1.0-macos-universal.tar.gz` (Apple silicon or Intel) and its `.sha256` file. We also offer `aarch64-apple-darwin.tar.gz` for Apple silicon and `x86_64-apple-darwin.tar.gz` for Intel, with the same `dutchman-electronics-0.1.0-` prefix. In Terminal, open the download folder and check the archive before extracting it:

   ```sh
   shasum -a 256 -c dutchman-electronics-0.1.0-macos-universal.tar.gz.sha256
   ```

   The result must say `OK`; stop if it fails. Substitute the chosen archive name if using a single-processor download.

2. Extract the archive. In Terminal, open the extracted folder containing `dutchman-electronics`. Put the program directly in `~/Library/Application Support/Dutchman/modules/`, creating the folder if needed, then remove quarantine from the copied file:

   ```sh
   mkdir -p "$HOME/Library/Application Support/Dutchman/modules"
   mv ./dutchman-electronics "$HOME/Library/Application Support/Dutchman/modules/"
   xattr -d com.apple.quarantine "$HOME/Library/Application Support/Dutchman/modules/dutchman-electronics"
   "$HOME/Library/Application Support/Dutchman/modules/dutchman-electronics" --version
   ```

   If `xattr` says the quarantine attribute does not exist, no removal is needed.

## Linux

1. Download `dutchman-electronics-0.1.0-x86_64-unknown-linux-gnu.tar.gz` for an Intel/AMD 64-bit computer, or `dutchman-electronics-0.1.0-aarch64-unknown-linux-gnu.tar.gz` for ARM64, and its `.sha256` file. In a terminal, open the download folder and check before extracting:

   ```sh
   sha256sum -c dutchman-electronics-0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256
   ```

   The result must say `OK`; stop if it fails. Substitute the ARM64 archive name if needed.

2. Extract the archive. In a terminal, open the extracted folder containing `dutchman-electronics`. Put the program directly in `$XDG_DATA_HOME/dutchman/modules/` (default `~/.local/share/dutchman/modules/`), creating the folder if needed:

   ```sh
   modules="${XDG_DATA_HOME:-$HOME/.local/share}/dutchman/modules"
   mkdir -p "$modules"
   mv ./dutchman-electronics "$modules/"
   chmod +x "$modules/dutchman-electronics"
   "$modules/dutchman-electronics" --version
   ```

   `chmod +x` ensures the program can run; it changes nothing if that permission is already set. This location works with the tarball, AppImage and Arch package.

## Check the installation

The full-path commands above run `dutchman-electronics --version`; we expect version `0.1.0`. The modules folder does not need to be on PATH. Restart Dutchman and check Settings to see where it found the module. This folder survives Dutchman updates.

Run `dutchman formats` in a terminal (or use the full path to the Dutchman command). It lists the electronics formats as “placed parts and flat board layers, through the electronics module (`dutchman-electronics`, installed separately)”. This is a list of supported formats, not an installation check. Open a `.PcbDoc` or `.kicad_pcb` in Dutchman to check conversion.

## Alternatives: beside the program or on PATH

We recommend the per-user modules folder above. Dutchman searches beside its running program first (with symbolic links resolved), then the per-user modules folder, then PATH. An older copy beside the program takes priority; Settings shows the selected location.

We can also put the program beside a tarball installation's `dutchman`, or in a folder on PATH. On Windows, the MSI puts both the app and command in `%LOCALAPPDATA%\Programs\Dutchman\` and adds it to the user PATH. On Linux, an AppImage is read-only, so use a PATH folder such as `~/.local/bin`; log out and back in if that folder was just created. An Arch installation can use `/usr/bin` or any PATH folder. A macOS app opened from Finder does not see the Terminal PATH, so installing in `/usr/local/bin` alone does not make the module available to that app.
