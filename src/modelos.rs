//! Modelos de projeto da página inicial. Cada um gera arquivos que já rodam com o comando
//! indicado. `{nome}` vira o nome do pacote (com hífen) e `{modulo}` o nome com sublinhado.
use std::path::{Path, PathBuf};

pub struct Modelo {
    pub nome: &'static str,
    pub linguagem: &'static str,
    pub descricao: &'static str,
    /// Como rodar, mostrado no cartão e no README gerado.
    pub rodar: &'static str,
    arquivos: &'static [(&'static str, &'static str)],
}

const GITIGNORE_PY: &str = "__pycache__/\n*.pyc\n.venv/\n.env\ndata/output/\n";
const GITIGNORE_RS: &str = "/target\n.env\n";

pub const MODELOS: &[Modelo] = &[
    Modelo {
        nome: "Aplicativo FeatherWorks",
        linguagem: "Rust · Iced",
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
        nome: "Programa Rust",
        linguagem: "Rust",
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
        nome: "API REST em Rust",
        linguagem: "Rust · Axum",
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
        nome: "Programa Python",
        linguagem: "Python",
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
        nome: "Pacote Python",
        linguagem: "Python · pyproject",
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
        nome: "Site com FastAPI",
        linguagem: "Python · FastAPI",
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
        nome: "Site estático",
        linguagem: "HTML · CSS · JS",
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
        nome: "Automação Python",
        linguagem: "Python",
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
    fn todos_os_modelos_criam_os_arquivos_e_nao_sobrescrevem() {
        let onde = std::env::temp_dir().join(format!("feathercode-modelos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&onde);
        for (i, m) in MODELOS.iter().enumerate() {
            let raiz = criar(m, &format!("teste-{i}"), &onde).unwrap();
            for (c, _) in m.arquivos {
                let c = c
                    .replace("{nome}", &format!("teste-{i}"))
                    .replace("{modulo}", &format!("teste_{i}"));
                assert!(raiz.join(&c).is_file(), "{}: falta {c}", m.nome);
            }
            assert!(!std::fs::read_to_string(raiz.join("README.md"))
                .unwrap()
                .contains("{nome}"));
            assert!(criar(m, &format!("teste-{i}"), &onde).is_err());
        }
        std::fs::remove_dir_all(&onde).unwrap();
    }
}
