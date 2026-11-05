//! Problemas: erros e avisos do `cargo check`, para a lista embaixo do editor e o sublinhado.
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Problema {
    pub caminho: PathBuf,
    /// Linha e coluna do começo, contando de 1 (como o compilador).
    pub linha: usize,
    pub coluna: usize,
    /// Coluna do fim, se o trecho termina na mesma linha.
    pub fim: Option<usize>,
    pub erro: bool,
    pub mensagem: String,
}

impl Problema {
    /// `src/main.rs:42:5`, relativo à pasta.
    pub fn onde(&self, raiz: &Path) -> String {
        let rel = self.caminho.strip_prefix(raiz).unwrap_or(&self.caminho);
        format!("{}:{}:{}", rel.display(), self.linha, self.coluna)
    }

    /// O sublinhado no editor: colunas (em letras) viram bytes da linha.
    pub fn sublinhado(&self, texto_da_linha: &str) -> pelicano::Warning {
        let byte = |coluna: usize| {
            texto_da_linha
                .char_indices()
                .nth(coluna.saturating_sub(1))
                .map_or(texto_da_linha.len(), |(b, _)| b)
        };
        let ini = byte(self.coluna);
        let fim = self.fim.map_or(texto_da_linha.len(), byte);
        // Trecho vazio (o compilador às vezes aponta um ponto só): pega a letra do começo.
        let fim = if fim > ini {
            fim
        } else {
            texto_da_linha[ini..].chars().next().map_or(ini, |c| ini + c.len_utf8())
        };
        pelicano::Warning {
            line: self.linha - 1,
            range: ini..fim,
            tip: self.mensagem.clone(),
        }
    }
}

/// Só projeto Rust tem a lista: é de onde vêm os problemas.
pub fn disponivel(raiz: &Path) -> bool {
    raiz.join("Cargo.toml").is_file()
}

/// Roda `cargo check` na pasta e lê os problemas dos arquivos dela (os das dependências ficam de fora).
pub async fn checar(raiz: PathBuf) -> Result<Vec<Problema>, String> {
    let saida = crate::sistema::comando("cargo")
        .args(["check", "--message-format=json", "--quiet"])
        .current_dir(&raiz)
        .output()
        .await
        .map_err(|e| format!("Não deu para rodar o cargo: {e}"))?;
    let lista = ler(&String::from_utf8_lossy(&saida.stdout), &raiz);
    // Falhou sem dizer onde (Cargo.toml quebrado, por exemplo): o motivo vem no stderr.
    if !saida.status.success() && lista.is_empty() {
        let erro = String::from_utf8_lossy(&saida.stderr);
        return Err(erro.lines().find(|l| l.starts_with("error")).unwrap_or(erro.trim()).to_string());
    }
    Ok(lista)
}

/// Uma mensagem JSON por linha; vale `compiler-message` de erro ou aviso com trecho principal.
fn ler(saida: &str, raiz: &Path) -> Vec<Problema> {
    let mut v: Vec<Problema> = Vec::new();
    for linha in saida.lines() {
        let Ok(j) = serde_json::from_str::<Value>(linha) else { continue };
        if j["reason"] != "compiler-message" {
            continue;
        }
        let m = &j["message"];
        let erro = match m["level"].as_str() {
            Some("error") => true,
            Some("warning") => false,
            _ => continue,
        };
        let Some(span) = m["spans"].as_array().into_iter().flatten().find(|s| s["is_primary"] == true) else {
            continue;
        };
        let caminho = raiz.join(span["file_name"].as_str().unwrap_or_default());
        if !caminho.starts_with(raiz) || caminho.components().any(|c| c.as_os_str() == "..") {
            continue;
        }
        let num = |k: &str| span[k].as_u64().unwrap_or(1) as usize;
        let p = Problema {
            caminho,
            linha: num("line_start").max(1),
            coluna: num("column_start"),
            fim: (span["line_end"] == span["line_start"]).then(|| num("column_end")),
            erro,
            mensagem: m["message"].as_str().unwrap_or_default().to_string(),
        };
        // O mesmo aviso chega uma vez por alvo (lib e bin): fica um só.
        if !v.contains(&p) {
            v.push(p);
        }
    }
    // Erros primeiro, depois pela ordem dos arquivos.
    v.sort_by(|a, b| (!a.erro, &a.caminho, a.linha).cmp(&(!b.erro, &b.caminho, b.linha)));
    v
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_erros_e_avisos_da_pasta_e_sublinha_pelas_letras() {
        let raiz = Path::new("/p");
        let saida = [
            r#"{"reason":"compiler-artifact","target":{}}"#,
            r#"{"reason":"compiler-message","message":{"level":"warning","message":"function `velho` is never used","spans":[{"file_name":"src/main.rs","line_start":45,"line_end":45,"column_start":4,"column_end":9,"is_primary":true}]}}"#,
            r#"{"reason":"compiler-message","message":{"level":"warning","message":"function `velho` is never used","spans":[{"file_name":"src/main.rs","line_start":45,"line_end":45,"column_start":4,"column_end":9,"is_primary":true}]}}"#,
            r#"{"reason":"compiler-message","message":{"level":"error","message":"mismatched types","spans":[{"file_name":"src/main.rs","line_start":42,"line_end":43,"column_start":5,"column_end":2,"is_primary":true}]}}"#,
            r#"{"reason":"compiler-message","message":{"level":"warning","message":"de fora","spans":[{"file_name":"/home/x/.cargo/registry/a.rs","line_start":1,"line_end":1,"column_start":1,"column_end":2,"is_primary":true}]}}"#,
            r#"{"reason":"compiler-message","message":{"level":"error","message":"aborting due to 1 previous error","spans":[]}}"#,
        ]
        .join("\n");
        let v = ler(&saida, raiz);
        assert_eq!(v.len(), 2);
        assert!(v[0].erro && v[0].fim.is_none());
        assert_eq!(v[0].onde(raiz), "src/main.rs:42:5");
        assert_eq!(v[1].mensagem, "function `velho` is never used");
        let w = v[1].sublinhado("fn velho() {}");
        assert_eq!((w.line, w.range), (44, 3..8));
        // Linha com acento: a coluna conta letras, o trecho é em bytes.
        let w = Problema { coluna: 2, fim: Some(3), ..v[1].clone() }.sublinhado("ção");
        assert_eq!(w.range, 2..4);
    }
}
