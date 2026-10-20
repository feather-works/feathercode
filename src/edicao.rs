//! Tab/Shift+Tab nas linhas selecionadas, Ctrl+; para comentar pela linguagem do arquivo,
//! e Ctrl+Z/Ctrl+Y (o `text_editor` do Iced 0.13 não tem desfazer).
use iced::widget::text_editor::{Action, Content, Motion};
use std::path::Path;

const RECUO: &str = "    ";

/// Primeira e última linha tocadas pela seleção, ou a linha do cursor.
pub fn linhas_da_selecao(c: &Content) -> (usize, usize) {
    let (l, _) = c.cursor_position();
    let Some(sel) = c.selection() else {
        return (l, l);
    };
    let k = sel.matches('\n').count();
    if k == 0 {
        return (l, l);
    }
    let primeira = sel.split('\n').next().unwrap_or_default();
    let ultima = sel.rsplit('\n').next().unwrap_or_default();
    let linha = |i: usize| c.line(i).map(|t| t.to_string()).unwrap_or_default();
    // A seleção termina no cursor (arrastou para baixo) ou começa nele (para cima).
    let termina_no_cursor =
        l >= k && linha(l).starts_with(ultima) && linha(l - k).ends_with(primeira);
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

/// Recua (ou desfaz o recuo de) as linhas `ini..=fim`.
pub fn recuar(texto: &str, ini: usize, fim: usize, mais: bool) -> String {
    mapear_linhas(texto, ini, fim, |l| {
        if mais {
            if l.is_empty() {
                String::new()
            } else {
                format!("{RECUO}{l}")
            }
        } else {
            let tirar = l
                .chars()
                .take(RECUO.len())
                .take_while(|c| *c == ' ')
                .count();
            let tirar = if tirar == 0 && l.starts_with('\t') {
                1
            } else {
                tirar
            };
            l[tirar..].to_string()
        }
    })
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

/// Comenta as linhas, ou tira o comentário se todas já estiverem comentadas.
pub fn comentar(texto: &str, ini: usize, fim: usize, (abre, fecha): (&str, &str)) -> String {
    let linhas: Vec<&str> = texto.split('\n').collect();
    let alvo = || {
        linhas
            .iter()
            .take(fim + 1)
            .skip(ini)
            .filter(|l| !l.trim().is_empty())
    };
    let (abre_t, fecha_t) = (abre.trim_end(), fecha.trim_start());
    let todas =
        alvo().all(|l| l.trim_start().starts_with(abre_t) && l.trim_end().ends_with(fecha_t));
    // Todas recebem a marca na mesma coluna: a do menor recuo.
    let coluna = alvo()
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    mapear_linhas(texto, ini, fim, |l| {
        if l.trim().is_empty() {
            return l.to_string();
        }
        if todas {
            let recuo = l.len() - l.trim_start().len();
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
            format!("{}{resto}", &l[..recuo])
        } else {
            format!("{}{abre}{}{fecha}", &l[..coluna], &l[coluna..])
        }
    })
}

fn mapear_linhas(texto: &str, ini: usize, fim: usize, f: impl Fn(&str) -> String) -> String {
    texto
        .split('\n')
        .enumerate()
        .map(|(i, l)| {
            if (ini..=fim).contains(&i) {
                f(l)
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Troca o texto do editor e seleciona de novo as linhas mexidas (ou volta o cursor, se era uma só).
pub fn trocar_texto(c: &mut Content, novo: &str, ini: usize, fim: usize, coluna: usize) {
    *c = Content::with_text(novo);
    posicionar(c, ini, fim, coluna);
}

/// Cursor na coluna da linha `ini` (se `ini == fim`), ou as linhas `ini..=fim` selecionadas.
fn posicionar(c: &mut Content, ini: usize, fim: usize, coluna: usize) {
    c.perform(Action::Move(Motion::DocumentStart));
    for _ in 0..ini {
        c.perform(Action::Move(Motion::Down));
    }
    if ini == fim {
        let largura = c.line(ini).map_or(0, |l| l.chars().count());
        for _ in 0..coluna.min(largura) {
            c.perform(Action::Move(Motion::Right));
        }
        return;
    }
    for _ in ini..fim {
        c.perform(Action::Select(Motion::Down));
    }
    c.perform(Action::Select(Motion::End));
}

/// Retrato do texto e do cursor antes de uma edição.
struct Foto {
    texto: String,
    linha: usize,
    coluna: usize,
}

fn foto(c: &Content) -> Foto {
    let (linha, coluna) = c.cursor_position();
    Foto {
        texto: texto_exato(c),
        linha,
        coluna,
    }
}

/// As linhas como estão: o `text()` do Iced sempre põe um `\n` no fim, e voltar com ele criaria linha.
fn texto_exato(c: &Content) -> String {
    (0..c.line_count())
        .map(|i| c.line(i).map(|l| l.to_string()).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

const MAX_DESFAZER: usize = 300;

/// Pilhas do Ctrl+Z e do Ctrl+Y. Digitação seguida vira um passo só, como nos outros editores.
// shortcut: guarda o texto inteiro a cada passo; com arquivo de muitos MB, trocar por diferenças.
#[derive(Default)]
pub struct Desfazer {
    antes: Vec<Foto>,
    depois: Vec<Foto>,
    ultimo: Option<std::time::Instant>,
}

impl Desfazer {
    /// Chamar antes de mexer no texto. `agrupar` junta com o passo anterior se ele foi há pouco.
    pub fn antes_de_editar(&mut self, c: &Content, agrupar: bool) {
        let agora = std::time::Instant::now();
        let junto = agrupar
            && self
                .ultimo
                .is_some_and(|t| agora.duration_since(t).as_millis() < 800);
        if !junto {
            self.antes.push(foto(c));
            if self.antes.len() > MAX_DESFAZER {
                self.antes.remove(0);
            }
        }
        self.depois.clear();
        self.ultimo = agrupar.then_some(agora);
    }

    pub fn desfazer(&mut self, c: &mut Content) -> bool {
        trocar(&mut self.antes, &mut self.depois, c, &mut self.ultimo)
    }

    pub fn refazer(&mut self, c: &mut Content) -> bool {
        trocar(&mut self.depois, &mut self.antes, c, &mut self.ultimo)
    }
}

fn trocar(
    de: &mut Vec<Foto>,
    para: &mut Vec<Foto>,
    c: &mut Content,
    ultimo: &mut Option<std::time::Instant>,
) -> bool {
    let Some(f) = de.pop() else {
        return false;
    };
    para.push(foto(c));
    *c = Content::with_text(&f.texto);
    // O `with_text` engole a última linha vazia: devolve ela.
    while c.line_count() < f.texto.split('\n').count() {
        c.perform(Action::Move(Motion::DocumentEnd));
        c.perform(Action::Edit(iced::widget::text_editor::Edit::Enter));
    }
    posicionar(c, f.linha, f.linha, f.coluna);
    *ultimo = None;
    true
}

/// Digitar e apagar caractere juntam num passo; Enter, colar e o resto são passos próprios.
pub fn agrupa(acao: &Action) -> bool {
    use iced::widget::text_editor::Edit;
    matches!(
        acao,
        Action::Edit(Edit::Insert(c)) if !c.is_whitespace() || *c == ' '
    ) || matches!(acao, Action::Edit(Edit::Backspace | Edit::Delete))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn desfaz_e_refaz() {
        use iced::widget::text_editor::Edit;
        let mut c = Content::with_text("ab");
        let mut d = Desfazer::default();
        c.perform(Action::Move(Motion::DocumentEnd));
        for ch in ['c', 'd'] {
            let acao = Action::Edit(Edit::Insert(ch));
            d.antes_de_editar(&c, agrupa(&acao));
            c.perform(acao);
        }
        let enter = Action::Edit(Edit::Enter);
        d.antes_de_editar(&c, agrupa(&enter));
        c.perform(enter);
        assert_eq!(texto_exato(&c), "abcd\n");
        assert!(d.desfazer(&mut c));
        assert_eq!(texto_exato(&c), "abcd");
        assert!(d.desfazer(&mut c));
        assert_eq!(texto_exato(&c), "ab", "as duas letras seguidas são um passo só");
        assert!(!d.desfazer(&mut c));
        assert!(d.refazer(&mut c));
        assert_eq!(texto_exato(&c), "abcd");
        assert!(d.refazer(&mut c));
        assert_eq!(texto_exato(&c), "abcd\n");
    }

    #[test]
    fn tab_e_shift_tab_nas_linhas() {
        let t = "a\n  b\n\nc";
        assert_eq!(recuar(t, 0, 2, true), "    a\n      b\n\nc");
        assert_eq!(recuar("      x\n  y\n\tz", 0, 2, false), "  x\ny\nz");
    }

    #[test]
    fn comentar_e_descomentar() {
        let t = "fn a() {\n    let x = 1;\n\n    x\n}";
        let c = comentar(t, 1, 3, ("// ", ""));
        assert_eq!(c, "fn a() {\n    // let x = 1;\n\n    // x\n}");
        assert_eq!(comentar(&c, 1, 3, ("// ", "")), t);
        assert_eq!(
            comentar("<p>oi</p>", 0, 0, ("<!-- ", " -->")),
            "<!-- <p>oi</p> -->"
        );
        assert_eq!(
            comentar("<!-- <p>oi</p> -->", 0, 0, ("<!-- ", " -->")),
            "<p>oi</p>"
        );
        assert_eq!(marca_comentario(Path::new("a.py")), Some(("# ", "")));
    }
}
