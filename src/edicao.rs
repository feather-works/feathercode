//! Tab/Shift+Tab nas linhas selecionadas e Ctrl+; para comentar pela linguagem do arquivo.
//! O desfazer (Ctrl+Z/Ctrl+Y) é o do próprio editor do pelicano.
use pelicano::Content;
use std::path::Path;

const RECUO: &str = "    ";

/// Primeira e última linha (contando de 1) tocadas pela seleção, ou a linha do cursor.
pub fn linhas_da_selecao(c: &Content) -> (usize, usize) {
    let (l, _) = c.cursor();
    let Some(sel) = c.selection() else {
        return (l, l);
    };
    let k = sel.matches('\n').count();
    if k == 0 {
        return (l, l);
    }
    let primeira = sel.split('\n').next().unwrap_or_default();
    let ultima = sel.rsplit('\n').next().unwrap_or_default();
    let linha = |i: usize| c.line(i).unwrap_or_default();
    // A seleção termina no cursor (arrastou para baixo) ou começa nele (para cima).
    let termina_no_cursor =
        l > k && linha(l).starts_with(ultima) && linha(l - k).ends_with(primeira);
    let (ini, fim) = if termina_no_cursor {
        (l - k, l)
    } else {
        (l, l + k)
    };
    // Linhas inteiras selecionadas terminam no começo da seguinte: essa não conta.
    if ultima.is_empty() && fim > ini {
        (ini, fim - 1)
    } else {
        (ini, fim)
    }
}

/// Recua (ou desfaz o recuo de) as linhas da seleção, num passo do desfazer.
pub fn recuar(c: &Content, mais: bool) {
    c.edit_lines(|l| {
        if mais {
            (0, if l.is_empty() { "" } else { RECUO }.into())
        } else {
            (tirar_recuo(l), String::new())
        }
    });
}

/// Quantos bytes do começo o Shift+Tab tira: até quatro espaços, ou um tab.
fn tirar_recuo(l: &str) -> usize {
    match l.chars().take(RECUO.len()).take_while(|c| *c == ' ').count() {
        0 if l.starts_with('\t') => 1,
        n => n,
    }
}

/// Como o arquivo recua as linhas: com tab ou com tantos espaços.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Recuo {
    Tabs,
    Espacos(usize),
}

impl Recuo {
    /// O que o Tab põe e o Enter repete a cada nível.
    pub fn unidade(self) -> String {
        match self {
            Recuo::Tabs => "\t".into(),
            Recuo::Espacos(n) => " ".repeat(n),
        }
    }
}

impl std::fmt::Display for Recuo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Recuo::Tabs => f.write_str("Tabs"),
            Recuo::Espacos(n) => write!(f, "Espaços: {n}"),
        }
    }
}

/// Olha as primeiras linhas: mais linhas com tab, é tab; senão, o menor recuo em espaços
/// (2, 4 ou 8; o que não decide fica 4).
pub fn detectar_recuo(c: &Content) -> Recuo {
    let (mut tabs, mut espacos, mut menor) = (0, 0, usize::MAX);
    for i in 1..=c.line_count().min(400) {
        let Some(l) = c.line(i) else { break };
        if l.trim().is_empty() {
            continue;
        }
        if l.starts_with('\t') {
            tabs += 1;
        } else {
            let n = l.len() - l.trim_start_matches(' ').len();
            if n > 0 {
                espacos += 1;
                menor = menor.min(n);
            }
        }
    }
    if tabs > espacos {
        Recuo::Tabs
    } else {
        Recuo::Espacos(match menor {
            2 | 8 => menor,
            _ => 4,
        })
    }
}

/// O que o Enter escreve no cursor (`coluna` conta de 1): a quebra e o mesmo recuo da linha, mais
/// um nível depois de `{`, `(`, `[` ou `:` (Python).
pub fn quebra_com_recuo(linha: &str, coluna: usize, recuo: Recuo) -> String {
    let antes: String = linha.chars().take(coluna.saturating_sub(1)).collect();
    let atual: String = antes.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
    let mais = antes.trim_end().ends_with(['{', '(', '[', ':']);
    format!("\n{atual}{}", if mais { recuo.unidade() } else { String::new() })
}

/// Marca de comentário de linha pela extensão: (abre, fecha).
pub fn marca_comentario(arquivo: &Path) -> Option<(&'static str, &'static str)> {
    let ext = arquivo.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "rs" | "js" | "mjs" | "ts" | "tsx" | "jsx" | "c" | "h" | "cpp" | "hpp" | "cs" | "java"
        | "kt" | "go" | "swift" | "dart" | "scss" | "jsonc" => ("// ", ""),
        "py" | "sh" | "bash" | "toml" | "yaml" | "yml" | "rb" | "r" | "ps1" | "conf" | "ini"
        | "env" | "dockerfile" | "mk" => ("# ", ""),
        "sql" | "lua" | "hs" => ("-- ", ""),
        "html" | "htm" | "xml" | "svg" | "md" | "vue" => ("<!-- ", " -->"),
        "css" => ("/* ", " */"),
        _ => return None,
    })
}

/// Comenta as linhas da seleção, ou tira o comentário se todas já estiverem comentadas.
pub fn comentar(c: &Content, marca: (&str, &str)) {
    let (ini, fim) = linhas_da_selecao(c);
    let linhas: Vec<String> = (ini..=fim).filter_map(|i| c.line(i)).collect();
    let (todas, coluna) = como_comentar(linhas.iter().map(String::as_str), marca);
    c.edit_lines(|l| (l.len(), comentar_linha(l, todas, coluna, marca)));
}

/// Se todas as linhas (não vazias) já estão comentadas, e a coluna da marca: a do menor recuo.
fn como_comentar<'a>(
    linhas: impl Iterator<Item = &'a str> + Clone,
    (abre, fecha): (&str, &str),
) -> (bool, usize) {
    let alvo = linhas.filter(|l| !l.trim().is_empty());
    let (abre_t, fecha_t) = (abre.trim_end(), fecha.trim_start());
    let todas = alvo
        .clone()
        .all(|l| l.trim_start().starts_with(abre_t) && l.trim_end().ends_with(fecha_t));
    (todas, alvo.map(recuo).min().unwrap_or(0))
}

fn recuo(l: &str) -> usize {
    l.len() - l.trim_start().len()
}

fn comentar_linha(l: &str, todas: bool, coluna: usize, (abre, fecha): (&str, &str)) -> String {
    if l.trim().is_empty() {
        return l.to_string();
    }
    if !todas {
        // O recuo é de espaços (ASCII): cortar nele nunca cai no meio de um caractere.
        let coluna = coluna.min(recuo(l));
        return format!("{}{abre}{}{fecha}", &l[..coluna], &l[coluna..]);
    }
    let (abre_t, fecha_t) = (abre.trim_end(), fecha.trim_start());
    let resto = l.trim_start();
    let resto = resto
        .strip_prefix(abre)
        .or_else(|| resto.strip_prefix(abre_t))
        .unwrap_or(resto);
    let resto = if fecha.is_empty() {
        resto
    } else {
        resto
            .strip_suffix(fecha)
            .or_else(|| resto.strip_suffix(fecha_t))
            .unwrap_or(resto)
    };
    format!("{}{resto}", &l[..recuo(l)])
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O mesmo que o atalho faz no editor, sobre as linhas `ini..=fim` de um texto.
    fn comentar_texto(t: &str, ini: usize, fim: usize, marca: (&str, &str)) -> String {
        let linhas: Vec<&str> = t.split('\n').collect();
        let (todas, coluna) = como_comentar(linhas[ini..=fim].iter().copied(), marca);
        linhas
            .iter()
            .enumerate()
            .map(|(i, l)| {
                if (ini..=fim).contains(&i) {
                    comentar_linha(l, todas, coluna, marca)
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn shift_tab_tira_ate_quatro_espacos_ou_um_tab() {
        assert_eq!(tirar_recuo("      x"), 4);
        assert_eq!(tirar_recuo("  y"), 2);
        assert_eq!(tirar_recuo("\tz"), 1);
        assert_eq!(tirar_recuo("w"), 0);
    }

    #[test]
    fn comentar_e_descomentar() {
        let t = "fn a() {\n    let x = 1;\n\n    x\n}";
        let c = comentar_texto(t, 1, 3, ("// ", ""));
        assert_eq!(c, "fn a() {\n    // let x = 1;\n\n    // x\n}");
        assert_eq!(comentar_texto(&c, 1, 3, ("// ", "")), t);
        assert_eq!(
            comentar_texto("<p>oi</p>", 0, 0, ("<!-- ", " -->")),
            "<!-- <p>oi</p> -->"
        );
        assert_eq!(
            comentar_texto("<!-- <p>oi</p> -->", 0, 0, ("<!-- ", " -->")),
            "<p>oi</p>"
        );
        assert_eq!(marca_comentario(Path::new("a.py")), Some(("# ", "")));
    }

    #[test]
    fn linha_mais_curta_que_a_coluna_nao_quebra() {
        assert_eq!(comentar_linha("x", false, 4, ("// ", "")), "// x");
    }

    #[test]
    fn enter_repete_o_recuo_e_sobe_um_nivel_depois_da_chave() {
        let e4 = Recuo::Espacos(4);
        assert_eq!(quebra_com_recuo("    let x = 1;", 15, e4), "\n    ");
        assert_eq!(quebra_com_recuo("fn a() {", 9, e4), "\n    ");
        assert_eq!(quebra_com_recuo("\tif x:", 7, Recuo::Tabs), "\n\t\t");
        // Enter no meio do recuo não inventa espaço.
        assert_eq!(quebra_com_recuo("        x", 3, e4), "\n  ");
    }

    #[test]
    fn detecta_tabs_ou_espacos() {
        let c = Content::with_text("fn a() {\n  x\n  y\n}\n");
        assert_eq!(detectar_recuo(&c), Recuo::Espacos(2));
        let c = Content::with_text("a:\n\tb\n\tc\n    d\n");
        assert_eq!(detectar_recuo(&c), Recuo::Tabs);
        assert_eq!(detectar_recuo(&Content::with_text("sem recuo\n")).to_string(), "Espaços: 4");
    }
}
