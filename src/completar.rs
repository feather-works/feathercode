//! Autocompletar simples, sem servidor de linguagem: as palavras-chave da linguagem e os nomes
//! que já aparecem no arquivo (a função `teste` definida em cima vira sugestão em `tes`).
use pelicano::Content;
use std::path::Path;

/// Palavras-chave por extensão, para o arquivo novo que ainda não tem nomes.
fn palavras_chave(arquivo: &Path) -> &'static [&'static str] {
    match arquivo.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "rs" => &[
            "as", "async", "await", "break", "const", "continue", "crate", "else", "enum", "false",
            "impl", "loop", "match", "move", "return", "self", "Self", "static", "struct", "super",
            "trait", "true", "type", "unsafe", "where", "while", "let", "mut", "pub", "use", "mod",
            "for", "fn", "if", "in", "ref", "Some", "None", "Ok", "Err", "Option", "Result", "String",
            "Vec", "Box", "println!", "format!", "vec!", "assert_eq!",
        ],
        "py" | "pyi" => &[
            "False", "None", "True", "and", "assert", "async", "await", "break", "class", "continue",
            "def", "elif", "else", "except", "finally", "for", "from", "global", "import", "lambda",
            "nonlocal", "not", "pass", "raise", "return", "try", "while", "with", "yield", "print",
            "range", "len", "self", "isinstance", "enumerate",
        ],
        "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" => &[
            "async", "await", "break", "case", "catch", "class", "const", "continue", "default",
            "else", "export", "extends", "false", "finally", "for", "function", "import", "let",
            "new", "null", "return", "switch", "this", "throw", "true", "try", "typeof", "undefined",
            "while", "console", "interface", "type",
        ],
        _ => &[],
    }
}

fn de_nome(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// O pedaço de nome logo antes do cursor (`coluna` conta de 1): o que a sugestão completa.
pub fn prefixo(linha: &str, coluna: usize) -> String {
    let antes: Vec<char> = linha.chars().take(coluna.saturating_sub(1)).collect();
    let ini = antes.iter().rposition(|c| !de_nome(*c)).map_or(0, |i| i + 1);
    antes[ini..].iter().collect()
}

/// Sugestões para `prefixo`: nomes do arquivo que começam com ele (os mais usados primeiro) e
/// depois as palavras-chave. Fica de fora a própria palavra igual ao prefixo.
pub fn sugestoes(c: &Content, arquivo: &Path, prefixo: &str) -> Vec<String> {
    const MAX: usize = 30;
    if prefixo.chars().count() < 2 || prefixo.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Vec::new();
    }
    // Sensível a maiúsculas: aceitar só acrescenta o resto da palavra (o editor não apaga o prefixo).
    let combina = |p: &str| p != prefixo && p.starts_with(prefixo);
    let mut contagem: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let texto = c.text();
    for palavra in texto.split(|ch: char| !de_nome(ch)).filter(|p| combina(p)) {
        *contagem.entry(palavra).or_default() += 1;
    }
    let mut nomes: Vec<(&str, usize)> = contagem.into_iter().collect();
    nomes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.len().cmp(&b.0.len())).then(a.0.cmp(b.0)));
    let mut v: Vec<String> = nomes.into_iter().map(|(p, _)| p.to_string()).collect();
    for p in palavras_chave(arquivo).iter().filter(|p| combina(p)) {
        if !v.iter().any(|x| x == p) {
            v.push(p.to_string());
        }
    }
    v.truncate(MAX);
    v
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn funcao_definida_em_cima_vira_sugestao() {
        let c = Content::with_text("def teste():\n    pass\n\ndef tempo():\n    return teste()\n\ntes");
        let py = Path::new("a.py");
        assert_eq!(prefixo("    tes", 8), "tes");
        assert_eq!(sugestoes(&c, py, "tes"), ["teste"]);
        // A mais usada vem primeiro; depois as palavras-chave.
        let c2 = Content::with_text("def teste():\n    pass\n\ndef tempo():\n    return teste()\n\nte");
        assert_eq!(sugestoes(&c2, py, "te")[..2], ["teste", "tempo"]);
        assert_eq!(sugestoes(&Content::with_text("le"), Path::new("a.rs"), "le"), ["let"]);
        // Uma letra só, ou número, não sugere.
        assert!(sugestoes(&c, py, "t").is_empty());
        assert!(sugestoes(&c, py, "12").is_empty());
    }
}
