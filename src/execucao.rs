//! Descobre como rodar o projeto da pasta (botão Rodar, F5).
use std::path::Path;

/// Comando que roda o projeto da pasta, pelo que existe nela; o arquivo aberto decide quando a pasta não diz.
pub fn comando_do_projeto(raiz: &Path, arquivo: Option<&Path>) -> Option<String> {
    if let Ok(cargo) = std::fs::read_to_string(raiz.join("Cargo.toml")) {
        if cargo.contains("[workspace]") {
            // Workspace da suíte: roda o crate da janela.
            let gui = std::fs::read_dir(raiz.join("crates"))
                .ok()?
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .find(|n| n.ends_with("-gui"));
            return Some(gui.map_or("cargo run".into(), |g| format!("cargo run -p {g}")));
        }
        return Some("cargo run".into());
    }
    if raiz.join("package.json").is_file() {
        return Some("npm start".into());
    }
    for py in ["main.py", "app.py", "src/main.py"] {
        if raiz.join(py).is_file() {
            return Some(format!("python3 {py}"));
        }
    }
    let arquivo = arquivo?;
    let relativo = arquivo
        .strip_prefix(raiz)
        .unwrap_or(arquivo)
        .display()
        .to_string();
    match arquivo.extension()?.to_str()? {
        "py" => Some(format!("python3 \"{relativo}\"")),
        "sh" => Some(format!("sh \"{relativo}\"")),
        "js" => Some(format!("node \"{relativo}\"")),
        "html" => Some(format!("xdg-open \"{relativo}\"")),
        _ => None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn descobre_o_comando_pela_pasta() {
        let raiz = std::env::temp_dir().join(format!("feathercode-rodar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(raiz.join("crates/app-gui")).unwrap();
        assert_eq!(
            comando_do_projeto(&raiz, Some(&raiz.join("x.py"))).as_deref(),
            Some("python3 \"x.py\"")
        );
        assert_eq!(comando_do_projeto(&raiz, None), None);
        std::fs::write(
            raiz.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        assert_eq!(
            comando_do_projeto(&raiz, None).as_deref(),
            Some("cargo run -p app-gui")
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
