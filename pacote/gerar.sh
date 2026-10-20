#!/usr/bin/env bash
# Gera os instaladores do FeatherCode em pacote/saida/, com os nomes fixos que o
# launcher procura na release (releases/latest/download/<nome>):
#   feathercode-linux-x86_64.tar.gz   Linux portátil (instalar.sh na pasta do usuário)
#   feathercode-linux-amd64.deb       Debian, Ubuntu, Mint…
#   FeatherCode-windows-x64.exe       Windows 10/11 (instalador por usuário, sem administrador)
#   SHA256SUMS
#
# Uso: pacote/gerar.sh [linux|windows ...]   (sem argumentos = todos)
# Precisa de: cargo, dpkg-deb; para Windows, o alvo x86_64-pc-windows-gnu,
# mingw-w64 e makensis.
set -euo pipefail

pacote="$(cd "$(dirname "$0")" && pwd)"
raiz="$(dirname "$pacote")"
alvo="${CARGO_TARGET_DIR:-$raiz/target}"
versao="$(sed -n 's/^version = "\(.*\)"/\1/p' "$raiz/Cargo.toml" | head -1)"
saida="$pacote/saida"
alvos=("$@")
[[ ${#alvos[@]} -eq 0 ]] && alvos=(linux windows)
rm -rf "$saida" && mkdir -p "$saida"
echo "FeatherCode $versao → $saida"

linux() {
  cargo build --release --manifest-path "$raiz/Cargo.toml"
  local bin="$alvo/release/feathercode"

  # .deb
  local d; d="$(mktemp -d)"
  install -Dm755 "$bin" "$d/usr/bin/feathercode"
  install -Dm644 "$pacote/feathercode.desktop" "$d/usr/share/applications/feathercode.desktop"
  for t in 16 24 32 48 64 128 256 512; do
    install -Dm644 "$pacote/icones/feathercode-$t.png" "$d/usr/share/icons/hicolor/${t}x${t}/apps/feathercode.png"
  done
  install -Dm644 "$pacote/icones/feathercode.svg" "$d/usr/share/icons/hicolor/scalable/apps/feathercode.svg"
  install -Dm644 "$raiz/LICENSE" "$d/usr/share/doc/feathercode/copyright"
  local tamanho; tamanho="$(du -sk "$d/usr" | cut -f1)"
  mkdir -p "$d/DEBIAN"
  cat > "$d/DEBIAN/control" <<CTL
Package: feathercode
Version: $versao
Section: devel
Priority: optional
Architecture: amd64
Installed-Size: $tamanho
Depends: libc6 (>= 2.39), libxkbcommon0, libwayland-client0 | libx11-6, libfontconfig1
Recommends: libvulkan1 | libgl1, git
Maintainer: FeatherWorks <https://github.com/feather-works>
Homepage: https://github.com/feather-works/feathercode
Description: Editor de código da suíte FeatherWorks
 Editor de código em Rust com interface nativa, terminal, Git e o
 Claude Code (ou o Codex e o Antigravity) no painel lateral.
CTL
  cat > "$d/DEBIAN/postinst" <<'SH'
#!/bin/sh
set -e
command -v update-desktop-database >/dev/null && update-desktop-database -q /usr/share/applications || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t /usr/share/icons/hicolor || true
SH
  cp "$d/DEBIAN/postinst" "$d/DEBIAN/postrm"
  chmod 755 "$d/DEBIAN/postinst" "$d/DEBIAN/postrm"
  dpkg-deb --root-owner-group --build "$d" "$saida/feathercode-linux-amd64.deb" >/dev/null
  rm -rf "$d"

  # .tar.gz portátil: "<app>-<versão>/instalar.sh", o mesmo que o launcher roda.
  local t; t="$(mktemp -d)/feathercode-$versao"
  mkdir -p "$t"
  cp "$bin" "$t/feathercode"
  cp "$pacote/feathercode.desktop" "$raiz/LICENSE" "$t/"
  cp -r "$pacote/icones" "$t/"
  cat > "$t/instalar.sh" <<'SH'
#!/bin/sh
# Instala o FeatherCode só para este usuário (~/.local), sem administrador.
set -e
cd "$(dirname "$0")"
mkdir -p ~/.local/bin ~/.local/share/applications
install -m755 feathercode ~/.local/bin/feathercode
sed "s#^Exec=feathercode#Exec=$HOME/.local/bin/feathercode#" feathercode.desktop > ~/.local/share/applications/feathercode.desktop
for t in 16 24 32 48 64 128 256 512; do
  mkdir -p ~/.local/share/icons/hicolor/${t}x${t}/apps
  cp icones/feathercode-$t.png ~/.local/share/icons/hicolor/${t}x${t}/apps/feathercode.png
done
update-desktop-database ~/.local/share/applications 2>/dev/null || true
echo "FeatherCode instalado. Procure por \"FeatherCode\" no menu de aplicativos."
SH
  chmod 755 "$t/instalar.sh"
  tar -C "$(dirname "$t")" -czf "$saida/feathercode-linux-x86_64.tar.gz" "feathercode-$versao"
  rm -rf "$(dirname "$t")"
}

windows() {
  cargo build --release --target x86_64-pc-windows-gnu --manifest-path "$raiz/Cargo.toml"
  local exe="$alvo/x86_64-pc-windows-gnu/release/feathercode.exe"
  x86_64-w64-mingw32-strip "$exe" 2>/dev/null || true
  (cd "$pacote" && makensis -V2 -DVERSAO="$versao" -DEXE="$exe" -DSAIDA="$saida/FeatherCode-windows-x64.exe" feathercode.nsi)
}

for a in "${alvos[@]}"; do
  echo "== $a"
  "$a"
done
(cd "$saida" && sha256sum -- * | grep -v SHA256SUMS > SHA256SUMS)
ls -lh "$saida"
