//! Preferências do FeatherCode: tema, pastas e arquivos recentes. Um JSON na pasta de config.
use serde_json::{json, Value};
use std::path::PathBuf;

const MAX_RECENTES: usize = 10;

#[derive(Default)]
pub struct Config {
    pub escuro: bool,
    pub pastas: Vec<PathBuf>,
    pub arquivos: Vec<PathBuf>,
    /// Onde a última pessoa criou um projeto novo.
    pub pasta_projetos: Option<PathBuf>,
}

fn caminho() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("feathercode").join("config.json"))
}

impl Config {
    pub fn ler() -> Self {
        let v: Value = caminho()
            .and_then(|c| std::fs::read_to_string(c).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Value::Null);
        let lista = |k: &str| {
            v[k].as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| p.as_str())
                .map(PathBuf::from)
                .collect()
        };
        Config {
            // Primeira abertura: escuro, como o artifact aprovado.
            escuro: v["escuro"].as_bool().unwrap_or(true),
            pastas: lista("pastas"),
            arquivos: lista("arquivos"),
            pasta_projetos: v["pasta_projetos"].as_str().map(PathBuf::from),
        }
    }

    pub fn gravar(&self) {
        let Some(c) = caminho() else { return };
        let v = json!({"escuro": self.escuro, "pastas": self.pastas, "arquivos": self.arquivos, "pasta_projetos": self.pasta_projetos});
        // Preferência perdida não é dado do usuário: falha ao gravar só é ignorada.
        let _ = std::fs::create_dir_all(c.parent().expect("config tem pasta"));
        let _ = std::fs::write(c, v.to_string());
    }

    pub fn usar_pasta(&mut self, p: PathBuf) {
        recente(&mut self.pastas, p);
        self.gravar();
    }

    pub fn usar_arquivo(&mut self, p: PathBuf) {
        recente(&mut self.arquivos, p);
        self.gravar();
    }
}

fn recente(lista: &mut Vec<PathBuf>, p: PathBuf) {
    lista.retain(|x| *x != p);
    lista.insert(0, p);
    lista.truncate(MAX_RECENTES);
}

#[cfg(test)]
mod testes {
    #[test]
    fn recente_vai_para_o_topo_sem_repetir() {
        let mut v = vec!["a".into(), "b".into(), "c".into()];
        super::recente(&mut v, "c".into());
        assert_eq!(v, ["c", "a", "b"].map(std::path::PathBuf::from));
    }
}
