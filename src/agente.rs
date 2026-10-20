//! Conversa com o `claude` instalado, pelo mesmo protocolo que a extensão do VS Code usa
//! (Agent SDK): uma mensagem JSON por linha na entrada e na saída do processo.
use iced::futures::{SinkExt, Stream};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum Evento {
    /// O processo abriu; por este canal vão as linhas para a entrada dele.
    Pronto(mpsc::UnboundedSender<String>),
    Mensagem(Value),
    /// O processo terminou; `Some` traz o motivo quando foi erro.
    Encerrado(Option<String>),
}

/// Os agentes que o painel sabe conduzir. O painel fala o protocolo do Claude Code; os outros
/// passam por um adaptador (`adaptadores.rs`) que traduz nos dois sentidos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provedor {
    Claude,
    Codex,
    Antigravity,
}

impl Provedor {
    pub const TODOS: [Provedor; 3] = [Provedor::Claude, Provedor::Codex, Provedor::Antigravity];

    pub fn programa(self) -> &'static str {
        match self {
            Provedor::Claude => "claude",
            Provedor::Codex => "codex",
            Provedor::Antigravity => "agy",
        }
    }
}

impl std::fmt::Display for Provedor {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self {
            Provedor::Claude => "Claude Code",
            Provedor::Codex => "Codex",
            Provedor::Antigravity => "Antigravity",
        })
    }
}

/// Traduz entre o processo do agente e o painel (que entende as mensagens do Claude Code).
pub trait Adaptador: Send + 'static {
    /// Linhas a mandar ao processo assim que ele abre.
    fn abrir(&mut self) -> Vec<String> {
        Vec::new()
    }
    /// Uma linha JSON do processo: o que vai para o painel e o que volta para o processo.
    fn de_fora(&mut self, v: Value) -> (Vec<Value>, Vec<String>);
    /// Uma linha do painel (no formato do Claude Code): o que mandar ao processo.
    fn de_dentro(&mut self, v: Value) -> Vec<String>;
}

/// O Claude Code fala o protocolo do painel: nada a traduzir.
struct Direto;

impl Adaptador for Direto {
    fn de_fora(&mut self, v: Value) -> (Vec<Value>, Vec<String>) {
        (vec![v], Vec::new())
    }
    fn de_dentro(&mut self, v: Value) -> Vec<String> {
        vec![v.to_string()]
    }
}

/// Abre o agente escolhido na pasta e repassa tudo o que ele escreve. Soltar o stream mata o processo.
pub fn conversa(
    provedor: Provedor,
    pasta: PathBuf,
    modo: &'static str,
    retomar: Option<String>,
) -> impl Stream<Item = Evento> {
    let (args, adaptador): (Vec<String>, Box<dyn Adaptador>) = match provedor {
        Provedor::Claude => {
            let mut a: Vec<String> = [
                "--output-format",
                "stream-json",
                "--input-format",
                "stream-json",
                "--verbose",
                "--permission-prompt-tool",
                "stdio",
                "--permission-mode",
                modo,
            ]
            .map(String::from)
            .into();
            if let Some(id) = retomar {
                a.extend(["--resume".into(), id]);
            }
            (a, Box::new(Direto))
        }
        Provedor::Codex => (
            vec!["app-server".into()],
            Box::new(crate::adaptadores::Codex::novo(&pasta, modo)),
        ),
        Provedor::Antigravity => (
            crate::adaptadores::Antigravity::argumentos(modo),
            Box::new(crate::adaptadores::Antigravity::default()),
        ),
    };
    processo(provedor.programa(), args, pasta, adaptador)
}

fn processo(
    programa: &'static str,
    args: Vec<String>,
    pasta: PathBuf,
    mut adaptador: Box<dyn Adaptador>,
) -> impl Stream<Item = Evento> {
    iced::stream::channel(64, move |mut saida| async move {
        let filho = tokio::process::Command::new(programa)
            .args(&args)
            .current_dir(&pasta)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn();
        let mut filho = match filho {
            Ok(f) => f,
            Err(e) => {
                let motivo = format!("Não consegui abrir o {programa} ({e}). Confira se ele está instalado e no PATH.");
                let _ = saida.send(Evento::Encerrado(Some(motivo))).await;
                return;
            }
        };
        let mut entrada = filho.stdin.take().expect("stdin pedido");
        let mut linhas = BufReader::new(filho.stdout.take().expect("stdout pedido")).lines();
        let mut erros = filho.stderr.take().expect("stderr pedido");
        // Lido em paralelo: se ninguém esvaziar o stderr, o processo trava quando ele enche.
        let erros = tokio::spawn(async move {
            let mut s = String::new();
            let _ = erros.read_to_string(&mut s).await;
            s
        });
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        let _ = saida.send(Evento::Pronto(tx)).await;
        let mut pendentes = adaptador.abrir();
        'laco: loop {
            for m in pendentes.drain(..) {
                if entrada.write_all(format!("{m}\n").as_bytes()).await.is_err()
                    || entrada.flush().await.is_err()
                {
                    break 'laco;
                }
            }
            tokio::select! {
                l = linhas.next_line() => match l {
                    Ok(Some(l)) => {
                        if let Ok(v) = serde_json::from_str(&l) {
                            let (para_o_painel, respostas) = adaptador.de_fora(v);
                            for v in para_o_painel {
                                let _ = saida.send(Evento::Mensagem(v)).await;
                            }
                            pendentes = respostas;
                        }
                    }
                    _ => break,
                },
                Some(m) = rx.recv() => {
                    if let Ok(v) = serde_json::from_str(&m) {
                        pendentes = adaptador.de_dentro(v);
                    }
                }
            }
        }
        let status = filho.wait().await;
        let texto = erros.await.unwrap_or_default();
        let motivo = match status {
            Ok(s) if s.success() => None,
            Ok(s) => Some(format!("O {programa} terminou com erro ({s}). {}", texto.trim())),
            Err(e) => Some(e.to_string()),
        };
        let _ = saida.send(Evento::Encerrado(motivo)).await;
    })
}

pub fn mensagem_usuario(texto: &str) -> String {
    json!({"type": "user", "message": {"role": "user", "content": texto}}).to_string()
}

/// `sempre` repassa as sugestões do próprio claude ("não perguntar de novo nesta sessão").
pub fn permitir(request_id: &str, input: &Value, sempre: Option<&Value>) -> String {
    let mut resposta = json!({"behavior": "allow", "updatedInput": input});
    if let Some(s) = sempre.filter(|s| s.as_array().is_some_and(|a| !a.is_empty())) {
        resposta["updatedPermissions"] = s.clone();
    }
    json!({"type": "control_response", "response": {"subtype": "success", "request_id": request_id, "response": resposta}}).to_string()
}

pub fn negar(request_id: &str) -> String {
    let resposta = json!({"behavior": "deny", "message": "A pessoa recusou pelo FeatherCode. Pergunte o que mudar antes de tentar de novo."});
    json!({"type": "control_response", "response": {"subtype": "success", "request_id": request_id, "response": resposta}}).to_string()
}

/// Pedido do editor para o claude: `interrupt`, `set_permission_mode`…
pub fn controle(id: u64, pedido: Value) -> String {
    json!({"type": "control_request", "request_id": format!("feathercode-{id}"), "request": pedido})
        .to_string()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Resposta {
    Pendente,
    Gravou,
    Sempre,
    Recusou,
}

#[derive(Debug, Clone)]
pub enum Item {
    Eu(String),
    /// Resposta do agente já lida como Markdown.
    Texto(Vec<iced::widget::markdown::Item>),
    Ferramenta {
        id: String,
        frase: String,
        erro: bool,
    },
    Pedido {
        request_id: String,
        frase: String,
        previa: Vec<(char, String)>,
        input: Value,
        sugestoes: Value,
        resposta: Resposta,
    },
    Aviso(String),
}

#[derive(Debug)]
pub enum Sinal {
    Item(Item),
    /// Resultado de uma ferramenta já mostrada (pelo id do `tool_use`).
    Resultado {
        id: String,
        erro: bool,
    },
    /// Uso da conta, de 0 a 1: janela de 5 horas e de 7 dias.
    Uso(Option<f32>, Option<f32>),
    Inicio(Info),
    /// Tokens que a última resposta ocupou no contexto.
    Contexto(u64),
    /// Tamanho da janela de contexto do modelo, informado no fim do turno.
    JanelaContexto(u64),
    FimDoTurno(Option<String>),
}

/// O que o claude carregou ao abrir: aparece em "Detalhes da sessão".
#[derive(Debug, Clone, Default)]
pub struct Info {
    pub modelo: String,
    pub versao: String,
    /// (nome, estado): connected, pending, needs-auth, failed…
    pub mcp: Vec<(String, String)>,
    pub plugins: Vec<String>,
    pub skills: usize,
    pub comandos: usize,
    pub agentes: Vec<String>,
}

fn nomes(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().or(x["name"].as_str()))
        .map(String::from)
        .collect()
}

pub fn interpretar(v: &Value) -> Vec<Sinal> {
    let conteudo = || {
        v["message"]["content"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    };
    match v["type"].as_str() {
        Some("system") if v["subtype"] == "init" => vec![Sinal::Inicio(Info {
            modelo: v["model"].as_str().unwrap_or_default().into(),
            versao: v["claude_code_version"].as_str().unwrap_or_default().into(),
            mcp: v["mcp_servers"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|s| {
                    (
                        s["name"].as_str().unwrap_or("?").into(),
                        s["status"].as_str().unwrap_or("?").into(),
                    )
                })
                .collect(),
            plugins: nomes(&v["plugins"]),
            skills: v["skills"].as_array().map_or(0, Vec::len),
            comandos: v["slash_commands"].as_array().map_or(0, Vec::len),
            agentes: nomes(&v["agents"]),
        })],
        Some("assistant") => {
            let u = &v["message"]["usage"];
            let contexto = [
                "input_tokens",
                "cache_read_input_tokens",
                "cache_creation_input_tokens",
            ]
            .iter()
            .filter_map(|k| u[*k].as_u64())
            .sum::<u64>();
            let itens = conteudo();
            let sinais = itens.iter().filter_map(|c| match c["type"].as_str() {
                Some("text") => c["text"]
                    .as_str()
                    .filter(|t| !t.trim().is_empty())
                    .map(|t| {
                        Sinal::Item(Item::Texto(
                            iced::widget::markdown::parse(t.trim()).collect(),
                        ))
                    }),
                Some("tool_use") => Some(Sinal::Item(Item::Ferramenta {
                    id: c["id"].as_str().unwrap_or_default().into(),
                    frase: frase(c["name"].as_str().unwrap_or("?"), &c["input"]),
                    erro: false,
                })),
                _ => None,
            });
            (contexto > 0)
                .then_some(Sinal::Contexto(contexto))
                .into_iter()
                .chain(sinais)
                .collect()
        }
        Some("user") => conteudo()
            .iter()
            .filter(|c| c["type"] == "tool_result")
            .map(|c| Sinal::Resultado {
                id: c["tool_use_id"].as_str().unwrap_or_default().into(),
                erro: c["is_error"] == true,
            })
            .collect(),
        Some("control_request") if v["request"]["subtype"] == "can_use_tool" => {
            let r = &v["request"];
            let nome = r["tool_name"].as_str().unwrap_or("?");
            vec![Sinal::Item(Item::Pedido {
                request_id: v["request_id"].as_str().unwrap_or_default().into(),
                frase: frase(nome, &r["input"]),
                previa: previa(nome, &r["input"]),
                input: r["input"].clone(),
                sugestoes: r["permission_suggestions"].clone(),
                resposta: Resposta::Pendente,
            })]
        }
        Some("rate_limit_event") => {
            let janelas = &v["rate_limit_info"]["unifiedWindows"];
            let uso = |j: &str| janelas[j]["utilization"].as_f64().map(|u| u as f32);
            vec![Sinal::Uso(uso("five_hour"), uso("seven_day"))]
        }
        Some("result") => {
            let erro = (v["is_error"] == true).then(|| {
                v["result"]
                    .as_str()
                    .unwrap_or("O turno terminou com erro.")
                    .to_string()
            });
            let janela = v["modelUsage"]
                .as_object()
                .into_iter()
                .flat_map(|m| m.values())
                .filter_map(|m| m["contextWindow"].as_u64())
                .max();
            janela
                .map(Sinal::JanelaContexto)
                .into_iter()
                .chain([Sinal::FimDoTurno(erro)])
                .collect()
        }
        _ => Vec::new(),
    }
}

fn nome_curto(caminho: &str) -> &str {
    caminho.rsplit(['/', '\\']).next().unwrap_or(caminho)
}

/// "Leu main.rs", "Rodou `cargo test`": o que a ferramenta faz, em uma linha.
fn frase(nome: &str, input: &Value) -> String {
    let s = |k: &str| input[k].as_str().unwrap_or_default();
    match nome {
        "Read" => format!("Leu {}", nome_curto(s("file_path"))),
        "Write" => format!("Gravar {}", nome_curto(s("file_path"))),
        "Edit" | "MultiEdit" | "Diff" => format!("Editar {}", nome_curto(s("file_path"))),
        "Bash" => format!("Rodar {}", s("command").lines().next().unwrap_or_default()),
        "Grep" => format!("Buscou {}", s("pattern")),
        "Glob" => format!("Procurou arquivos {}", s("pattern")),
        "WebFetch" => format!("Abrir {}", s("url")),
        "WebSearch" => format!("Pesquisou \"{}\"", s("query")),
        "TodoWrite" => "Atualizou a lista de tarefas".into(),
        outro => outro.to_string(),
    }
}

const LINHAS_PREVIA: usize = 14;

/// O que vai mudar, para a pessoa decidir: diff do Edit, começo do Write, comando do Bash.
fn previa(nome: &str, input: &Value) -> Vec<(char, String)> {
    let linhas = |sinal: char, t: &str| {
        t.lines()
            .map(move |l| (sinal, l.to_string()))
            .collect::<Vec<_>>()
    };
    let mut v = match nome {
        "Edit" => [
            linhas('-', input["old_string"].as_str().unwrap_or_default()),
            linhas('+', input["new_string"].as_str().unwrap_or_default()),
        ]
        .concat(),
        "MultiEdit" => input["edits"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|e| {
                [
                    linhas('-', e["old_string"].as_str().unwrap_or_default()),
                    linhas('+', e["new_string"].as_str().unwrap_or_default()),
                ]
                .concat()
            })
            .collect(),
        "Write" => linhas('+', input["content"].as_str().unwrap_or_default()),
        "Bash" => linhas('$', input["command"].as_str().unwrap_or_default()),
        // Diferença pronta (Codex): cada linha já traz o seu sinal.
        "Diff" => input["diff"]
            .as_str()
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.starts_with("+++") && !l.starts_with("---") && !l.starts_with("@@"))
            .map(|l| match l.chars().next() {
                Some(c @ ('+' | '-')) => (c, l[1..].to_string()),
                _ => (' ', l.get(1..).unwrap_or_default().to_string()),
            })
            .collect(),
        _ => Vec::new(),
    };
    if v.len() > LINHAS_PREVIA {
        let resto = v.len() - LINHAS_PREVIA;
        v.truncate(LINHAS_PREVIA);
        v.push((' ', format!("… mais {resto} linha(s)")));
    }
    v
}

/// Conversa de exemplo para o modo de captura (fotos da tela sem gastar a conta).
pub fn exemplo() -> Vec<Item> {
    let ferramenta = |id: &str, nome, input: Value| Item::Ferramenta {
        id: id.into(),
        frase: frase(nome, &input),
        erro: false,
    };
    let edicao = json!({"file_path": "src/arvore.rs", "old_string": "const IGNORADAS: [&str; 4] = [\".git\", \"target\", \"node_modules\", \".~lock\"];", "new_string": "const IGNORADAS: [&str; 5] = [\".git\", \"target\", \"node_modules\", \".~lock\", \"dist\"];"});
    vec![
        Item::Eu("Esconde a pasta dist da árvore também".into()),
        ferramenta("1", "Read", json!({"file_path": "src/arvore.rs"})),
        ferramenta("2", "Grep", json!({"pattern": "IGNORADAS"})),
        Item::Texto(
            iced::widget::markdown::parse(
                "A lista fica em `IGNORADAS`, no começo de `arvore.rs`. Acrescentei `\"dist\"`.",
            )
            .collect(),
        ),
        Item::Pedido {
            request_id: "x".into(),
            frase: frase("Edit", &edicao),
            previa: previa("Edit", &edicao),
            input: edicao,
            sugestoes: Value::Null,
            resposta: Resposta::Pendente,
        },
    ]
}

/// Conversa guardada pelo Claude Code em `~/.claude/projects/<pasta>/<id>.jsonl`.
#[derive(Debug, Clone)]
pub struct Sessao {
    pub id: String,
    pub titulo: String,
    pub quando: u64,
}

/// Pasta onde o Claude Code guarda as conversas desta pasta: o caminho com tudo que não é letra ou
/// algarismo ASCII trocado por `-`.
fn pasta_sessoes(raiz: &Path) -> Option<PathBuf> {
    let nome: String = raiz
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    Some(dirs::home_dir()?.join(".claude/projects").join(nome))
}

/// O que a pessoa escreveu, sem o contexto do editor que o FeatherCode junta; `None` para o que não é dela.
fn texto_do_usuario(v: &Value) -> Option<String> {
    if v["isMeta"] == true || v["isSidechain"] == true {
        return None;
    }
    let texto = match &v["message"]["content"] {
        Value::String(s) => s.clone(),
        Value::Array(a) if a.iter().all(|x| x["type"] == "text") => a
            .iter()
            .filter_map(|x| x["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return None,
    };
    if let Some(i) = texto.find("<command-name>") {
        let resto = &texto[i + "<command-name>".len()..];
        return resto.split("</command-name>").next().map(String::from);
    }
    if texto.starts_with('<') {
        return None;
    }
    let texto = texto
        .split("\n\n<contexto_do_editor>")
        .next()
        .unwrap_or_default()
        .trim();
    (!texto.is_empty()).then(|| texto.to_string())
}

/// Conversas desta pasta, da mais nova para a mais velha.
pub fn sessoes(raiz: &Path) -> Vec<Sessao> {
    let Some(pasta) = pasta_sessoes(raiz) else {
        return Vec::new();
    };
    let Ok(itens) = std::fs::read_dir(pasta) else {
        return Vec::new();
    };
    let mut v: Vec<Sessao> = itens
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|e| {
            let quando = e
                .metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs();
            let texto = std::fs::read_to_string(e.path()).ok()?;
            let titulo = texto
                .lines()
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .filter(|v| v["type"] == "user")
                .find_map(|v| texto_do_usuario(&v))?;
            Some(Sessao {
                id: e.path().file_stem()?.to_string_lossy().into_owned(),
                titulo: titulo
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .chars()
                    .take(70)
                    .collect(),
                quando,
            })
        })
        .collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.quando));
    v.truncate(60);
    v
}

/// Os passos de uma conversa guardada, para mostrar antes de continuar com `--resume`.
pub fn sinais_da_sessao(raiz: &Path, id: &str) -> Vec<Sinal> {
    let Some(arquivo) = pasta_sessoes(raiz).map(|p| p.join(format!("{id}.jsonl"))) else {
        return Vec::new();
    };
    let texto = std::fs::read_to_string(arquivo).unwrap_or_default();
    let mut sinais = Vec::new();
    for v in texto
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
    {
        if v["isSidechain"] == true {
            continue;
        }
        if v["type"] == "user" {
            if let Some(t) = texto_do_usuario(&v) {
                sinais.push(Sinal::Item(Item::Eu(t)));
                continue;
            }
        }
        sinais.extend(interpretar(&v));
    }
    sinais
}

#[cfg(test)]
mod testes {
    #[test]
    fn titulo_da_sessao_sem_o_contexto_do_editor() {
        let v = serde_json::json!({"type": "user", "message": {"content": "eai\n\n<contexto_do_editor>\nArquivo aberto\n</contexto_do_editor>"}});
        assert_eq!(super::texto_do_usuario(&v).as_deref(), Some("eai"));
        let c = serde_json::json!({"type": "user", "message": {"content": "<command-message>init</command-message>\n<command-name>/init</command-name>"}});
        assert_eq!(super::texto_do_usuario(&c).as_deref(), Some("/init"));
        let meta = serde_json::json!({"type": "user", "isMeta": true, "message": {"content": "x"}});
        assert_eq!(super::texto_do_usuario(&meta), None);
    }

    use super::*;

    // Linhas reais do claude 2.1.304 (encurtadas), capturadas ao criar o FeatherCode.
    #[test]
    fn interpreta_o_pedido_de_permissao_e_o_uso() {
        let pedido: Value = serde_json::from_str(r#"{"type":"control_request","request_id":"a8a3","request":{"subtype":"can_use_tool","tool_name":"Write","input":{"file_path":"/tmp/x/ola.txt","content":"oi"},"permission_suggestions":[{"type":"setMode","mode":"acceptEdits","destination":"session"}],"tool_use_id":"toolu_01"}}"#).unwrap();
        let Some(Sinal::Item(Item::Pedido {
            request_id,
            frase,
            previa,
            ..
        })) = interpretar(&pedido).pop()
        else {
            panic!("sem pedido")
        };
        assert_eq!(
            (request_id.as_str(), frase.as_str()),
            ("a8a3", "Gravar ola.txt")
        );
        assert_eq!(previa, vec![('+', "oi".to_string())]);

        let uso: Value = serde_json::from_str(r#"{"type":"rate_limit_event","rate_limit_info":{"unifiedWindows":{"five_hour":{"utilization":0.43},"seven_day":{"utilization":0.63}}}}"#).unwrap();
        assert!(
            matches!(interpretar(&uso)[..], [Sinal::Uso(Some(a), Some(b))] if a == 0.43 && b == 0.63)
        );
    }

    #[test]
    fn resposta_de_permissao_no_formato_do_sdk() {
        let input = json!({"file_path": "a.rs"});
        let sugestoes =
            json!([{"type": "setMode", "mode": "acceptEdits", "destination": "session"}]);
        let v: Value = serde_json::from_str(&permitir("r1", &input, Some(&sugestoes))).unwrap();
        assert_eq!(v["response"]["request_id"], "r1");
        assert_eq!(v["response"]["response"]["behavior"], "allow");
        assert_eq!(v["response"]["response"]["updatedPermissions"], sugestoes);
        let v: Value = serde_json::from_str(&negar("r1")).unwrap();
        assert_eq!(v["response"]["response"]["behavior"], "deny");
    }
}
