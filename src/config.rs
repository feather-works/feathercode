//! Preferências do FeatherCode: tema e recentes. Um JSON na pasta de config do sistema.
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_RECENTES: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tema {
    Escuro,
    Claro,
    Sistema,
}

impl Tema {
    pub fn escuro(self) -> bool {
        match self {
            Tema::Escuro => true,
            Tema::Claro => false,
            Tema::Sistema => feather_ui::tema::escuro_resolvido(true, false),
        }
    }
}

/// Pasta ou arquivo aberto, com o momento (segundos desde 1970).
#[derive(Debug, Clone)]
pub struct Recente {
    pub caminho: PathBuf,
    pub quando: u64,
}

pub struct Config {
    pub tema: Tema,
    pub pastas: Vec<Recente>,
    pub arquivos: Vec<Recente>,
    /// Onde a pessoa criou ou clonou o último projeto.
    pub pasta_projetos: Option<PathBuf>,
    pub iniciar_git: bool,
    /// A tela de boas-vindas já foi vista.
    pub boas_vindas: bool,
}

fn caminho() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("feathercode").join("config.json"))
}

pub fn agora() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Config {
    pub fn ler() -> Self {
        let v: Value = caminho()
            .and_then(|c| std::fs::read_to_string(c).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Value::Null);
        // Aceita a lista antiga (só caminhos) e a nova ({caminho, quando}).
        let lista = |k: &str| {
            v[k].as_array()
                .into_iter()
                .flatten()
                .filter_map(|r| {
                    let caminho = r.as_str().or(r["caminho"].as_str())?;
                    Some(Recente {
                        caminho: caminho.into(),
                        quando: r["quando"].as_u64().unwrap_or(0),
                    })
                })
                .collect()
        };
        let tema = match v["tema"].as_str() {
            Some("claro") => Tema::Claro,
            Some("sistema") => Tema::Sistema,
            Some(_) => Tema::Escuro,
            None if v["escuro"] == false => Tema::Claro,
            // Primeira abertura: escuro, como o artifact aprovado.
            None => Tema::Escuro,
        };
        Config {
            tema,
            pastas: lista("pastas"),
            arquivos: lista("arquivos"),
            pasta_projetos: v["pasta_projetos"].as_str().map(PathBuf::from),
            iniciar_git: v["iniciar_git"].as_bool().unwrap_or(true),
            boas_vindas: v["boas_vindas"].as_bool().unwrap_or(false),
        }
    }

    pub fn gravar(&self) {
        // A foto de conferência não mexe nos recentes de quem usa o app.
        if std::env::var_os("FEATHERCODE_CAPTURAR").is_some() {
            return;
        }
        let Some(c) = caminho() else { return };
        let lista = |l: &[Recente]| {
            l.iter()
                .map(|r| json!({"caminho": r.caminho, "quando": r.quando}))
                .collect::<Vec<_>>()
        };
        let tema = match self.tema {
            Tema::Escuro => "escuro",
            Tema::Claro => "claro",
            Tema::Sistema => "sistema",
        };
        let v = json!({
            "tema": tema,
            "pastas": lista(&self.pastas),
            "arquivos": lista(&self.arquivos),
            "pasta_projetos": self.pasta_projetos,
            "iniciar_git": self.iniciar_git,
            "boas_vindas": self.boas_vindas,
        });
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

fn recente(lista: &mut Vec<Recente>, p: PathBuf) {
    lista.retain(|x| x.caminho != p);
    lista.insert(
        0,
        Recente {
            caminho: p,
            quando: agora(),
        },
    );
    lista.truncate(MAX_RECENTES);
}

/// "agora", "há 5 min", "há 2 h", "ontem", "há 3 dias".
pub fn ha_quanto(quando: u64, agora: u64) -> String {
    if quando == 0 {
        return String::new();
    }
    let s = agora.saturating_sub(quando);
    match s {
        0..60 => "agora".into(),
        60..3_600 => format!("há {} min", s / 60),
        3_600..86_400 => format!("há {} h", s / 3_600),
        86_400..172_800 => "ontem".into(),
        _ => format!("há {} dias", s / 86_400),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn recente_vai_para_o_topo_sem_repetir() {
        let mut v: Vec<Recente> = ["a", "b", "c"]
            .iter()
            .map(|c| Recente {
                caminho: c.into(),
                quando: 0,
            })
            .collect();
        recente(&mut v, "c".into());
        assert_eq!(
            v.iter()
                .map(|r| r.caminho.to_str().unwrap())
                .collect::<Vec<_>>(),
            ["c", "a", "b"]
        );
        assert!(v[0].quando > 0);
    }

    #[test]
    fn tempo_em_palavras() {
        assert_eq!(ha_quanto(1_000, 1_030), "agora");
        assert_eq!(ha_quanto(1_000, 1_000 + 300), "há 5 min");
        assert_eq!(ha_quanto(1_000, 1_000 + 7_200), "há 2 h");
        assert_eq!(ha_quanto(1_000, 1_000 + 90_000), "ontem");
        assert_eq!(ha_quanto(1_000, 1_000 + 3 * 86_400), "há 3 dias");
        assert_eq!(ha_quanto(0, 5), "");
    }
}
