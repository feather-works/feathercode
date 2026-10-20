//! Modelos de projeto da página inicial. Cada um gera arquivos que já rodam com o comando
//! indicado. `{nome}` vira o nome do pacote (com hífen) e `{modulo}` o nome com sublinhado.
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tipo {
    Desktop,
    Terminal,
    Api,
    Site,
    Automacao,
    Biblioteca,
}

impl Tipo {
    pub const TODOS: [Tipo; 6] = [
        Tipo::Desktop,
        Tipo::Terminal,
        Tipo::Api,
        Tipo::Site,
        Tipo::Automacao,
        Tipo::Biblioteca,
    ];
}

impl std::fmt::Display for Tipo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self {
            Tipo::Desktop => "Aplicativo desktop",
            Tipo::Terminal => "Programa de terminal",
            Tipo::Api => "API / servidor",
            Tipo::Site => "Site",
            Tipo::Automacao => "Automação",
            Tipo::Biblioteca => "Biblioteca",
        })
    }
}

pub struct Modelo {
    pub tipo: Tipo,
    /// "RS", "PY", "WEB": o selo colorido da linguagem.
    pub selo: &'static str,
    /// O que aparece no menu Tecnologia.
    pub tecnologia: &'static str,
    pub descricao: &'static str,
    /// Como rodar, mostrado no cartão e no README gerado.
    pub rodar: &'static str,
    arquivos: &'static [(&'static str, &'static str)],
}

const GITIGNORE_PY: &str = "__pycache__/\n*.pyc\n.venv/\n.env\ndata/output/\n";
const GITIGNORE_RS: &str = "/target\n.env\n";

pub const MODELOS: &[Modelo] = &[
    Modelo {
        tipo: Tipo::Desktop,
        selo: "RS",
        tecnologia: "Rust · Iced (FeatherWorks)",
        descricao: "Workspace com motor sem interface (core) e janela Iced com o tema feather-ui (gui).",
        rodar: "cargo run -p {nome}-gui",
        arquivos: &[
            ("Cargo.toml", "[workspace]\nresolver = \"2\"\nmembers = [\"crates/{nome}-core\", \"crates/{nome}-gui\"]\n"),
            (".gitignore", GITIGNORE_RS),
            ("crates/{nome}-core/Cargo.toml", "[package]\nname = \"{nome}-core\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n# O motor não depende de interface: compila e testa em segundos.\n[dependencies]\n"),
            ("crates/{nome}-core/src/lib.rs", "//! Regras do {nome}, sem interface.\n\npub fn saudacao(nome: &str) -> String {\n    format!(\"Olá, {}!\", nome)\n}\n\n#[cfg(test)]\nmod testes {\n    #[test]\n    fn sauda() {\n        assert_eq!(super::saudacao(\"mundo\"), \"Olá, mundo!\");\n    }\n}\n"),
            ("crates/{nome}-gui/Cargo.toml", "[package]\nname = \"{nome}-gui\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n{nome}-core = { path = \"../{nome}-core\" }\nfeather-ui = { git = \"https://github.com/feather-works/feather-ui\", tag = \"v0.2.10\" }\niced = { version = \"0.13\", features = [\"svg\", \"auto-detect-theme\"] }\n"),
            ("crates/{nome}-gui/src/main.rs", "use feather_ui::tema;\nuse iced::widget::{column, container, text};\nuse iced::{Element, Length};\n\nfn main() -> iced::Result {\n    tema::definir_tema(tema::escuro_resolvido(true, false), 0);\n    iced::application(\"{nome}\", App::update, App::view).theme(|_| tema::tema_iced()).run()\n}\n\n#[derive(Default)]\nstruct App;\n\n#[derive(Debug, Clone)]\nenum Msg {}\n\nimpl App {\n    fn update(&mut self, _msg: Msg) {}\n\n    fn view(&self) -> Element<'_, Msg> {\n        let corpo = column![text({modulo}_core::saudacao(\"FeatherWorks\")).size(24)];\n        container(corpo).center(Length::Fill).into()\n    }\n}\n"),
            ("README.md", "# {nome}\n\nAplicativo da suíte FeatherWorks.\n\n- `crates/{nome}-core`: motor, sem interface.\n- `crates/{nome}-gui`: janela Iced com o feather-ui.\n\nRodar: `cargo run -p {nome}-gui` · Testar o motor: `cargo test -p {nome}-core`\n"),
        ],
    },
    Modelo {
        tipo: Tipo::Terminal,
        selo: "RS",
        tecnologia: "Rust",
        descricao: "Programa de terminal com um módulo separado e teste.",
        rodar: "cargo run",
        arquivos: &[
            ("Cargo.toml", "[package]\nname = \"{nome}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n"),
            (".gitignore", GITIGNORE_RS),
            ("src/main.rs", "mod processamento;\n\nfn main() {\n    println!(\"Iniciando programa...\");\n    let resultado = processamento::somar(10, 20);\n    println!(\"Resultado: {}\", resultado);\n}\n"),
            ("src/processamento.rs", "pub fn somar(a: i32, b: i32) -> i32 {\n    a + b\n}\n\n#[cfg(test)]\nmod testes {\n    #[test]\n    fn soma() {\n        assert_eq!(super::somar(2, 3), 5);\n    }\n}\n"),
            ("README.md", "# {nome}\n\nRodar: `cargo run` · Testar: `cargo test` · Versão otimizada: `cargo build --release`\n"),
        ],
    },
    Modelo {
        tipo: Tipo::Api,
        selo: "RS",
        tecnologia: "Rust · Axum",
        descricao: "Servidor HTTP com Axum e Tokio, rota de saúde e rota JSON.",
        rodar: "cargo run",
        arquivos: &[
            ("Cargo.toml", "[package]\nname = \"{nome}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\naxum = \"0.7\"\ntokio = { version = \"1\", features = [\"macros\", \"rt-multi-thread\"] }\nserde_json = \"1\"\n"),
            (".gitignore", GITIGNORE_RS),
            (".env.example", "PORTA=3000\n"),
            ("src/main.rs", "use axum::{routing::get, Json, Router};\nuse serde_json::{json, Value};\n\n#[tokio::main]\nasync fn main() {\n    let porta = std::env::var(\"PORTA\").unwrap_or_else(|_| \"3000\".into());\n    let app = Router::new().route(\"/saude\", get(|| async { \"ok\" })).route(\"/ola\", get(ola));\n    let ouvinte = tokio::net::TcpListener::bind(format!(\"127.0.0.1:{porta}\")).await.expect(\"porta livre\");\n    println!(\"Servindo em http://127.0.0.1:{porta}\");\n    axum::serve(ouvinte, app).await.expect(\"servidor\");\n}\n\nasync fn ola() -> Json<Value> {\n    Json(json!({ \"mensagem\": \"Olá do {nome}\" }))\n}\n"),
            ("README.md", "# {nome}\n\nRodar: `cargo run` e abrir http://127.0.0.1:3000/ola\n"),
        ],
    },
    Modelo {
        tipo: Tipo::Terminal,
        selo: "PY",
        tecnologia: "Python",
        descricao: "Script com módulos separados, pastas de dados e teste.",
        rodar: "python main.py",
        arquivos: &[
            ("main.py", "from modules.processamento import processar\n\n\ndef main():\n    print(\"Iniciando programa...\")\n    print(f\"Resultado: {processar(10, 20)}\")\n\n\nif __name__ == \"__main__\":\n    main()\n"),
            ("modules/__init__.py", ""),
            ("modules/processamento.py", "def processar(a, b):\n    return a + b\n"),
            ("tests/test_processamento.py", "from modules.processamento import processar\n\n\ndef test_processar():\n    assert processar(2, 3) == 5\n"),
            ("data/input/.gitkeep", ""),
            ("data/output/.gitkeep", ""),
            ("requirements.txt", ""),
            (".gitignore", GITIGNORE_PY),
            ("README.md", "# {nome}\n\nRodar: `python main.py` · Testar: `python -m pytest`\n"),
        ],
    },
    Modelo {
        tipo: Tipo::Biblioteca,
        selo: "PY",
        tecnologia: "Python (pyproject)",
        descricao: "Layout src/ com pyproject.toml, núcleo e serviços separados, para distribuir.",
        rodar: "python -m {modulo}",
        arquivos: &[
            ("pyproject.toml", "[project]\nname = \"{nome}\"\nversion = \"0.1.0\"\nrequires-python = \">=3.10\"\ndependencies = []\n\n[build-system]\nrequires = [\"setuptools>=68\"]\nbuild-backend = \"setuptools.build_meta\"\n"),
            ("src/{modulo}/__init__.py", ""),
            ("src/{modulo}/__main__.py", "from {modulo}.core.engine import executar\n\nif __name__ == \"__main__\":\n    print(executar())\n"),
            ("src/{modulo}/core/__init__.py", ""),
            ("src/{modulo}/core/engine.py", "def executar():\n    return \"{nome} pronto\"\n"),
            ("src/{modulo}/services/__init__.py", ""),
            ("tests/test_engine.py", "from {modulo}.core.engine import executar\n\n\ndef test_executar():\n    assert \"pronto\" in executar()\n"),
            (".gitignore", GITIGNORE_PY),
            ("README.md", "# {nome}\n\nInstalar em modo de desenvolvimento: `python -m pip install -e .`\n\nRodar: `python -m {modulo}`\n"),
        ],
    },
    Modelo {
        tipo: Tipo::Site,
        selo: "PY",
        tecnologia: "Python · FastAPI (com servidor)",
        descricao: "Servidor Python com rotas, páginas HTML (Jinja2) e arquivos estáticos.",
        rodar: "uvicorn app.main:app --reload",
        arquivos: &[
            ("app/__init__.py", ""),
            ("app/main.py", "from fastapi import FastAPI, Request\nfrom fastapi.responses import HTMLResponse\nfrom fastapi.staticfiles import StaticFiles\nfrom fastapi.templating import Jinja2Templates\n\napp = FastAPI()\napp.mount(\"/static\", StaticFiles(directory=\"app/static\"), name=\"static\")\ntemplates = Jinja2Templates(directory=\"app/templates\")\n\n\n@app.get(\"/\", response_class=HTMLResponse)\ndef inicio(request: Request):\n    return templates.TemplateResponse(request=request, name=\"index.html\")\n"),
            ("app/templates/index.html", "<!DOCTYPE html>\n<html lang=\"pt-BR\">\n<head>\n    <meta charset=\"UTF-8\">\n    <title>{nome}</title>\n    <link rel=\"stylesheet\" href=\"/static/css/style.css\">\n</head>\n<body>\n    <h1>{nome}</h1>\n    <p>Site com backend em Python.</p>\n</body>\n</html>\n"),
            ("app/static/css/style.css", "body {\n    font-family: system-ui, sans-serif;\n    margin: 3rem auto;\n    max-width: 40rem;\n}\n"),
            ("requirements.txt", "fastapi\nuvicorn\njinja2\n"),
            (".gitignore", GITIGNORE_PY),
            ("README.md", "# {nome}\n\n```\npython -m pip install -r requirements.txt\nuvicorn app.main:app --reload\n```\n\nAbrir http://127.0.0.1:8000\n"),
        ],
    },
    Modelo {
        tipo: Tipo::Site,
        selo: "WEB",
        tecnologia: "HTML · CSS · JS (estático)",
        descricao: "Página sem servidor: estrutura, estilo e interatividade separados.",
        rodar: "python -m http.server",
        arquivos: &[
            ("index.html", "<!DOCTYPE html>\n<html lang=\"pt-BR\">\n<head>\n    <meta charset=\"UTF-8\">\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n    <title>{nome}</title>\n    <link rel=\"stylesheet\" href=\"assets/css/style.css\">\n</head>\n<body>\n    <h1>{nome}</h1>\n    <button id=\"botao\">Clique</button>\n    <script src=\"assets/js/main.js\"></script>\n</body>\n</html>\n"),
            ("assets/css/style.css", "body {\n    font-family: system-ui, sans-serif;\n    margin: 3rem auto;\n    max-width: 40rem;\n}\n"),
            ("assets/js/main.js", "document.getElementById(\"botao\").addEventListener(\"click\", () => {\n    document.querySelector(\"h1\").textContent = \"Funcionou!\";\n});\n"),
            ("assets/images/.gitkeep", ""),
            ("README.md", "# {nome}\n\nAbrir o `index.html` no navegador, ou servir a pasta: `python -m http.server`\n"),
        ],
    },
    Modelo {
        tipo: Tipo::Automacao,
        selo: "PY",
        tecnologia: "Python",
        descricao: "Tarefas de baixar, transformar e gerar relatório, com config e logs.",
        rodar: "python -m src.automacao",
        arquivos: &[
            ("src/__init__.py", ""),
            ("src/automacao/__init__.py", ""),
            ("src/automacao/__main__.py", "import logging\n\nfrom src.automacao.tasks import download, transform, report\n\nlogging.basicConfig(filename=\"logs/automacao.log\", level=logging.INFO, format=\"%(asctime)s %(message)s\")\n\n\ndef main():\n    dados = download.executar()\n    tratados = transform.executar(dados)\n    report.executar(tratados)\n    logging.info(\"Tarefa concluída\")\n\n\nif __name__ == \"__main__\":\n    main()\n"),
            ("src/automacao/tasks/__init__.py", ""),
            ("src/automacao/tasks/download.py", "def executar():\n    # Troque pela chamada à API real (ex.: requests.get(...)).\n    return [3, 1, 2]\n"),
            ("src/automacao/tasks/transform.py", "def executar(dados):\n    return sorted(dados)\n"),
            ("src/automacao/tasks/report.py", "from pathlib import Path\n\n\ndef executar(dados):\n    saida = Path(\"data/output/relatorio.txt\")\n    saida.write_text(\"\\n\".join(map(str, dados)), encoding=\"utf-8\")\n    print(f\"Relatório em {saida}\")\n"),
            ("config/settings.toml", "[api]\nurl = \"https://exemplo.com/dados\"\n"),
            ("data/output/.gitkeep", ""),
            ("logs/.gitkeep", ""),
            (".env.example", "TOKEN_API=\n"),
            (".gitignore", "__pycache__/\n.env\nlogs/*.log\ndata/output/*\n!data/output/.gitkeep\n"),
            ("README.md", "# {nome}\n\nRodar: `python -m src.automacao`\n\nAgendar no Linux (todo dia às 8h): `0 8 * * * cd /caminho/{nome} && python -m src.automacao`\n"),
        ],
    },
    Modelo {
        tipo: Tipo::Desktop,
        selo: "PY",
        tecnologia: "Python · PySide6",
        descricao: "Janela Qt com a interface separada do núcleo, pronta para crescer em telas e serviços.",
        rodar: "python -m app.main",
        arquivos: &[
            ("app/__init__.py", ""),
            ("app/main.py", "from PySide6.QtWidgets import QApplication, QLabel, QMainWindow\n\nfrom app.core.engine import saudacao\n\n\ndef main():\n    app = QApplication([])\n    janela = QMainWindow()\n    janela.setWindowTitle(\"{nome}\")\n    janela.resize(1000, 700)\n    janela.setCentralWidget(QLabel(saudacao(\"mundo\")))\n    janela.show()\n    app.exec()\n\n\nif __name__ == \"__main__\":\n    main()\n"),
            ("app/core/__init__.py", ""),
            ("app/core/engine.py", "def saudacao(nome):\n    return f\"Olá, {nome}!\"\n"),
            ("app/ui/__init__.py", ""),
            ("assets/icons/.gitkeep", ""),
            ("tests/test_engine.py", "from app.core.engine import saudacao\n\n\ndef test_saudacao():\n    assert saudacao(\"mundo\") == \"Olá, mundo!\"\n"),
            ("requirements.txt", "PySide6\n"),
            (".gitignore", GITIGNORE_PY),
            ("README.md", "# {nome}\n\n```\npython -m pip install -r requirements.txt\npython -m app.main\n```\n"),
        ],
    },
    Modelo {
        tipo: Tipo::Biblioteca,
        selo: "RS",
        tecnologia: "Rust",
        descricao: "Crate de biblioteca com teste e exemplo, para usar como dependência em outros projetos.",
        rodar: "cargo test",
        arquivos: &[
            ("Cargo.toml", "[package]\nname = \"{nome}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n"),
            (".gitignore", GITIGNORE_RS),
            ("src/lib.rs", "//! {nome}: biblioteca.\n\n/// Soma dois números.\npub fn somar(a: i64, b: i64) -> i64 {\n    a + b\n}\n\n#[cfg(test)]\nmod testes {\n    #[test]\n    fn soma() {\n        assert_eq!(super::somar(2, 3), 5);\n    }\n}\n"),
            ("examples/uso.rs", "fn main() {\n    println!(\"2 + 3 = {}\", {modulo}::somar(2, 3));\n}\n"),
            ("README.md", "# {nome}\n\nTestar: `cargo test` · Exemplo: `cargo run --example uso`\n"),
        ],
    },
    Modelo {
        tipo: Tipo::Api,
        selo: "PY",
        tecnologia: "Python · FastAPI",
        descricao: "API REST em Python com rotas separadas e documentação automática em /docs.",
        rodar: "uvicorn app.main:app --reload",
        arquivos: &[
            ("app/__init__.py", ""),
            ("app/main.py", "from fastapi import FastAPI\n\nfrom app.routes import itens\n\napp = FastAPI(title=\"{nome}\")\napp.include_router(itens.router)\n\n\n@app.get(\"/saude\")\ndef saude():\n    return {\"ok\": True}\n"),
            ("app/routes/__init__.py", ""),
            ("app/routes/itens.py", "from fastapi import APIRouter\n\nrouter = APIRouter(prefix=\"/itens\")\n\n\n@router.get(\"/\")\ndef listar():\n    return [{\"id\": 1, \"nome\": \"exemplo\"}]\n"),
            ("requirements.txt", "fastapi\nuvicorn\n"),
            (".env.example", "PORTA=8000\n"),
            (".gitignore", GITIGNORE_PY),
            ("README.md", "# {nome}\n\n```\npython -m pip install -r requirements.txt\nuvicorn app.main:app --reload\n```\n\nDocumentação: http://127.0.0.1:8000/docs\n"),
        ],
    },
];

/// Nome de pacote: minúsculas, hífen no lugar de espaço; recusa o que não serve de nome de pasta.
pub fn nome_de_pacote(nome: &str) -> Result<String, &'static str> {
    let n: String = nome
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_whitespace() || c == '_' {
                '-'
            } else {
                c
            }
        })
        .collect();
    if n.is_empty() {
        return Err("Dê um nome ao projeto.");
    }
    if !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        || !n.starts_with(|c: char| c.is_ascii_alphabetic())
    {
        return Err("Use letras sem acento, números e hífen, começando por uma letra.");
    }
    Ok(n)
}

/// Os modelos de um tipo, na ordem do menu Tecnologia.
pub fn do_tipo(tipo: Tipo) -> impl Iterator<Item = (usize, &'static Modelo)> {
    MODELOS
        .iter()
        .enumerate()
        .filter(move |(_, m)| m.tipo == tipo)
}

impl Modelo {
    /// Árvore de pastas que vai ser criada, como no `tree`.
    pub fn estrutura(&self, nome: &str) -> String {
        let pacote = nome_de_pacote(nome).unwrap_or_else(|_| "meu-projeto".into());
        let modulo = pacote.replace('-', "_");
        let mut caminhos: Vec<String> = self
            .arquivos
            .iter()
            .map(|(c, _)| c.replace("{nome}", &pacote).replace("{modulo}", &modulo))
            .collect();
        caminhos.sort();
        let mut linhas = vec![format!("{pacote}/")];
        desenhar(
            &caminhos.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
            "",
            &mut linhas,
        );
        linhas.join("\n")
    }
}

/// Desenha um nível: pastas primeiro (com o que tem dentro), depois arquivos.
fn desenhar(caminhos: &[&str], prefixo: &str, linhas: &mut Vec<String>) {
    let mut pastas: Vec<(&str, Vec<&str>)> = Vec::new();
    let mut arquivos = Vec::new();
    for c in caminhos {
        match c.split_once('/') {
            Some((pasta, resto)) => match pastas.iter_mut().find(|(p, _)| *p == pasta) {
                Some((_, filhos)) => filhos.push(resto),
                None => pastas.push((pasta, vec![resto])),
            },
            None => arquivos.push(*c),
        }
    }
    let total = pastas.len() + arquivos.len();
    let itens = pastas
        .into_iter()
        .map(|(p, f)| (p, Some(f)))
        .chain(arquivos.into_iter().map(|a| (a, None)));
    for (i, (nome, filhos)) in itens.enumerate() {
        let ultimo = i + 1 == total;
        let ramo = if ultimo { "└─ " } else { "├─ " };
        match filhos {
            // .gitkeep só existe para a pasta vazia ir junto: mostra a pasta, não o arquivo.
            Some(f) if f == [".gitkeep"] => linhas.push(format!("{prefixo}{ramo}{nome}/")),
            Some(f) => {
                linhas.push(format!("{prefixo}{ramo}{nome}/"));
                desenhar(
                    &f,
                    &format!("{prefixo}{}", if ultimo { "   " } else { "│  " }),
                    linhas,
                );
            }
            None => linhas.push(format!("{prefixo}{ramo}{nome}")),
        }
    }
}

/// Cria a pasta do projeto dentro de `onde`. Nunca escreve numa pasta que já existe.
pub fn criar(modelo: &Modelo, nome: &str, onde: &Path) -> Result<PathBuf, String> {
    let pacote = nome_de_pacote(nome)?;
    let raiz = onde.join(&pacote);
    if raiz.exists() {
        return Err(format!("Já existe {}. Escolha outro nome.", raiz.display()));
    }
    let modulo = pacote.replace('-', "_");
    let trocar = |s: &str| s.replace("{nome}", &pacote).replace("{modulo}", &modulo);
    for (caminho, conteudo) in modelo.arquivos {
        let destino = raiz.join(trocar(caminho));
        std::fs::create_dir_all(destino.parent().expect("arquivo tem pasta"))
            .map_err(|e| e.to_string())?;
        std::fs::write(&destino, trocar(conteudo)).map_err(|e| e.to_string())?;
    }
    Ok(raiz)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nome_vira_pacote_e_recusa_o_que_quebra() {
        assert_eq!(nome_de_pacote(" Meu Projeto ").unwrap(), "meu-projeto");
        assert!(nome_de_pacote("../fora").is_err());
        assert!(nome_de_pacote("ação").is_err());
        assert!(nome_de_pacote("1x").is_err());
    }

    #[test]
    fn todo_tipo_tem_modelo_e_a_estrutura_desenha_a_arvore() {
        for t in Tipo::TODOS {
            assert!(do_tipo(t).next().is_some(), "{t} sem modelo");
        }
        let rust = MODELOS
            .iter()
            .find(|m| m.tipo == Tipo::Terminal && m.selo == "RS")
            .unwrap();
        assert_eq!(rust.estrutura("Meu App"), "meu-app/\n├─ src/\n│  ├─ main.rs\n│  └─ processamento.rs\n├─ .gitignore\n├─ Cargo.toml\n└─ README.md");
    }

    #[test]
    fn todos_os_modelos_criam_os_arquivos_e_nao_sobrescrevem() {
        let onde = std::env::temp_dir().join(format!("feathercode-modelos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&onde);
        for (i, m) in MODELOS.iter().enumerate() {
            let raiz = criar(m, &format!("teste-{i}"), &onde).unwrap();
            for (c, _) in m.arquivos {
                let c = c
                    .replace("{nome}", &format!("teste-{i}"))
                    .replace("{modulo}", &format!("teste_{i}"));
                assert!(raiz.join(&c).is_file(), "{}: falta {c}", m.tecnologia);
            }
            assert!(!std::fs::read_to_string(raiz.join("README.md"))
                .unwrap()
                .contains("{nome}"));
            assert!(criar(m, &format!("teste-{i}"), &onde).is_err());
        }
        std::fs::remove_dir_all(&onde).unwrap();
    }
}
