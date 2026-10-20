//! Codex e Antigravity no painel do agente: cada um fala o próprio protocolo, e o adaptador
//! traduz para as mensagens do Claude Code (que é o que o painel entende) e de volta.
use crate::agente::Adaptador;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

fn texto_do_usuario(v: &Value) -> Option<String> {
    (v["type"] == "user").then(|| v["message"]["content"].as_str().map(String::from))?
}

fn assistente(conteudo: Value) -> Value {
    json!({"type": "assistant", "message": {"content": conteudo}})
}

fn ferramenta(id: &str, nome: &str, input: Value) -> Value {
    assistente(json!([{"type": "tool_use", "id": id, "name": nome, "input": input}]))
}

fn resultado(id: &str, erro: bool) -> Value {
    json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": id, "is_error": erro}]}})
}

/// Pedido de permissão no formato do painel; `id` volta no `control_response`.
fn pedido(id: &str, nome: &str, input: Value) -> Value {
    json!({
        "type": "control_request",
        "request_id": id,
        "request": {
            "subtype": "can_use_tool",
            "tool_name": nome,
            "input": input,
            // Não vazio: liga o "Sempre nesta sessão", que vira `acceptForSession`.
            "permission_suggestions": [{"type": "sessao"}],
        }
    })
}

/// `codex app-server`: JSON-RPC por linha (sem o campo `jsonrpc`).
pub struct Codex {
    cwd: String,
    modo: String,
    thread: Option<String>,
    turno: Option<String>,
    /// Pedidos feitos antes de a thread existir.
    fila: Vec<String>,
    proximo: u64,
    /// Diferenças de cada `fileChange`, para mostrar quando ele pedir permissão.
    alteracoes: HashMap<String, Value>,
    janela: Option<u64>,
}

impl Codex {
    pub fn novo(pasta: &Path, modo: &str) -> Self {
        Codex {
            cwd: pasta.display().to_string(),
            modo: modo.into(),
            thread: None,
            turno: None,
            fila: Vec::new(),
            proximo: 10,
            alteracoes: HashMap::new(),
            janela: None,
        }
    }

    fn chamar(&mut self, metodo: &str, params: Value) -> String {
        self.proximo += 1;
        json!({"id": self.proximo, "method": metodo, "params": params}).to_string()
    }

    /// Modo do painel → política do Codex: pedir antes de tudo, editar na pasta sem pedir, ou só ler.
    fn politicas(&self) -> (Value, Value) {
        match self.modo.as_str() {
            "acceptEdits" => (json!("on-request"), json!({"type": "workspaceWrite"})),
            "plan" => (json!("on-request"), json!({"type": "readOnly"})),
            _ => (json!("untrusted"), json!({"type": "workspaceWrite"})),
        }
    }

    fn turno(&mut self, texto: String) -> String {
        let (aprovacao, sandbox) = self.politicas();
        let thread = self.thread.clone();
        self.chamar(
            "turn/start",
            json!({
                "threadId": thread,
                "input": [{"type": "text", "text": texto}],
                "approvalPolicy": aprovacao,
                "sandboxPolicy": sandbox,
            }),
        )
    }

    fn item(&mut self, item: &Value, fim: bool) -> Vec<Value> {
        let id = item["id"].as_str().unwrap_or_default();
        match (item["type"].as_str(), fim) {
            (Some("agentMessage"), true) => item["text"]
                .as_str()
                .filter(|t| !t.trim().is_empty())
                .map(|t| assistente(json!([{"type": "text", "text": t}])))
                .into_iter()
                .collect(),
            (Some("commandExecution"), false) => {
                vec![ferramenta(id, "Bash", json!({"command": item["command"]}))]
            }
            (Some("fileChange"), false) => {
                let caminho = item["changes"][0]["path"].clone();
                let diff = item["changes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| {
                        let d = c["diff"].as_str()?;
                        // Arquivo novo ou apagado vem como o conteúdo cru, sem os sinais da diferença.
                        let sinal = match c["kind"]["type"].as_str() {
                            Some("add") => "+",
                            Some("delete") => "-",
                            _ => return Some(d.to_string()),
                        };
                        Some(
                            d.lines()
                                .map(|l| format!("{sinal}{l}"))
                                .collect::<Vec<_>>()
                                .join("\n"),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let input = json!({"file_path": caminho, "diff": diff});
                self.alteracoes.insert(id.into(), input.clone());
                vec![ferramenta(id, "Diff", input)]
            }
            (Some("commandExecution" | "fileChange"), true) => {
                let erro = item["status"] != "completed"
                    || item["exitCode"].as_i64().is_some_and(|c| c != 0);
                vec![resultado(id, erro)]
            }
            _ => Vec::new(),
        }
    }
}

impl Adaptador for Codex {
    fn abrir(&mut self) -> Vec<String> {
        vec![json!({"id": 1, "method": "initialize", "params": {"clientInfo": {"name": "feathercode", "version": env!("CARGO_PKG_VERSION")}}}).to_string()]
    }

    fn de_fora(&mut self, v: Value) -> (Vec<Value>, Vec<String>) {
        let p = &v["params"];
        // Pedido do servidor (tem `id` e `method`): permissão.
        if let (Some(id), Some(metodo)) = (v.get("id"), v["method"].as_str()) {
            let rid = id.to_string();
            return match metodo {
                "item/commandExecution/requestApproval" => (
                    vec![pedido(&rid, "Bash", json!({"command": p["command"]}))],
                    Vec::new(),
                ),
                "item/fileChange/requestApproval" => {
                    let input = self
                        .alteracoes
                        .get(p["itemId"].as_str().unwrap_or_default())
                        .cloned()
                        .unwrap_or(json!({"file_path": "", "diff": ""}));
                    (vec![pedido(&rid, "Diff", input)], Vec::new())
                }
                _ => (
                    Vec::new(),
                    vec![json!({"id": id, "error": {"code": -32601, "message": "O FeatherCode ainda não responde a este pedido."}}).to_string()],
                ),
            };
        }
        // Respostas às nossas chamadas.
        if let Some(id) = v["id"].as_u64() {
            if id == 1 {
                let (aprovacao, sandbox) = self.politicas();
                let inicio = self.chamar(
                    "thread/start",
                    json!({"cwd": self.cwd, "approvalPolicy": aprovacao, "sandbox": if sandbox["type"] == "readOnly" { "read-only" } else { "workspace-write" }}),
                );
                return (
                    Vec::new(),
                    vec![json!({"method": "initialized"}).to_string(), inicio],
                );
            }
            let r = &v["result"];
            if let Some(t) = r["thread"]["id"].as_str() {
                self.thread = Some(t.into());
                let init = json!({"type": "system", "subtype": "init", "model": r["model"], "claude_code_version": "Codex"});
                let fila: Vec<String> = std::mem::take(&mut self.fila);
                let turnos = fila.into_iter().map(|t| self.turno(t)).collect();
                return (vec![init], turnos);
            }
            if let Some(erro) = v["error"]["message"].as_str() {
                return (
                    vec![
                        json!({"type": "result", "is_error": true, "result": format!("Codex: {erro}")}),
                    ],
                    Vec::new(),
                );
            }
            return (Vec::new(), Vec::new());
        }
        let painel = match v["method"].as_str().unwrap_or_default() {
            "turn/started" => {
                self.turno = p["turn"]["id"].as_str().map(String::from);
                Vec::new()
            }
            "item/started" => self.item(&p["item"], false),
            "item/completed" => self.item(&p["item"], true),
            "thread/tokenUsage/updated" => {
                self.janela = p["tokenUsage"]["modelContextWindow"]
                    .as_u64()
                    .or(self.janela);
                let ultimo = &p["tokenUsage"]["last"];
                vec![
                    json!({"type": "assistant", "message": {"content": [], "usage": {"input_tokens": ultimo["inputTokens"]}}}),
                ]
            }
            "account/rateLimits/updated" => {
                let l = &p["rateLimits"];
                let uso = |j: &str| l[j]["usedPercent"].as_f64().map(|u| u / 100.0);
                vec![
                    json!({"type": "rate_limit_event", "rate_limit_info": {"unifiedWindows": {
                        "five_hour": {"utilization": uso("primary")},
                        "seven_day": {"utilization": uso("secondary")},
                    }}}),
                ]
            }
            "turn/completed" => {
                self.turno = None;
                let t = &p["turn"];
                let erro = t["status"] == "failed";
                let mut r = json!({"type": "result", "is_error": erro, "result": t["error"]["message"].as_str().unwrap_or("O turno terminou com erro.")});
                if let Some(j) = self.janela {
                    r["modelUsage"] = json!({"codex": {"contextWindow": j}});
                }
                vec![r]
            }
            _ => Vec::new(),
        };
        (painel, Vec::new())
    }

    fn de_dentro(&mut self, v: Value) -> Vec<String> {
        if let Some(texto) = texto_do_usuario(&v) {
            if self.thread.is_none() {
                self.fila.push(texto);
                return Vec::new();
            }
            return vec![self.turno(texto)];
        }
        match v["type"].as_str() {
            Some("control_request") => {
                let r = &v["request"];
                match r["subtype"].as_str() {
                    Some("interrupt") => match (self.thread.clone(), self.turno.clone()) {
                        (Some(t), Some(u)) => {
                            vec![self.chamar("turn/interrupt", json!({"threadId": t, "turnId": u}))]
                        }
                        _ => Vec::new(),
                    },
                    // Vale a partir do próximo turno (o `turn/start` leva a política).
                    Some("set_permission_mode") => {
                        self.modo = r["mode"].as_str().unwrap_or("default").into();
                        Vec::new()
                    }
                    _ => Vec::new(),
                }
            }
            Some("control_response") => {
                let r = &v["response"];
                let Ok(id) = r["request_id"].as_str().unwrap_or_default().parse::<u64>() else {
                    return Vec::new();
                };
                let resposta = &r["response"];
                let decisao = match resposta["behavior"].as_str() {
                    Some("allow") if resposta.get("updatedPermissions").is_some() => {
                        "acceptForSession"
                    }
                    Some("allow") => "accept",
                    _ => "decline",
                };
                vec![json!({"id": id, "result": {"decision": decisao}}).to_string()]
            }
            _ => Vec::new(),
        }
    }
}

/// `agy` sem janela (stream-json). Ele não consegue pedir permissão nesse modo: o que precisa
/// de permissão é recusado na hora, e o painel avisa.
#[derive(Default)]
pub struct Antigravity {
    /// Texto da resposta em andamento, por passo.
    textos: HashMap<u64, String>,
}

impl Antigravity {
    pub fn argumentos(modo: &str) -> Vec<String> {
        let mut a: Vec<String> = [
            "--output-format",
            "stream-json",
            "--input-format",
            "stream-json",
        ]
        .map(String::from)
        .into();
        match modo {
            "acceptEdits" => a.extend(["--mode".into(), "accept-edits".into()]),
            "plan" => a.extend(["--mode".into(), "plan".into()]),
            _ => {}
        }
        // `-p=` precisa ser o último.
        a.push("-p=".into());
        a
    }
}

/// Ferramenta do Antigravity → a do Claude Code de mesmo efeito, para o painel descrever igual.
fn ferramenta_agy(nome: &str, p: &Value) -> (String, Value) {
    let s = |k: &str| p[k].clone();
    match nome {
        "view_file" => ("Read".into(), json!({"file_path": s("AbsolutePath")})),
        "write_to_file" => ("Write".into(), json!({"file_path": s("TargetFile")})),
        "replace_file_content" | "multi_replace_file_content" => {
            ("Edit".into(), json!({"file_path": s("TargetFile")}))
        }
        "run_command" => ("Bash".into(), json!({"command": s("CommandLine")})),
        "grep_search" => ("Grep".into(), json!({"pattern": s("Query")})),
        outro => (outro.into(), p.clone()),
    }
}

impl Adaptador for Antigravity {
    fn de_fora(&mut self, v: Value) -> (Vec<Value>, Vec<String>) {
        let painel = match v["event"].as_str() {
            Some("init") => {
                vec![
                    json!({"type": "system", "subtype": "init", "model": "", "claude_code_version": "Antigravity"}),
                ]
            }
            Some("step_update") => {
                let u = &v["step_update"];
                let passo = u["step_index"].as_u64().unwrap_or_default();
                let id = format!("agy-{passo}");
                let estado = u["state"].as_str().unwrap_or_default();
                match u["step_type"].as_str() {
                    Some("agent_response") => {
                        let texto = self.textos.entry(passo).or_default();
                        texto.push_str(u["text_delta"].as_str().unwrap_or_default());
                        if estado == "ACTIVE" {
                            Vec::new()
                        } else {
                            let texto = self.textos.remove(&passo).unwrap_or_default();
                            let mut v = Vec::new();
                            if !texto.trim().is_empty() {
                                v.push(assistente(json!([{"type": "text", "text": texto}])));
                            }
                            if let Some(t) = u["usage"]["input_tokens"].as_u64() {
                                v.push(json!({"type": "assistant", "message": {"content": [], "usage": {"input_tokens": t}}}));
                            }
                            v
                        }
                    }
                    Some("tool") if estado == "ACTIVE" => {
                        let (nome, input) = ferramenta_agy(
                            u["tool_name"].as_str().unwrap_or("?"),
                            &u["tool_info"]["parameters"],
                        );
                        vec![ferramenta(&id, &nome, input)]
                    }
                    Some("tool") => vec![resultado(&id, estado == "ERROR")],
                    _ => Vec::new(),
                }
            }
            Some("result") => {
                let r = &v["result"];
                let negadas: Vec<&str> = r["denied_actions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|a| a["display_name"].as_str())
                    .collect();
                let mut v = Vec::new();
                if !negadas.is_empty() {
                    let aviso = format!(
                        "O Antigravity não pode pedir permissão por aqui, então recusou: {}. Para ele gravar arquivos, escolha \"Aceitar edições\" e comece uma conversa nova.",
                        negadas.join(", ")
                    );
                    v.push(assistente(json!([{"type": "text", "text": aviso}])));
                }
                v.push(json!({"type": "result", "is_error": r["status"] == "ERROR", "result": r["error"].as_str().unwrap_or("O turno terminou com erro.")}));
                v
            }
            _ => Vec::new(),
        };
        (painel, Vec::new())
    }

    fn de_dentro(&mut self, v: Value) -> Vec<String> {
        texto_do_usuario(&v)
            .map(|t| {
                json!({"event": "user", "message": {"role": "user", "content": t}}).to_string()
            })
            .into_iter()
            .collect()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn codex_vira_mensagens_do_painel() {
        let mut c = Codex::novo(Path::new("/tmp/x"), "default");
        let (_, r) = c.de_fora(json!({"id": 1, "result": {}}));
        assert!(r[1].contains("thread/start") && r[1].contains("untrusted"));
        // Pedido antes de a thread existir fica na fila e sai junto com ela.
        assert!(c
            .de_dentro(json!({"type": "user", "message": {"content": "oi"}}))
            .is_empty());
        let (painel, r) =
            c.de_fora(json!({"id": 11, "result": {"thread": {"id": "t1"}, "model": "gpt-5"}}));
        assert_eq!(painel[0]["subtype"], "init");
        assert!(r[0].contains("turn/start") && r[0].contains("\"oi\""));
        let (painel, _) = c.de_fora(json!({"method": "item/completed", "params": {"item": {"type": "agentMessage", "id": "m", "text": "olá"}}}));
        assert_eq!(painel[0]["message"]["content"][0]["text"], "olá");
        // Permissão: o pedido vira control_request e a resposta volta como decisão.
        let (painel, _) = c.de_fora(json!({"id": 7, "method": "item/commandExecution/requestApproval", "params": {"command": "ls"}}));
        assert_eq!(painel[0]["request_id"], "7");
        let sim = crate::agente::permitir("7", &json!({}), Some(&json!([{"type": "sessao"}])));
        let r = c.de_dentro(serde_json::from_str(&sim).unwrap());
        assert_eq!(r[0], r#"{"id":7,"result":{"decision":"acceptForSession"}}"#);
        let nao = crate::agente::negar("7");
        assert!(c.de_dentro(serde_json::from_str(&nao).unwrap())[0].contains("decline"));
    }

    #[test]
    fn antigravity_junta_o_texto_e_avisa_o_que_recusou() {
        let mut a = Antigravity::default();
        a.de_fora(json!({"event": "step_update", "step_update": {"step_index": 1, "state": "ACTIVE", "step_type": "agent_response", "text_delta": "o"}}));
        let (p, _) = a.de_fora(json!({"event": "step_update", "step_update": {"step_index": 1, "state": "DONE", "step_type": "agent_response", "text_delta": "i"}}));
        assert_eq!(p[0]["message"]["content"][0]["text"], "oi");
        let (p, _) = a.de_fora(json!({"event": "step_update", "step_update": {"step_index": 2, "state": "ACTIVE", "step_type": "tool", "tool_name": "run_command", "tool_info": {"parameters": {"CommandLine": "ls"}}}}));
        assert_eq!(p[0]["message"]["content"][0]["name"], "Bash");
        let (p, _) = a.de_fora(json!({"event": "result", "result": {"status": "SUCCESS", "denied_actions": [{"display_name": "WriteToFile"}]}}));
        assert!(p[0]["message"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("WriteToFile"));
        assert_eq!(p[1]["type"], "result");
        assert_eq!(
            Antigravity::argumentos("acceptEdits").last().unwrap(),
            "-p="
        );
    }
}

/// Conversa de verdade com o agente instalado (gasta a conta): `cargo test -- --ignored agente_de_verdade`.
/// `FEATHERCODE_AGENTE=codex|agy` escolhe qual.
#[cfg(test)]
mod de_verdade {
    use crate::agente::{self, Evento, Provedor};
    use iced::futures::StreamExt;

    #[tokio::test]
    #[ignore]
    async fn agente_de_verdade() {
        let provedor = match std::env::var("FEATHERCODE_AGENTE").as_deref() {
            Ok("agy") => Provedor::Antigravity,
            _ => Provedor::Codex,
        };
        // O agy do snap tem um /tmp só dele e não vê pasta oculta: a do teste fica visível na casa.
        let pasta = std::path::PathBuf::from(std::env::var("HOME").unwrap())
            .join(format!("feathercode-agente-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let modo = if provedor == Provedor::Antigravity {
            "acceptEdits"
        } else {
            "default"
        };
        let mut s = Box::pin(agente::conversa(provedor, pasta.clone(), modo, None));
        let mut canal = None;
        let mut pediu = false;
        while let Some(ev) = tokio::time::timeout(std::time::Duration::from_secs(120), s.next())
            .await
            .expect("o agente parou de responder")
        {
            match ev {
                Evento::Pronto(c) => {
                    c.send(agente::mensagem_usuario(
                        "Crie o arquivo oi.txt com o texto oi e responda pronto.",
                    ))
                    .unwrap();
                    canal = Some(c);
                }
                Evento::Mensagem(v) => {
                    println!("{v}");
                    if v["type"] == "control_request" {
                        pediu = true;
                        let id = v["request_id"].as_str().unwrap();
                        canal
                            .as_ref()
                            .unwrap()
                            .send(agente::permitir(id, &v["request"]["input"], None))
                            .unwrap();
                    }
                    if v["type"] == "result" {
                        break;
                    }
                }
                Evento::Encerrado(m) => panic!("encerrou: {m:?}"),
            }
        }
        let texto = std::fs::read_to_string(pasta.join("oi.txt")).unwrap_or_default();
        println!("pediu permissão: {pediu}; arquivo: {texto:?}");
        assert!(texto.contains("oi"));
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
