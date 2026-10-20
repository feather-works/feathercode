//! Árvore de arquivos da pasta aberta. Só lê uma pasta quando ela é aberta na tela.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// shortcut: lista fixa em vez de ler o .gitignore; trocar pelo crate `ignore` quando incomodar.
const IGNORADAS: [&str; 4] = [".git", "target", "node_modules", ".~lock"];

pub struct Arvore {
    pub raiz: PathBuf,
    abertas: HashSet<PathBuf>,
    filhos: HashMap<PathBuf, Vec<(PathBuf, bool)>>,
}

pub struct Linha {
    pub caminho: PathBuf,
    pub pasta: bool,
    pub nivel: usize,
    pub aberta: bool,
}

impl Arvore {
    pub fn nova(raiz: PathBuf) -> Self {
        let mut a = Arvore { raiz: raiz.clone(), abertas: HashSet::from([raiz.clone()]), filhos: HashMap::new() };
        a.ler(&raiz);
        a
    }

    fn ler(&mut self, pasta: &Path) {
        let mut itens: Vec<(PathBuf, bool)> = std::fs::read_dir(pasta)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| !IGNORADAS.iter().any(|i| e.file_name().to_string_lossy().starts_with(i)))
            .map(|e| (e.path(), e.file_type().is_ok_and(|t| t.is_dir())))
            .collect();
        itens.sort_by_key(|(p, pasta)| (!pasta, p.file_name().map(|n| n.to_string_lossy().to_lowercase())));
        self.filhos.insert(pasta.to_path_buf(), itens);
    }

    pub fn alternar(&mut self, pasta: &Path) {
        if !self.abertas.remove(pasta) {
            self.abertas.insert(pasta.to_path_buf());
            self.ler(pasta);
        }
    }

    /// Relê as pastas abertas: o agente pode ter criado ou apagado arquivos.
    pub fn recarregar(&mut self) {
        self.filhos.clear();
        for p in self.abertas.clone() {
            if p.is_dir() {
                self.ler(&p);
            } else {
                self.abertas.remove(&p);
            }
        }
    }

    pub fn linhas(&self) -> Vec<Linha> {
        let mut v = Vec::new();
        self.descer(&self.raiz, 0, &mut v);
        v
    }

    fn descer(&self, pasta: &Path, nivel: usize, v: &mut Vec<Linha>) {
        for (caminho, e_pasta) in self.filhos.get(pasta).into_iter().flatten() {
            let aberta = *e_pasta && self.abertas.contains(caminho);
            v.push(Linha { caminho: caminho.clone(), pasta: *e_pasta, nivel, aberta });
            if aberta {
                self.descer(caminho, nivel + 1, v);
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn pastas_primeiro_ignora_target_e_abre_sob_demanda() {
        let raiz = std::env::temp_dir().join(format!("feathercode-arvore-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        for p in ["src", "target", "Zeta"] {
            std::fs::create_dir_all(raiz.join(p)).unwrap();
        }
        for f in ["b.rs", "src/main.rs", "Cargo.toml"] {
            std::fs::write(raiz.join(f), "").unwrap();
        }
        let mut a = Arvore::nova(raiz.clone());
        let nomes = |a: &Arvore| a.linhas().iter().map(|l| l.caminho.strip_prefix(&raiz).unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>();
        assert_eq!(nomes(&a), ["src", "Zeta", "b.rs", "Cargo.toml"]);
        a.alternar(&raiz.join("src"));
        assert_eq!(nomes(&a), ["src", "src/main.rs", "Zeta", "b.rs", "Cargo.toml"]);
        std::fs::remove_dir_all(&raiz).unwrap();
    }
}
