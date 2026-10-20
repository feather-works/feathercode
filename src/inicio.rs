//! Página inicial (desenho aprovado no artifact, versão 6): começar, recentes, Claude Code,
//! aparência e projeto novo por tipo de produto + tecnologia, ou clonado de um repositório.
use crate::config::{self, Tema};
use crate::modelos::{self, Tipo, MODELOS};
use crate::*;
// Explícito: o glob traria também o `column!` da std e ficaria ambíguo.
use iced::widget::{checkbox, column, row};

const LARGURA_ESQUERDA: f32 = 330.0;
const MAX_RECENTES: usize = 5;

#[derive(Debug, Clone)]
pub enum Ini {
    AbrirRecente(PathBuf, bool),
    LimparRecentes,
    Tema(Tema),
    NovoProjeto,
    ClonarPainel,
    Tipo(Tipo),
    Tecnologia(Tec),
    Nome(String),
    Git(bool),
    EscolherOnde,
    OndeEscolhido(Option<PathBuf>),
    Criar,
    Url(String),
    Clonar,
    Clonado(Result<PathBuf, String>),
    ClaudeVersao(Option<String>),
    OutrosAgentes(Vec<(&'static str, String)>),
    Comecar,
}

/// Item do menu Tecnologia: o índice em `MODELOS`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tec(usize);

impl std::fmt::Display for Tec {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(MODELOS[self.0].tecnologia)
    }
}

pub struct LinhaRecente {
    caminho: PathBuf,
    pasta: bool,
    quando: u64,
    ramo: Option<String>,
}

pub struct Estado {
    tipo: Tipo,
    modelo: usize,
    nome: String,
    erro: String,
    clonar: bool,
    url: String,
    clonando: bool,
    /// `None` enquanto confere; `Some(None)` = não achou o `claude`.
    claude: Option<Option<String>>,
    pub outros: Vec<(&'static str, String)>,
    recentes: Vec<LinhaRecente>,
}

impl Estado {
    pub fn novo() -> Self {
        Estado {
            tipo: Tipo::Desktop,
            modelo: modelos::do_tipo(Tipo::Desktop).next().map_or(0, |(i, _)| i),
            nome: String::new(),
            erro: String::new(),
            clonar: false,
            url: String::new(),
            clonando: false,
            claude: None,
            outros: Vec::new(),
            recentes: Vec::new(),
        }
    }
}

/// Versão de um programa pelo `--version`: a primeira palavra que começa com algarismo.
async fn versao(programa: &str) -> Option<String> {
    let saida = tokio::process::Command::new(programa)
        .arg("--version")
        .output()
        .await
        .ok()?;
    let texto = String::from_utf8_lossy(&saida.stdout).into_owned();
    saida.status.success().then(|| {
        texto
            .split_whitespace()
            .find(|p| p.starts_with(|c: char| c.is_ascii_digit()))
            .unwrap_or_default()
            .to_string()
    })
}

/// Confere o Claude Code e os outros agentes de linha de comando instalados.
pub fn conferir_claude() -> Task<Msg> {
    Task::batch([
        Task::perform(versao("claude"), |v| Msg::Ini(Ini::ClaudeVersao(v))),
        Task::perform(
            async {
                let mut v = Vec::new();
                for (nome, programa) in OUTROS_AGENTES {
                    if let Some(ver) = versao(programa).await {
                        v.push((nome, ver));
                    }
                }
                v
            },
            |v| Msg::Ini(Ini::OutrosAgentes(v)),
        ),
    ])
}

/// Os outros agentes que o painel sabe conduzir (`adaptadores.rs`), se estiverem instalados.
const OUTROS_AGENTES: [(&str, &str); 2] = [("Codex", "codex"), ("Antigravity", "agy")];

/// Nome da pasta do clone: o fim do endereço sem `.git`.
fn nome_do_repositorio(url: &str) -> Option<&str> {
    let nome = url
        .trim()
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()?
        .trim_end_matches(".git");
    (!nome.is_empty() && !nome.starts_with('.') && !nome.contains('\\')).then_some(nome)
}

async fn clonar(url: String, destino: PathBuf) -> Result<PathBuf, String> {
    let saida = tokio::process::Command::new("git")
        .args(["clone", "--", &url])
        .arg(&destino)
        .output()
        .await
        .map_err(|e| format!("Não deu para rodar o git ({e}). Instale o Git e tente de novo."))?;
    if saida.status.success() {
        Ok(destino)
    } else {
        Err(String::from_utf8_lossy(&saida.stderr).trim().to_string())
    }
}

impl Code {
    /// Junta pastas e arquivos recentes, do mais novo para o mais velho, com o ramo de cada pasta.
    pub(crate) fn atualizar_recentes(&mut self) {
        let pastas = self.config.pastas.iter().map(|r| (r, true));
        let arquivos = self.config.arquivos.iter().map(|r| (r, false));
        let mut v: Vec<LinhaRecente> = pastas
            .chain(arquivos)
            .map(|(r, pasta)| LinhaRecente {
                caminho: r.caminho.clone(),
                pasta,
                quando: r.quando,
                ramo: if pasta { ramo(&r.caminho) } else { None },
            })
            .collect();
        v.sort_by_key(|r| std::cmp::Reverse(r.quando));
        // Até MAX_RECENTES de cada: pastas e arquivos aparecem em listas separadas.
        let (mut pastas, mut arquivos) = (0, 0);
        v.retain(|r| {
            let n = if r.pasta { &mut pastas } else { &mut arquivos };
            *n += 1;
            *n <= MAX_RECENTES
        });
        self.ini.recentes = v;
    }

    /// Pasta-mãe do projeto novo: a escolhida antes, a de cima da última pasta aberta, ou a pessoal.
    pub(crate) fn onde_criar(&self) -> PathBuf {
        self.config
            .pasta_projetos
            .clone()
            .or_else(|| {
                self.config
                    .pastas
                    .first()
                    .and_then(|r| r.caminho.parent())
                    .map(Path::to_path_buf)
            })
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| ".".into())
    }

    pub(crate) fn atualizar_inicio(&mut self, m: Ini) -> Task<Msg> {
        match m {
            Ini::AbrirRecente(p, true) => return self.abrir_pasta(p),
            Ini::AbrirRecente(p, false) => {
                let Some(pai) = p.parent().map(Path::to_path_buf) else {
                    return Task::none();
                };
                let abrir = self.abrir_pasta(pai);
                return Task::batch([abrir, Task::done(Msg::AbrirArquivo(p))]);
            }
            Ini::LimparRecentes => {
                self.config.pastas.clear();
                self.config.arquivos.clear();
                self.config.gravar();
                self.atualizar_recentes();
            }
            Ini::Tema(t) => {
                self.config.tema = t;
                tema::definir_tema(t.escuro(), ROXO);
                self.config.gravar();
            }
            Ini::NovoProjeto => {
                self.ini.clonar = false;
                self.ini.erro.clear();
                return text_input::focus(text_input::Id::new("nome-projeto"));
            }
            Ini::ClonarPainel => {
                self.ini.clonar = true;
                self.ini.erro.clear();
                return text_input::focus(text_input::Id::new("url-clone"));
            }
            Ini::Tipo(t) => {
                self.ini.tipo = t;
                self.ini.modelo = modelos::do_tipo(t).next().map_or(0, |(i, _)| i);
            }
            Ini::Tecnologia(Tec(i)) => self.ini.modelo = i,
            Ini::Nome(n) => {
                self.ini.nome = n;
                self.ini.erro.clear();
            }
            Ini::Url(u) => {
                self.ini.url = u;
                self.ini.erro.clear();
            }
            Ini::Git(g) => {
                self.config.iniciar_git = g;
                self.config.gravar();
            }
            Ini::EscolherOnde => {
                let de = self.onde_criar();
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Onde criar o projeto")
                        .set_directory(de)
                        .pick_folder(),
                    |h| Msg::Ini(Ini::OndeEscolhido(h.map(|h| h.path().to_path_buf()))),
                );
            }
            Ini::OndeEscolhido(Some(p)) => {
                self.config.pasta_projetos = Some(p);
                self.config.gravar();
            }
            Ini::OndeEscolhido(None) => {}
            Ini::Criar => {
                let modelo = &MODELOS[self.ini.modelo];
                match modelos::criar(modelo, &self.ini.nome, &self.onde_criar()) {
                    Ok(raiz) => {
                        if self.config.iniciar_git {
                            let git = std::process::Command::new("git")
                                .args(["init", "-q"])
                                .current_dir(&raiz)
                                .status();
                            if !git.is_ok_and(|s| s.success()) {
                                self.aviso = "Projeto criado, mas o git init não rodou. Confira se o Git está instalado.".into();
                            }
                        }
                        self.ini.nome.clear();
                        let abrir = self.abrir_pasta(raiz.clone());
                        let leia = raiz.join("README.md");
                        return if leia.is_file() {
                            Task::batch([abrir, Task::done(Msg::AbrirArquivo(leia))])
                        } else {
                            abrir
                        };
                    }
                    Err(erro) => self.ini.erro = erro,
                }
            }
            Ini::Clonar => {
                let Some(nome) = nome_do_repositorio(&self.ini.url) else {
                    self.ini.erro = "Cole o endereço do repositório, ex.: https://github.com/feather-works/feather-ui".into();
                    return Task::none();
                };
                let destino = self.onde_criar().join(nome);
                if destino.exists() {
                    self.ini.erro =
                        format!("Já existe {}. Escolha outro lugar.", destino.display());
                    return Task::none();
                }
                self.ini.clonando = true;
                return Task::perform(clonar(self.ini.url.trim().to_string(), destino), |r| {
                    Msg::Ini(Ini::Clonado(r))
                });
            }
            Ini::Clonado(r) => {
                self.ini.clonando = false;
                match r {
                    Ok(p) => {
                        self.ini.url.clear();
                        self.ini.clonar = false;
                        return self.abrir_pasta(p);
                    }
                    Err(erro) => {
                        self.ini.erro = if erro.is_empty() {
                            "O git clone falhou.".into()
                        } else {
                            erro
                        }
                    }
                }
            }
            Ini::ClaudeVersao(v) => self.ini.claude = Some(v),
            Ini::OutrosAgentes(v) => self.ini.outros = v,
            Ini::Comecar => {
                self.config.boas_vindas = true;
                self.config.gravar();
            }
        }
        Task::none()
    }

    pub(crate) fn inicio_view(&self) -> Element<'_, Msg> {
        if !self.config.boas_vindas {
            return self.boas_vindas_view();
        }
        row![
            self.coluna_esquerda(),
            linha_vertical(),
            self.coluna_direita()
        ]
        .height(Length::Fill)
        .into()
    }

    fn coluna_esquerda(&self) -> Element<'_, Msg> {
        let e = &self.ini;
        let saudacao = if e.recentes.is_empty() {
            "Bem-vindo ao FeatherCode"
        } else {
            "Bom te ver de novo"
        };
        let marca = row![
            pena(48.0),
            column![
                text(saudacao).size(15).font(NEGRITO),
                text("Abra onde parou ou comece algo novo.")
                    .size(12)
                    .color(COR_TEXTO_MUTED())
            ]
            .spacing(2),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        let acoes = column![
            acao(
                "folder-open",
                "Abrir pasta",
                "Ctrl+O",
                true,
                Msg::AbrirPasta
            ),
            acao(
                "plus",
                "Novo projeto",
                "a partir de modelo",
                false,
                Msg::Ini(Ini::NovoProjeto)
            ),
            acao(
                "link",
                "Clonar repositório",
                "endereço do Git",
                false,
                Msg::Ini(Ini::ClonarPainel)
            ),
        ]
        .spacing(6);

        let agora = config::agora();
        let limpar: Element<Msg> = if e.recentes.is_empty() {
            Space::with_width(0).into()
        } else {
            link_pequeno("Limpar", Msg::Ini(Ini::LimparRecentes))
        };
        let linha_recente = |r: &LinhaRecente| -> Element<'_, Msg> {
            let nome = r
                .caminho
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| r.caminho.display().to_string());
            // Pasta: onde ela fica. Arquivo: as duas pastas de cima, que dizem de qual projeto é.
            let pai = match r.caminho.parent() {
                Some(p) if r.pasta => encurtar(p),
                Some(p) => {
                    let partes: Vec<_> =
                        p.iter().map(|c| c.to_string_lossy().into_owned()).collect();
                    partes[partes.len().saturating_sub(2)..].join("/")
                }
                None => String::new(),
            };
            let simbolo = icone_traco(if r.pasta { "folder" } else { "file-code" }, 14.0);
            let ramo: Element<Msg> = match &r.ramo {
                Some(rm) => container(
                    text(rm.clone())
                        .font(MONO)
                        .size(10)
                        .color(COR_TEXTO_MUTED()),
                )
                .padding([0, 5])
                .style(|_| container::Style {
                    border: Border {
                        color: COR_BORDA(),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
                .into(),
                None => Space::with_width(0).into(),
            };
            let linha = row![
                simbolo,
                column![
                    text(nome).size(13),
                    text(pai).size(11).color(COR_TEXTO_MUTED())
                ]
                .width(Length::Fill),
                ramo,
                text(config::ha_quanto(r.quando, agora))
                    .size(11)
                    .color(COR_TEXTO_MUTED()),
            ]
            .spacing(8)
            .align_y(Alignment::Center);
            button(linha)
                .on_press(Msg::Ini(Ini::AbrirRecente(r.caminho.clone(), r.pasta)))
                .padding([5, 6])
                .width(Length::Fill)
                .style(|_, s| item_lista(false, s, 6.0))
                .into()
        };
        let secao = |titulo: &'static str,
                     pasta: bool,
                     vazio: &'static str,
                     extra: Element<'static, Msg>| {
            let mut c = Column::new().spacing(1).push(
                row![titulo_secao(titulo), horizontal_space(), extra].align_y(Alignment::Center),
            );
            let mut algum = false;
            for r in e.recentes.iter().filter(|r| r.pasta == pasta) {
                c = c.push(linha_recente(r));
                algum = true;
            }
            if !algum {
                c = c.push(text(vazio).size(12).color(COR_TEXTO_MUTED()));
            }
            c
        };
        let recentes = column![
            secao(
                "Pastas recentes",
                true,
                "As pastas que você abrir aparecem aqui.",
                limpar
            ),
            secao(
                "Arquivos recentes",
                false,
                "Os arquivos que você abrir aparecem aqui.",
                Space::with_width(0).into()
            ),
        ]
        .spacing(16);

        let claude = self.cartao_claude();

        let corpo = column![marca, acoes, recentes, claude]
            .spacing(20)
            .padding([22, 20]);
        container(
            tema::rolagem(corpo)
                .style(rolagem_discreta)
                .height(Length::Fill),
        )
        .width(LARGURA_ESQUERDA)
        .height(Length::Fill)
        .style(|_| fundo(COR_PAINEL_FAIXA()))
        .into()
    }

    fn coluna_direita(&self) -> Element<'_, Msg> {
        let (titulo, sub) = if self.ini.clonar {
            (
                "Clonar repositório",
                "Baixa o projeto com o Git e abre a pasta no editor.",
            )
        } else {
            (
                "Novo projeto",
                "Os arquivos já vêm prontos para rodar, e a pasta abre no editor.",
            )
        };
        let cabecalho = column![
            text(titulo).size(15).font(NEGRITO),
            text(sub).size(12).color(COR_TEXTO_MUTED())
        ]
        .spacing(2);
        let cartao = if self.ini.clonar {
            self.cartao_clonar()
        } else {
            self.cartao_novo()
        };
        // shortcut: sem rolagem para o cartão poder ocupar a altura toda; janela muito baixa corta o pé.
        container(column![cabecalho, cartao].spacing(14).height(Length::Fill))
            .padding([22, 24])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| fundo(COR_PAINEL()))
            .into()
    }

    fn cartao_novo(&self) -> Element<'_, Msg> {
        let e = &self.ini;
        let m = &MODELOS[e.modelo];
        let tecnologias: Vec<Tec> = modelos::do_tipo(e.tipo).map(|(i, _)| Tec(i)).collect();
        let menus = row![
            rotulado(
                "Tipo de produto",
                pick_list(Tipo::TODOS, Some(e.tipo), |t| Msg::Ini(Ini::Tipo(t)))
                    .text_size(13)
                    .padding([7, 10])
                    .width(Length::Fill)
                    .style(lista_com_borda)
                    .menu_style(tema::estilo_menu_popup)
                    .into()
            ),
            rotulado(
                "Tecnologia",
                row![
                    selo(m.selo),
                    pick_list(tecnologias, Some(Tec(e.modelo)), |t| Msg::Ini(
                        Ini::Tecnologia(t)
                    ))
                    .text_size(13)
                    .padding([7, 10])
                    .width(Length::Fill)
                    .style(lista_com_borda)
                    .menu_style(tema::estilo_menu_popup)
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into()
            ),
        ]
        .spacing(14);

        let pacote = modelos::nome_de_pacote(&e.nome);
        let destino: Element<Msg> = match &pacote {
            Ok(n) => text(format!(
                "Vai criar {}",
                encurtar(&self.onde_criar().join(n))
            ))
            .size(12)
            .color(COR_TEXTO_MUTED())
            .into(),
            Err(err) if !e.nome.is_empty() => text(*err).size(12).color(COR_ERRO()).into(),
            Err(_) => Space::with_height(0).into(),
        };
        let nome = text_input("meu-projeto", &e.nome)
            .id(text_input::Id::new("nome-projeto"))
            .on_input(|n| Msg::Ini(Ini::Nome(n)))
            .on_submit(Msg::Ini(Ini::Criar))
            .size(13)
            .padding([7, 10])
            .style(campo_com_borda);
        let campos = column![
            rotulado("Nome do projeto", nome.into()),
            self.campo_onde(),
            destino
        ]
        .spacing(12)
        .width(Length::Fill);
        let estrutura = container(
            column![
                text("Estrutura").size(11).color(COR_TEXTO_MUTED()),
                tema::rolagem(text(m.estrutura(&e.nome)).font(MONO).size(12))
                    .style(rolagem_discreta)
                    .height(Length::Fill)
            ]
            .spacing(6),
        )
        .padding([10, 12])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            border: Border {
                color: COR_BORDA_SUAVE(),
                width: 1.0,
                radius: 8.0.into(),
            },
            ..fundo(COR_PAINEL_FAIXA())
        });

        let comando = m
            .rodar
            .replace("{nome}", pacote.as_deref().unwrap_or("meu-projeto"))
            .replace(
                "{modulo}",
                &pacote.as_deref().unwrap_or("meu_projeto").replace('-', "_"),
            );
        let git = checkbox("Iniciar repositório Git", self.config.iniciar_git)
            .on_toggle(|g| Msg::Ini(Ini::Git(g)))
            .size(14)
            .text_size(12)
            .spacing(7)
            .style(caixa_marcar);
        let criar = botao_primario(
            "Criar e abrir",
            pacote.is_ok().then_some(Msg::Ini(Ini::Criar)),
        );
        let rodape = row![
            text(format!("$ {comando}"))
                .font(MONO)
                .size(12)
                .color(COR_TEXTO_MUTED())
                .width(Length::Fill),
            git,
            criar
        ]
        .spacing(16)
        .align_y(Alignment::Center);

        cartao(
            column![
                menus,
                text(m.descricao).size(12).color(COR_TEXTO_MUTED()),
                row![campos, estrutura].spacing(18).height(Length::Fill),
                linha_horizontal(),
                rodape,
                self.erro_view()
            ]
            .spacing(16)
            .into(),
        )
    }

    fn cartao_clonar(&self) -> Element<'_, Msg> {
        let e = &self.ini;
        let destino: Element<Msg> = match nome_do_repositorio(&e.url) {
            Some(n) => text(format!(
                "Vai criar {}",
                encurtar(&self.onde_criar().join(n))
            ))
            .size(12)
            .color(COR_TEXTO_MUTED())
            .into(),
            None => Space::with_height(0).into(),
        };
        let url = text_input("https://github.com/usuario/projeto.git", &e.url)
            .id(text_input::Id::new("url-clone"))
            .on_input(|u| Msg::Ini(Ini::Url(u)))
            .on_submit(Msg::Ini(Ini::Clonar))
            .size(13)
            .padding([7, 10])
            .style(campo_com_borda);
        let pronto = !e.clonando && nome_do_repositorio(&e.url).is_some();
        let botao = botao_primario(
            if e.clonando {
                "Clonando…"
            } else {
                "Clonar e abrir"
            },
            pronto.then_some(Msg::Ini(Ini::Clonar)),
        );
        cartao(
            column![
                rotulado("Endereço do repositório", url.into()),
                self.campo_onde(),
                destino,
                linha_horizontal(),
                row![
                    link_pequeno("Voltar para Novo projeto", Msg::Ini(Ini::NovoProjeto)),
                    horizontal_space(),
                    botao
                ]
                .align_y(Alignment::Center),
                self.erro_view(),
            ]
            .spacing(14)
            .into(),
        )
    }

    fn cartao_claude(&self) -> Element<'_, Msg> {
        let (cor, fundo_claude, titulo, frase): (Color, Color, String, String) =
            match &self.ini.claude {
                None => (
                    COR_TEXTO_MUTED(),
                    COR_HOVER(),
                    "Claude Code".into(),
                    "Conferindo se está instalado…".into(),
                ),
                Some(Some(v)) => {
                    let uso = self
                        .chat
                        .uso
                        .0
                        .map_or("Pronto para o painel do agente".into(), |u| {
                            format!("{:.0}% da sessão usada", u * 100.0)
                        });
                    (
                        VERDE,
                        mistura_cor(COR_PAINEL_FAIXA(), VERDE, 0.14),
                        format!("Claude Code {v}"),
                        uso,
                    )
                }
                Some(None) => (
                    AMARELO,
                    mistura_cor(COR_PAINEL_FAIXA(), AMARELO, 0.14),
                    "Claude Code não encontrado".into(),
                    "Para instalar: curl -fsSL https://claude.ai/install.sh | bash".into(),
                ),
            };
        let claude = container(
            row![
                text("●").size(10).color(cor),
                column![
                    text(titulo).size(12).font(NEGRITO),
                    text(frase).size(11).color(COR_TEXTO_MUTED())
                ]
                .spacing(1)
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .padding([8, 10])
        .width(Length::Fill)
        .style(move |_| container::Style {
            border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..fundo(fundo_claude)
        });
        let mut col = column![claude].spacing(4);
        for (nome, v) in &self.ini.outros {
            col = col.push(
                row![
                    text("●").size(10).color(VERDE),
                    column![
                        text(format!("{nome} {v}")).size(12).font(NEGRITO),
                        text("Instalado; escolha no rodapé do painel do agente.")
                            .size(11)
                            .color(COR_TEXTO_MUTED())
                    ]
                    .spacing(1)
                ]
                .spacing(10)
                .padding([4, 10])
                .align_y(Alignment::Center),
            );
        }
        col.into()
    }

    /// Primeira abertura: tema, Claude Code e pasta dos projetos.
    fn boas_vindas_view(&self) -> Element<'_, Msg> {
        let cabecalho = column![
            pena(56.0),
            text("Bem-vindo ao FeatherCode").size(19).font(NEGRITO),
            text("Duas escolhas rápidas e você já começa.")
                .size(12)
                .color(COR_TEXTO_MUTED()),
        ]
        .spacing(6)
        .align_x(Alignment::Center)
        .width(Length::Fill);
        let temas = row![
            miniatura(Tema::Escuro, self.config.tema),
            miniatura(Tema::Claro, self.config.tema),
            miniatura(Tema::Sistema, self.config.tema)
        ]
        .spacing(8);
        let rodape = row![
            link_pequeno("Pular", Msg::Ini(Ini::Comecar)),
            horizontal_space(),
            botao_primario("Começar", Some(Msg::Ini(Ini::Comecar)))
        ]
        .align_y(Alignment::Center);
        let cartao = container(
            column![
                cabecalho,
                passo("1", "Aparência", temas.into()),
                passo("2", "Agente de IA", self.cartao_claude()),
                linha_horizontal(),
                rodape,
            ]
            .spacing(22),
        )
        .padding(iced::Padding {
            top: 28.0,
            right: 30.0,
            bottom: 22.0,
            left: 30.0,
        })
        .width(560)
        .style(|_| container::Style {
            border: Border {
                color: COR_BORDA(),
                width: 1.0,
                radius: 12.0.into(),
            },
            ..fundo(COR_PAINEL())
        });
        container(cartao)
            .center(Length::Fill)
            .style(|_| fundo(COR_CROMO()))
            .into()
    }

    fn campo_onde(&self) -> Element<'_, Msg> {
        let caixa = container(
            row![
                text(encurtar(&self.onde_criar()))
                    .size(13)
                    .width(Length::Fill),
                link_pequeno("Escolher…", Msg::Ini(Ini::EscolherOnde))
            ]
            .align_y(Alignment::Center),
        )
        .padding(iced::Padding {
            top: 3.0,
            right: 4.0,
            bottom: 3.0,
            left: 10.0,
        })
        .style(|_| container::Style {
            border: Border {
                color: COR_BORDA(),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..fundo(COR_PAINEL_FAIXA())
        });
        rotulado("Criar em", caixa.into())
    }

    fn erro_view(&self) -> Element<'_, Msg> {
        if self.ini.erro.is_empty() {
            Space::with_height(0).into()
        } else {
            text(&self.ini.erro).size(12).color(COR_ERRO()).into()
        }
    }
}

/// Passo numerado da tela de boas-vindas.
fn passo<'a>(n: &'a str, titulo: &'a str, conteudo: Element<'a, Msg>) -> Element<'a, Msg> {
    let numero = container(text(n).size(11).font(NEGRITO).color(COR_AZUL_ACENTO()))
        .center(22)
        .style(|_| container::Style {
            border: Border {
                radius: 11.0.into(),
                ..Default::default()
            },
            ..fundo(COR_AZUL_FUNDO())
        });
    row![
        numero,
        column![text(titulo).size(13).font(NEGRITO), conteudo]
            .spacing(8)
            .width(Length::Fill)
    ]
    .spacing(12)
    .into()
}

fn cartao(corpo: Element<'_, Msg>) -> Element<'_, Msg> {
    container(corpo)
        .padding([18, 20])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            border: Border {
                color: COR_BORDA(),
                width: 1.0,
                radius: 10.0.into(),
            },
            ..fundo(COR_PAINEL())
        })
        .into()
}

/// `/home/x/GitHub` vira `~/GitHub`.
fn encurtar(p: &Path) -> String {
    match dirs::home_dir().and_then(|h| p.strip_prefix(h).ok().map(Path::to_path_buf)) {
        Some(r) if r.as_os_str().is_empty() => "~".into(),
        Some(r) => format!("~/{}", r.display()),
        None => p.display().to_string(),
    }
}

fn titulo_secao(t: &str) -> Element<'_, Msg> {
    text(t.to_uppercase())
        .size(10)
        .font(NEGRITO)
        .color(COR_TEXTO_MUTED())
        .into()
}

fn rotulado<'a>(rotulo: &'a str, campo: Element<'a, Msg>) -> Element<'a, Msg> {
    column![text(rotulo).size(11).color(COR_TEXTO_MUTED()), campo]
        .spacing(5)
        .width(Length::Fill)
        .into()
}

fn link_pequeno(t: &str, msg: Msg) -> Element<'_, Msg> {
    button(text(t).size(12).color(COR_AZUL_ACENTO()))
        .on_press(msg)
        .padding([3, 6])
        .style(|_, s| item_lista(false, s, 4.0))
        .into()
}

fn acao<'a>(
    icone: &str,
    titulo: &'a str,
    dica: &'a str,
    principal: bool,
    msg: Msg,
) -> Element<'a, Msg> {
    let quadro = container(icone_traco(icone, 14.0))
        .center(26)
        .style(move |_| container::Style {
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..fundo(if principal {
                COR_AZUL_FUNDO()
            } else {
                COR_HOVER()
            })
        });
    let corpo = row![
        quadro,
        column![
            text(titulo).size(13).font(NEGRITO),
            text(dica).size(11).color(COR_TEXTO_MUTED())
        ]
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    button(corpo)
        .on_press(msg)
        .padding([7, 10])
        .width(Length::Fill)
        .style(move |_, s| button::Style {
            background: Some(
                (if matches!(s, button::Status::Hovered) {
                    COR_HOVER()
                } else {
                    COR_PAINEL()
                })
                .into(),
            ),
            text_color: COR_TEXTO(),
            border: Border {
                color: if principal {
                    COR_AZUL_ACENTO()
                } else {
                    COR_BORDA_SUAVE()
                },
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// "RS", "PY", "WEB" na cor da linguagem.
fn selo(t: &str) -> Element<'_, Msg> {
    let cor = match t {
        "RS" => Color::from_rgb8(0xb7, 0x41, 0x0e),
        "PY" => Color::from_rgb8(0x35, 0x72, 0xa5),
        _ => Color::from_rgb8(0xc2, 0x57, 0x1b),
    };
    container(
        text(t)
            .font(Font {
                weight: font::Weight::Bold,
                ..MONO
            })
            .size(9)
            .color(Color::WHITE),
    )
    .center_y(20)
    .center_x(30)
    .style(move |_| container::Style {
        background: Some(cor.into()),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// Miniatura do tema: duas faixas com as cores dele e o nome.
fn miniatura<'a>(t: Tema, atual: Tema) -> Element<'a, Msg> {
    let (cor_a, cor_b, nome) = match t {
        Tema::Escuro => (
            Color::from_rgb8(0x2a, 0x2d, 0x32),
            Color::from_rgb8(0x3d, 0x41, 0x47),
            "Escuro",
        ),
        Tema::Claro => (
            Color::from_rgb8(0xf1, 0xf3, 0xf4),
            Color::from_rgb8(0xda, 0xdc, 0xe0),
            "Claro",
        ),
        Tema::Sistema => (
            Color::from_rgb8(0x8a, 0x8e, 0x94),
            Color::from_rgb8(0xda, 0xdc, 0xe0),
            "Sistema",
        ),
    };
    let faixa = |cor: Color| {
        container(Space::with_height(6))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(cor.into()),
                border: Border {
                    color: COR_BORDA(),
                    width: 0.5,
                    radius: 2.0.into(),
                },
                ..Default::default()
            })
    };
    let ativo = t == atual;
    let corpo = column![
        faixa(cor_a),
        row![faixa(cor_b), Space::with_width(Length::Fill)],
        text(nome).size(11)
    ]
    .spacing(3);
    button(corpo)
        .on_press(Msg::Ini(Ini::Tema(t)))
        .padding(6)
        .width(Length::Fill)
        .style(move |_, s| button::Style {
            background: Some(
                (if matches!(s, button::Status::Hovered) {
                    COR_HOVER()
                } else {
                    COR_PAINEL()
                })
                .into(),
            ),
            text_color: if ativo {
                COR_TEXTO()
            } else {
                COR_TEXTO_MUTED()
            },
            border: Border {
                color: if ativo {
                    COR_AZUL_ACENTO()
                } else {
                    COR_BORDA()
                },
                width: if ativo { 2.0 } else { 1.0 },
                radius: 7.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn botao_primario(rotulo: &str, msg: Option<Msg>) -> Element<'_, Msg> {
    let ativo = msg.is_some();
    button(text(rotulo).size(12).font(NEGRITO))
        .on_press_maybe(msg)
        .padding([6, 14])
        .style(move |_, s| {
            let cor = if !ativo {
                COR_BORDA()
            } else if matches!(s, button::Status::Hovered) {
                mistura_cor(COR_AZUL_ACENTO(), COR_TEXTO(), 0.15)
            } else {
                COR_AZUL_ACENTO()
            };
            button::Style {
                background: Some(cor.into()),
                text_color: if ativo {
                    COR_PAINEL()
                } else {
                    COR_TEXTO_MUTED()
                },
                border: Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

fn caixa_marcar(_: &iced::Theme, s: checkbox::Status) -> checkbox::Style {
    let marcada = matches!(
        s,
        checkbox::Status::Active { is_checked: true }
            | checkbox::Status::Hovered { is_checked: true }
            | checkbox::Status::Disabled { is_checked: true }
    );
    checkbox::Style {
        background: (if marcada {
            COR_AZUL_ACENTO()
        } else {
            COR_PAINEL_FAIXA()
        })
        .into(),
        icon_color: COR_PAINEL(),
        border: Border {
            color: if marcada {
                COR_AZUL_ACENTO()
            } else {
                COR_BORDA()
            },
            width: 1.0,
            radius: 4.0.into(),
        },
        text_color: Some(COR_TEXTO()),
    }
}

fn lista_com_borda(_: &iced::Theme, s: pick_list::Status) -> pick_list::Style {
    pick_list::Style {
        text_color: COR_TEXTO(),
        placeholder_color: COR_TEXTO_MUTED(),
        handle_color: COR_TEXTO_MUTED(),
        background: (if matches!(s, pick_list::Status::Hovered) {
            COR_HOVER()
        } else {
            COR_PAINEL_FAIXA()
        })
        .into(),
        border: Border {
            color: if matches!(s, pick_list::Status::Opened) {
                COR_AZUL_ACENTO()
            } else {
                COR_BORDA()
            },
            width: 1.0,
            radius: 6.0.into(),
        },
    }
}

fn campo_com_borda(t: &iced::Theme, s: text_input::Status) -> text_input::Style {
    let foco = matches!(s, text_input::Status::Focused);
    text_input::Style {
        background: COR_PAINEL_FAIXA().into(),
        border: Border {
            color: if foco { COR_AZUL_ACENTO() } else { COR_BORDA() },
            width: 1.0,
            radius: 6.0.into(),
        },
        ..campo_sem_borda(t, s)
    }
}

#[cfg(test)]
mod testes {
    use super::nome_do_repositorio;

    #[test]
    fn nome_da_pasta_do_clone() {
        assert_eq!(
            nome_do_repositorio("https://github.com/feather-works/feather-ui.git"),
            Some("feather-ui")
        );
        assert_eq!(
            nome_do_repositorio("git@github.com:feather-works/featherpdf.git/"),
            Some("featherpdf")
        );
        assert_eq!(nome_do_repositorio("  "), None);
        assert_eq!(nome_do_repositorio("https://x/.."), None);
    }
}
