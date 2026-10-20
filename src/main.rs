//! FeatherCode: editor de código da suíte FeatherWorks, com o Claude Code no painel lateral.
mod agente;
mod arvore;
mod config;
mod edicao;
mod execucao;
mod inicio;
mod modelos;
mod paineis;
mod terminal;

use agente::{Item, Resposta, Sinal};
use feather_ui::cromo;
use feather_ui::icones::icone_traco;
use feather_ui::marca::{self, Produto};
use feather_ui::tema::{
    self, COR_AZUL_ACENTO, COR_AZUL_FUNDO, COR_BORDA, COR_BORDA_SUAVE, COR_CROMO, COR_CROMO_2,
    COR_ERRO, COR_HOVER, COR_PAINEL, COR_PAINEL_FAIXA, COR_TEXTO, COR_TEXTO_MUTED,
};
use iced::widget::text::{LineHeight, Wrapping};
use iced::widget::{
    button, column, container, horizontal_space, markdown, mouse_area, pick_list, progress_bar,
    row, scrollable, stack, text, text_editor, text_input, tooltip, Column, Space,
};
use iced::{
    font, highlighter, keyboard, window, Alignment, Background, Border, Color, Element, Font,
    Length, Size, Subscription, Task,
};
use paineis::Lado;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::UnboundedSender;

const NEGRITO: Font = Font {
    weight: font::Weight::Semibold,
    ..Font::DEFAULT
};
const MONO: Font = Font::MONOSPACE;
/// Índice de "Roxo" em `tema::ACENTOS`: a cor do FeatherCode.
const ROXO: usize = 5;
const AMARELO: Color = Color::from_rgb(0.9, 0.7, 0.25);
const VERDE: Color = Color::from_rgb(0.42, 0.75, 0.5);
const LARGURA_ARVORE: f32 = 240.0;
const LARGURA_AGENTE: f32 = 380.0;
const ALTURA_ABAS: f32 = 36.0;
const ALTURA_TERMINAL: f32 = 190.0;
const ALTURA_STATUS: f32 = 24.0;
const TAMANHO_FONTE: f32 = 13.0;
const ALTURA_LINHA: f32 = 20.0;
/// Largura do caractere da fonte mono em 13 px (7,8) com folga, para não cortar a linha mais longa.
const LARGURA_CARACTERE: f32 = 8.0;
const MARGEM_V: f32 = 8.0;
const MARGEM_H: f32 = 12.0;

fn main() -> iced::Result {
    let config = config::Config::ler();
    tema::definir_tema(config.tema.escuro(), ROXO);
    // Pena da suíte na cor de destaque do FeatherCode.
    marca::definir_marca(Produto::FeatherSheet, true);
    #[allow(unused_mut)]
    let mut janela = window::Settings {
        size: Size::new(1380.0, 860.0),
        min_size: Some(Size::new(960.0, 560.0)),
        // A barra de título é desenhada pelo app, como no resto da suíte.
        decorations: false,
        ..Default::default()
    };
    #[cfg(target_os = "linux")]
    {
        janela.platform_specific.application_id = "feathercode".into();
    }
    iced::application("FeatherCode", Code::update, Code::view)
        .theme(|_| tema::tema_iced())
        .window(janela)
        .subscription(Code::subscription)
        .run_with(move || Code::novo(config))
}

struct Aba {
    caminho: PathBuf,
    conteudo: text_editor::Content,
    sujo: bool,
    /// Colunas da linha mais comprida: a largura da área rolável.
    maior: usize,
}

fn maior_linha(t: &str) -> usize {
    t.lines()
        .map(|l| {
            l.chars()
                .map(|c| if c == '\t' { 4 } else { 1 })
                .sum::<usize>()
        })
        .max()
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Modo {
    Perguntar,
    AceitarEdicoes,
    Planejar,
}

impl Modo {
    const TODOS: [Modo; 3] = [Modo::Perguntar, Modo::AceitarEdicoes, Modo::Planejar];
    /// Nome do modo no claude (`--permission-mode`).
    fn id(self) -> &'static str {
        match self {
            Modo::Perguntar => "default",
            Modo::AceitarEdicoes => "acceptEdits",
            Modo::Planejar => "plan",
        }
    }
}

impl std::fmt::Display for Modo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self {
            Modo::Perguntar => "Pedir antes de editar",
            Modo::AceitarEdicoes => "Aceitar edições",
            Modo::Planejar => "Só planejar",
        })
    }
}

#[derive(Debug, Clone)]
enum Msg {
    AbrirPasta,
    PastaEscolhida(Option<PathBuf>),
    Alternar(PathBuf),
    AbrirArquivo(PathBuf),
    Editar(text_editor::Action),
    TrocarAba(usize),
    FecharAba(usize),
    Salvar,
    Pedido(String),
    Enviar,
    Parar,
    NovaConversa,
    Modo(Modo),
    Agente(agente::Evento),
    Responder(usize, Resposta),
    FocarTerminal,
    RodarProjeto,
    Term(iced_term::Event),
    NovoTerminal,
    FecharTerminal(u64),
    TrocarTerminal(usize),
    Janela(Option<window::Id>),
    Tamanho(Size),
    Capturar,
    Detalhes,
    Historico,
    Retomar(String),
    IrInicio,
    Ini(inicio::Ini),
    Link(markdown::Url),
    Capturado(window::Screenshot),
    Arrastar,
    Minimizar,
    Maximizar,
    Fechar,
    Lado(Lado),
    AlternarAgente,
    AlternarTerminal,
    Busca(String),
    BuscarAgora,
    Achados(Vec<paineis::Achado>),
    IrPara(PathBuf, usize),
    GitAtualizar,
    Git(Result<paineis::EstadoGit, String>),
    Menu(Option<usize>),
    TemaCodigo(usize),
    Filtro(String),
    ProcurarArquivo,
    Recuar(bool),
    Comentar,
}

struct Code {
    /// Página inicial à mostra (nenhuma pasta aberta ainda, ou a pessoa voltou para ela).
    inicio: bool,
    config: config::Config,
    ini: inicio::Estado,
    arvore: arvore::Arvore,
    abas: Vec<Aba>,
    ativa: usize,
    /// Muda a cada conversa nova: a subscription troca de id e o processo antigo morre.
    conversa: u64,
    ligado: bool,
    canal: Option<UnboundedSender<String>>,
    /// Linhas escritas antes de o claude abrir.
    fila: Vec<String>,
    itens: Vec<Item>,
    ocupado: bool,
    pedido: String,
    modo: Modo,
    uso: (Option<f32>, Option<f32>),
    info: agente::Info,
    /// Tokens no contexto e tamanho da janela do modelo.
    contexto: (u64, u64),
    detalhes: bool,
    proximo_controle: u64,
    terminais: terminal::Terminais,
    aviso: String,
    ramo: Option<String>,
    tamanho: Size,
    janela: Option<window::Id>,
    ultimo_clique_barra: Option<std::time::Instant>,
    /// Painel ao lado do trilho; `None` = recolhido.
    lado: Option<Lado>,
    agente_aberto: bool,
    terminal_aberto: bool,
    busca: String,
    achados: Option<Vec<paineis::Achado>>,
    buscando: bool,
    git: Option<Result<paineis::EstadoGit, String>>,
    menu: Option<usize>,
    /// Conversa guardada que o próximo `claude` continua (`--resume`).
    retomar: Option<String>,
    /// Lista do histórico à mostra no painel do agente.
    historico: Option<Vec<agente::Sessao>>,
    /// Texto do campo "Procurar arquivo" do explorador.
    filtro: String,
    /// Lida na primeira letra do filtro; vazia de novo quando a pasta muda.
    todos_arquivos: Vec<PathBuf>,
}

impl Code {
    fn novo(mut config: config::Config) -> (Self, Task<Msg>) {
        // Sem pasta na linha de comando, abre na página inicial.
        let pedida = std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .map(|p| p.canonicalize().unwrap_or(p));
        if let Some(p) = &pedida {
            config.usar_pasta(p.clone());
        }
        let inicio = pedida.is_none();
        let pasta = pedida
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| ".".into());
        let code = Code {
            inicio,
            config,
            ini: inicio::Estado::novo(),
            arvore: arvore::Arvore::nova(pasta.clone()),
            abas: Vec::new(),
            ativa: 0,
            conversa: 0,
            ligado: false,
            canal: None,
            fila: Vec::new(),
            itens: Vec::new(),
            ocupado: false,
            pedido: String::new(),
            modo: Modo::Perguntar,
            uso: (None, None),
            info: agente::Info::default(),
            contexto: (0, 0),
            detalhes: false,
            proximo_controle: 0,
            terminais: terminal::Terminais::default(),
            aviso: String::new(),
            ramo: ramo(&pasta),
            tamanho: Size::new(1380.0, 860.0),
            janela: None,
            ultimo_clique_barra: None,
            lado: Some(Lado::Explorador),
            agente_aberto: true,
            terminal_aberto: true,
            busca: String::new(),
            achados: None,
            buscando: false,
            git: None,
            menu: None,
            retomar: None,
            historico: None,
            filtro: String::new(),
            todos_arquivos: Vec::new(),
        };
        let mut code = code;
        if !code.inicio {
            if let Err(e) = code.terminais.novo(&code.arvore.raiz.clone(), None) {
                code.aviso = e;
            }
        }
        // Modo de captura: FEATHERCODE_CAPTURAR=foto.png fotografa a janela e fecha;
        // FEATHERCODE_CAPTURAR_ARQUIVO abre um arquivo e FEATHERCODE_CAPTURAR_EXEMPLO=1 põe uma conversa de exemplo.
        if let Some(arquivo) = std::env::var_os("FEATHERCODE_CAPTURAR_ARQUIVO") {
            let _ = code.update(Msg::AbrirArquivo(code.arvore.raiz.join(arquivo)));
        }
        // FEATHERCODE_CAPTURAR_MENU=1 deixa o menu Exibir aberto na foto.
        if let Some(i) = std::env::var("FEATHERCODE_CAPTURAR_MENU")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            code.menu = Some(i);
        }
        if std::env::var_os("FEATHERCODE_CAPTURAR_HISTORICO").is_some() {
            code.historico = Some(agente::sessoes(&code.arvore.raiz));
        }
        if std::env::var_os("FEATHERCODE_CAPTURAR_EXEMPLO").is_some() {
            code.itens = agente::exemplo();
            code.uso = (Some(0.43), Some(0.63));
            code.contexto = (420_000, 1_000_000);
            code.info.modelo = "claude-opus-5-5[1m]".into();
        }
        code.atualizar_recentes();
        // FEATHERCODE_CAPTURAR_GIT=1 abre o painel Git na foto.
        let git = if std::env::var_os("FEATHERCODE_CAPTURAR_GIT").is_some() {
            code.update(Msg::Lado(Lado::Git))
        } else {
            Task::none()
        };
        (
            code,
            Task::batch([
                window::get_latest().map(Msg::Janela),
                inicio::conferir_claude(),
                git,
            ]),
        )
    }

    fn subscription(&self) -> Subscription<Msg> {
        let teclas = keyboard::on_key_press(|tecla, m| match tecla.as_ref() {
            keyboard::Key::Character("s") if m.command() => Some(Msg::Salvar),
            keyboard::Key::Character("o") if m.command() => Some(Msg::AbrirPasta),
            keyboard::Key::Character("b") if m.command() => Some(Msg::Lado(Lado::Explorador)),
            keyboard::Key::Character("p") if m.command() => Some(Msg::ProcurarArquivo),
            keyboard::Key::Named(keyboard::key::Named::F5) => Some(Msg::RodarProjeto),
            keyboard::Key::Character("j") if m.command() => Some(Msg::AlternarTerminal),
            keyboard::Key::Character("f" | "F") if m.command() && m.shift() => {
                Some(Msg::Lado(Lado::Buscar))
            }
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Msg::Menu(None)),
            _ => None,
        });
        let mut subs = vec![
            teclas,
            window::resize_events().map(|(_, s)| Msg::Tamanho(s)),
        ];
        if self.ligado {
            let conversa = agente::conversa(
                self.arvore.raiz.clone(),
                self.modo.id(),
                self.retomar.clone(),
            );
            subs.push(
                Subscription::run_with_id(("agente", self.conversa), conversa).map(Msg::Agente),
            );
        }
        subs.push(self.terminais.subscription());
        Subscription::batch(subs)
    }

    fn enviar(&mut self, linha: String) {
        match &self.canal {
            Some(c) if c.send(linha.clone()).is_ok() => {}
            _ => self.fila.push(linha),
        }
    }

    fn controle(&mut self, pedido: serde_json::Value) {
        self.proximo_controle += 1;
        let linha = agente::controle(self.proximo_controle, pedido);
        self.enviar(linha);
    }

    fn focar_terminal(&self) -> Task<Msg> {
        self.terminais.ativo().map_or_else(Task::none, |t| {
            iced_term::TerminalView::focus(t.widget_id())
        })
    }

    fn tema_codigo(&self) -> highlighter::Theme {
        match self
            .config
            .tema_codigo
            .and_then(|i| highlighter::Theme::ALL.get(i))
        {
            Some(t) => *t,
            None if tema::escuro() => highlighter::Theme::Base16Ocean,
            None => highlighter::Theme::InspiredGitHub,
        }
    }

    fn aba(&self) -> Option<&Aba> {
        self.abas.get(self.ativa)
    }

    fn relativo<'a>(&self, p: &'a Path) -> &'a Path {
        p.strip_prefix(&self.arvore.raiz).unwrap_or(p)
    }

    /// O que o claude precisa saber do editor junto com o pedido: arquivo, linha e seleção.
    fn contexto(&self) -> String {
        let Some(aba) = self.aba() else {
            return String::new();
        };
        let (linha, _) = aba.conteudo.cursor_position();
        let mut s = format!(
            "\n\n<contexto_do_editor>\nArquivo aberto no FeatherCode: {} (cursor na linha {})",
            self.relativo(&aba.caminho).display(),
            linha + 1
        );
        if let Some(sel) = aba.conteudo.selection().filter(|s| !s.trim().is_empty()) {
            // shortcut: seleção cortada em 20 mil caracteres; mandar como anexo se fizer falta.
            let sel: String = sel.chars().take(20_000).collect();
            s.push_str(&format!("\nTrecho selecionado:\n```\n{sel}\n```"));
        }
        s.push_str("\n</contexto_do_editor>");
        s
    }

    /// Recarrega do disco as abas sem mudança local que o agente alterou.
    fn recarregar_abas(&mut self) {
        for aba in self.abas.iter_mut().filter(|a| !a.sujo) {
            // shortcut: aba com mudança não salva fica como está; avisar do conflito quando incomodar.
            if let Ok(t) = std::fs::read_to_string(&aba.caminho) {
                if t.trim_end() != aba.conteudo.text().trim_end() {
                    aba.maior = maior_linha(&t);
                    aba.conteudo = text_editor::Content::with_text(&t);
                }
            }
        }
    }

    fn sinal(&mut self, s: Sinal) {
        match s {
            Sinal::Item(i) => self.itens.push(i),
            Sinal::Resultado { id, erro } => {
                for item in self.itens.iter_mut().rev() {
                    if let Item::Ferramenta { id: i, erro: e, .. } = item {
                        if *i == id {
                            *e = erro;
                            break;
                        }
                    }
                }
                self.recarregar_abas();
            }
            Sinal::Uso(cinco, sete) => self.uso = (cinco.or(self.uso.0), sete.or(self.uso.1)),
            Sinal::Inicio(i) => self.info = i,
            Sinal::Contexto(n) => self.contexto.0 = n,
            Sinal::JanelaContexto(n) => self.contexto.1 = n,
            Sinal::FimDoTurno(erro) => {
                self.ocupado = false;
                self.itens.extend(erro.map(Item::Aviso));
                self.arvore.recarregar();
                self.ramo = ramo(&self.arvore.raiz);
                self.recarregar_abas();
            }
        }
    }

    fn abrir_pasta(&mut self, p: PathBuf) -> Task<Msg> {
        if self.abas.iter().any(|a| a.sujo) {
            self.aviso = "Salve ou feche as abas com mudanças antes de trocar de pasta.".into();
            return Task::none();
        }
        if !p.is_dir() {
            self.aviso = format!("A pasta {} não existe mais.", p.display());
            self.config.pastas.retain(|x| x.caminho != p);
            self.atualizar_recentes();
            self.config.gravar();
            return Task::none();
        }
        self.config.usar_pasta(p.clone());
        self.ramo = ramo(&p);
        // Os terminais da pasta anterior fecham; um shell novo já abre na pasta nova.
        self.terminais = terminal::Terminais::default();
        if let Err(e) = self.terminais.novo(&p, None) {
            self.aviso = e;
        }
        self.arvore = arvore::Arvore::nova(p);
        self.abas.clear();
        self.ativa = 0;
        self.inicio = false;
        self.aviso.clear();
        self.achados = None;
        self.filtro.clear();
        self.todos_arquivos.clear();
        self.git = None;
        self.update(Msg::NovaConversa)
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        // Escolher qualquer coisa fecha o menu; o que chega sozinho (agente, git, busca) não.
        if !matches!(
            msg,
            Msg::Menu(_)
                | Msg::Janela(_)
                | Msg::Capturar
                | Msg::Capturado(_)
                | Msg::Agente(_)
                | Msg::Tamanho(_)
                | Msg::Term(_)
                | Msg::Git(_)
                | Msg::Achados(_)
                | Msg::Ini(inicio::Ini::ClaudeVersao(_) | inicio::Ini::OutrosAgentes(_))
        ) {
            self.menu = None;
        }
        match msg {
            Msg::Menu(m) => self.menu = m,
            Msg::Filtro(f) => {
                if self.todos_arquivos.is_empty() && !f.trim().is_empty() {
                    self.todos_arquivos = paineis::arquivos(&self.arvore.raiz);
                }
                self.filtro = f;
            }
            Msg::ProcurarArquivo => {
                self.lado = Some(Lado::Explorador);
                return text_input::focus(text_input::Id::new("filtro"));
            }
            Msg::Recuar(mais) => {
                let Some(aba) = self.abas.get_mut(self.ativa) else {
                    return Task::none();
                };
                // Tab sem seleção só põe espaços no cursor, como em qualquer editor.
                if mais && aba.conteudo.selection().is_none() {
                    for _ in 0..4 {
                        aba.conteudo
                            .perform(text_editor::Action::Edit(text_editor::Edit::Insert(' ')));
                    }
                } else {
                    let (ini, fim) = edicao::linhas_da_selecao(&aba.conteudo);
                    let (_, col) = aba.conteudo.cursor_position();
                    let novo = edicao::recuar(&aba.conteudo.text(), ini, fim, mais);
                    let col = if mais { col + 4 } else { col.saturating_sub(4) };
                    edicao::trocar_texto(&mut aba.conteudo, &novo, ini, fim, col);
                }
                aba.sujo = true;
                aba.maior = maior_linha(&aba.conteudo.text());
            }
            Msg::Comentar => {
                let Some(aba) = self.abas.get_mut(self.ativa) else {
                    return Task::none();
                };
                let Some(marca) = edicao::marca_comentario(&aba.caminho) else {
                    self.aviso = "Não sei comentar este tipo de arquivo.".into();
                    return Task::none();
                };
                let (ini, fim) = edicao::linhas_da_selecao(&aba.conteudo);
                let (_, col) = aba.conteudo.cursor_position();
                let novo = edicao::comentar(&aba.conteudo.text(), ini, fim, marca);
                edicao::trocar_texto(&mut aba.conteudo, &novo, ini, fim, col);
                aba.sujo = true;
                aba.maior = maior_linha(&aba.conteudo.text());
            }
            Msg::Lado(l) => {
                self.lado = if self.lado == Some(l) && l != Lado::Buscar {
                    None
                } else {
                    Some(l)
                };
                match self.lado {
                    Some(Lado::Buscar) => return text_input::focus(text_input::Id::new("busca")),
                    Some(Lado::Git) => return self.update(Msg::GitAtualizar),
                    _ => {}
                }
            }
            Msg::AlternarAgente => self.agente_aberto = !self.agente_aberto,
            Msg::AlternarTerminal => self.terminal_aberto = !self.terminal_aberto,
            Msg::TemaCodigo(i) => {
                self.config.tema_codigo = Some(i);
                self.config.gravar();
            }
            Msg::Busca(b) => self.busca = b,
            Msg::BuscarAgora => {
                if self.busca.trim().is_empty() || self.buscando {
                    return Task::none();
                }
                self.buscando = true;
                let (raiz, termo) = (self.arvore.raiz.clone(), self.busca.trim().to_string());
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || paineis::buscar(&raiz, &termo))
                            .await
                            .unwrap_or_default()
                    },
                    Msg::Achados,
                );
            }
            Msg::Achados(v) => {
                self.buscando = false;
                self.achados = Some(v);
            }
            Msg::IrPara(p, linha) => {
                let abrir = self.update(Msg::AbrirArquivo(p));
                if let Some(aba) = self.abas.get_mut(self.ativa) {
                    aba.conteudo.perform(text_editor::Action::Move(
                        text_editor::Motion::DocumentStart,
                    ));
                    for _ in 0..linha {
                        aba.conteudo
                            .perform(text_editor::Action::Move(text_editor::Motion::Down));
                    }
                }
                let y = (linha as f32 * ALTURA_LINHA - 120.0).max(0.0);
                return Task::batch([
                    abrir,
                    scrollable::scroll_to(
                        scrollable::Id::new("editor"),
                        scrollable::AbsoluteOffset { x: 0.0, y },
                    ),
                ]);
            }
            Msg::GitAtualizar => {
                return Task::perform(paineis::status_git(self.arvore.raiz.clone()), Msg::Git)
            }
            Msg::Git(r) => self.git = Some(r),
            Msg::AbrirPasta => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Abrir pasta")
                        .pick_folder(),
                    |h| Msg::PastaEscolhida(h.map(|h| h.path().to_path_buf())),
                );
            }
            Msg::PastaEscolhida(Some(p)) => return self.abrir_pasta(p),
            Msg::Ini(m) => return self.atualizar_inicio(m),
            Msg::IrInicio => {
                if self.abas.iter().any(|a| a.sujo) {
                    self.aviso =
                        "Salve ou feche as abas com mudanças antes de voltar ao início.".into();
                } else {
                    self.inicio = true;
                    self.atualizar_recentes();
                }
            }
            Msg::PastaEscolhida(None) => {}
            Msg::Alternar(p) => self.arvore.alternar(&p),
            Msg::AbrirArquivo(p) => {
                if let Some(i) = self.abas.iter().position(|a| a.caminho == p) {
                    self.ativa = i;
                } else {
                    match std::fs::read_to_string(&p) {
                        Ok(t) => {
                            self.config.usar_arquivo(p.clone());
                            self.abas.push(Aba {
                                caminho: p,
                                maior: maior_linha(&t),
                                conteudo: text_editor::Content::with_text(&t),
                                sujo: false,
                            });
                            self.ativa = self.abas.len() - 1;
                            self.aviso.clear();
                        }
                        Err(e) => {
                            self.aviso = format!(
                                "Não deu para abrir {} como texto: {e}",
                                self.relativo(&p).display()
                            )
                        }
                    }
                }
            }
            // O text_editor fica com a altura toda do texto; a roda do mouse vira rolagem do scrollable de fora.
            Msg::Editar(text_editor::Action::Scroll { lines }) => {
                return scrollable::scroll_by(
                    scrollable::Id::new("editor"),
                    scrollable::AbsoluteOffset {
                        x: 0.0,
                        y: lines as f32 * ALTURA_LINHA,
                    },
                );
            }
            Msg::Editar(acao) => {
                if let Some(aba) = self.abas.get_mut(self.ativa) {
                    let editou = acao.is_edit();
                    aba.conteudo.perform(acao);
                    if editou {
                        aba.sujo = true;
                        aba.maior = maior_linha(&aba.conteudo.text());
                    }
                }
            }
            Msg::TrocarAba(i) => self.ativa = i,
            Msg::FecharAba(i) if i >= self.abas.len() => {}
            Msg::FecharAba(i) => {
                // shortcut: fecha sem perguntar mesmo com mudança; pedir confirmação no diálogo da suíte depois.
                self.abas.remove(i);
                self.ativa = self.ativa.min(self.abas.len().saturating_sub(1));
            }
            Msg::Salvar => {
                if let Some(aba) = self.abas.get_mut(self.ativa) {
                    match std::fs::write(&aba.caminho, aba.conteudo.text()) {
                        Ok(()) => {
                            aba.sujo = false;
                            self.aviso.clear();
                        }
                        Err(e) => self.aviso = format!("Não deu para salvar: {e}"),
                    }
                }
            }
            Msg::Pedido(t) => self.pedido = t,
            Msg::Enviar => {
                let pedido = self.pedido.trim().to_string();
                if pedido.is_empty() || self.ocupado {
                    return Task::none();
                }
                let linha = agente::mensagem_usuario(&format!("{pedido}{}", self.contexto()));
                self.itens.push(Item::Eu(pedido));
                self.pedido.clear();
                self.ocupado = true;
                self.ligado = true;
                self.enviar(linha);
            }
            Msg::Parar => self.controle(serde_json::json!({"subtype": "interrupt"})),
            Msg::NovaConversa => {
                self.conversa += 1;
                self.ligado = false;
                self.canal = None;
                self.fila.clear();
                self.itens.clear();
                self.ocupado = false;
                self.contexto = (0, 0);
                self.retomar = None;
                self.historico = None;
            }
            Msg::Detalhes => self.detalhes = !self.detalhes,
            Msg::Historico => {
                self.historico = match self.historico {
                    Some(_) => None,
                    None => Some(agente::sessoes(&self.arvore.raiz)),
                };
            }
            Msg::Retomar(id) => {
                let _ = self.update(Msg::NovaConversa);
                for s in agente::sinais_da_sessao(&self.arvore.raiz, &id) {
                    self.sinal(s);
                }
                self.ocupado = false;
                self.retomar = Some(id);
            }
            Msg::Modo(m) => {
                self.modo = m;
                if self.canal.is_some() {
                    self.controle(
                        serde_json::json!({"subtype": "set_permission_mode", "mode": m.id()}),
                    );
                }
            }
            Msg::Agente(agente::Evento::Pronto(canal)) => {
                for linha in self.fila.drain(..) {
                    let _ = canal.send(linha);
                }
                self.canal = Some(canal);
            }
            Msg::Agente(agente::Evento::Mensagem(v)) => {
                for s in agente::interpretar(&v) {
                    self.sinal(s);
                }
            }
            Msg::Agente(agente::Evento::Encerrado(motivo)) => {
                self.itens.extend(motivo.map(Item::Aviso));
                self.ligado = false;
                self.canal = None;
                self.ocupado = false;
                self.conversa += 1;
            }
            Msg::Responder(i, r) => {
                let Some(Item::Pedido {
                    request_id,
                    input,
                    sugestoes,
                    resposta,
                    ..
                }) = self.itens.get_mut(i)
                else {
                    return Task::none();
                };
                if *resposta != Resposta::Pendente {
                    return Task::none();
                }
                *resposta = r;
                let linha = match r {
                    Resposta::Gravou => agente::permitir(request_id, input, None),
                    Resposta::Sempre => agente::permitir(request_id, input, Some(sugestoes)),
                    _ => agente::negar(request_id),
                };
                self.enviar(linha);
            }
            Msg::FocarTerminal => {
                self.terminal_aberto = true;
                if let Some(t) = self.terminais.ativo() {
                    return iced_term::TerminalView::focus(t.widget_id());
                }
                return self.update(Msg::NovoTerminal);
            }
            Msg::NovoTerminal => {
                self.terminal_aberto = true;
                match self.terminais.novo(&self.arvore.raiz, None) {
                    Ok(_) => return self.focar_terminal(),
                    Err(e) => self.aviso = e,
                }
            }
            Msg::FecharTerminal(id) => self.terminais.fechar(id),
            Msg::TrocarTerminal(i) => {
                self.terminais.ativa = i;
                return self.focar_terminal();
            }
            Msg::Term(ev) => self.terminais.tratar(ev),
            Msg::RodarProjeto => {
                let arquivo = self.aba().map(|a| a.caminho.clone());
                match execucao::comando_do_projeto(&self.arvore.raiz, arquivo.as_deref()) {
                    Some(cmd) => {
                        self.terminal_aberto = true;
                        match self.terminais.rodar(&self.arvore.raiz, &cmd) {
                            Ok(_) => return self.focar_terminal(),
                            Err(e) => self.aviso = e,
                        }
                    }
                    None => {
                        self.aviso =
                            "Não sei como rodar esta pasta. Digite o comando no terminal.".into()
                    }
                }
            }
            Msg::Janela(id) => {
                self.janela = id;
                if std::env::var_os("FEATHERCODE_CAPTURAR").is_some() {
                    return Task::perform(
                        tokio::time::sleep(std::time::Duration::from_millis(900)),
                        |_| Msg::Capturar,
                    );
                }
            }
            Msg::Link(url) => {
                #[cfg(target_os = "linux")]
                let abrir = std::process::Command::new("xdg-open")
                    .arg(url.as_str())
                    .spawn();
                #[cfg(target_os = "windows")]
                let abrir = std::process::Command::new("cmd")
                    .args(["/C", "start", "", url.as_str()])
                    .spawn();
                #[cfg(target_os = "macos")]
                let abrir = std::process::Command::new("open").arg(url.as_str()).spawn();
                if let Err(e) = abrir {
                    self.aviso = format!("Não deu para abrir o link: {e}");
                }
            }
            Msg::Capturar => {
                return self
                    .janela
                    .map_or_else(Task::none, |id| window::screenshot(id).map(Msg::Capturado))
            }
            Msg::Capturado(foto) => {
                if let Some(caminho) = std::env::var_os("FEATHERCODE_CAPTURAR") {
                    if let Err(e) = image::save_buffer(
                        &caminho,
                        &foto.bytes,
                        foto.size.width,
                        foto.size.height,
                        image::ColorType::Rgba8,
                    ) {
                        eprintln!("[captura] {e}");
                    }
                }
                return self.janela.map_or_else(Task::none, window::close);
            }
            Msg::Tamanho(s) => self.tamanho = s,
            Msg::Arrastar => {
                let Some(id) = self.janela else {
                    return Task::none();
                };
                return feather_ui::janelas::arrastar_ou_maximizar(
                    id,
                    &mut self.ultimo_clique_barra,
                );
            }
            Msg::Minimizar => {
                return self
                    .janela
                    .map_or_else(Task::none, |id| window::minimize(id, true))
            }
            Msg::Maximizar => return self.janela.map_or_else(Task::none, window::toggle_maximize),
            Msg::Fechar => return self.janela.map_or_else(Task::none, window::close),
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Msg> {
        // Na página inicial não há pasta aberta: nada de nome nem botão "Início".
        let pasta = if self.inicio {
            String::new()
        } else {
            self.arvore
                .raiz
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        let voltar: Element<Msg> = if self.inicio {
            Space::with_width(0).into()
        } else {
            botao_texto(
                row![icone_traco("layout-grid", 14.0), text("Início").size(12)]
                    .spacing(6)
                    .align_y(Alignment::Center),
                Msg::IrInicio,
            )
        };
        let nome = container(
            row![
                container(
                    row![pena(27.0), text("FeatherCode").size(13).font(NEGRITO)]
                        .spacing(10)
                        .align_y(Alignment::Center)
                )
                .width(paineis::LARGURA_MARCA),
                self.menus_view(),
                text(pasta).size(13).color(COR_TEXTO_MUTED()),
                horizontal_space(),
                voltar,
                botao_texto(
                    row![
                        icone_traco("folder-open", 14.0),
                        text("Abrir pasta").size(12)
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                    Msg::AbrirPasta
                ),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .padding([0, 12])
        .width(Length::Fill)
        .height(cromo::BARRA_ALTURA)
        .align_y(Alignment::Center)
        .style(tema::estilo_container_topo);
        let barra = cromo::barra_integrada(
            nome.into(),
            cromo::AcoesJanela {
                arrastar: Msg::Arrastar,
                minimizar: Msg::Minimizar,
                maximizar: Msg::Maximizar,
                fechar: Msg::Fechar,
            },
        );

        let janela: Element<Msg> = if self.inicio {
            column![
                barra,
                self.inicio_view(),
                linha_horizontal(),
                self.status_view()
            ]
            .into()
        } else {
            self.area_de_trabalho(barra)
        };
        let janela = match self.menu_aberto_view() {
            Some(menu) => stack![janela, menu].into(),
            None => janela,
        };
        cromo::moldura_janela(janela)
    }

    fn area_de_trabalho<'a>(&'a self, barra: Element<'a, Msg>) -> Element<'a, Msg> {
        let mut centro = column![self.abas_view(), self.editor_view()].width(Length::Fill);
        if self.terminal_aberto {
            centro = centro.push(linha_horizontal()).push(self.terminal_view());
        }
        let mut meio = row![self.trilho_view(), linha_vertical()].height(Length::Fill);
        if let Some(lado) = self.lado_view() {
            meio = meio.push(lado).push(linha_vertical());
        }
        meio = meio.push(centro);
        if self.agente_aberto {
            meio = meio.push(linha_vertical()).push(self.agente_view());
        }
        column![barra, meio, linha_horizontal(), self.status_view()].into()
    }

    fn arvore_view(&self) -> Element<'_, Msg> {
        let aberto = self.aba().map(|a| a.caminho.as_path());
        let linhas = self.arvore.linhas().into_iter().map(|l| {
            let nome = l
                .caminho
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let seta: Element<Msg> = if l.pasta {
                icone_traco(
                    if l.aberta {
                        "chevron-down"
                    } else {
                        "chevron-right"
                    },
                    12.0,
                )
                .into()
            } else {
                Space::with_width(12).into()
            };
            let ativo = aberto == Some(l.caminho.as_path());
            let msg = if l.pasta {
                Msg::Alternar(l.caminho)
            } else {
                Msg::AbrirArquivo(l.caminho)
            };
            button(
                row![
                    Space::with_width(l.nivel as f32 * 14.0),
                    seta,
                    text(nome).size(13)
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .on_press(msg)
            .width(Length::Fill)
            .padding([3, 12])
            .style(move |_, s| item_lista(ativo, s, 0.0))
            .into()
        });
        let titulo = text(
            self.arvore
                .raiz
                .file_name()
                .map(|n| n.to_string_lossy().to_uppercase())
                .unwrap_or_default(),
        )
        .size(11)
        .font(NEGRITO)
        .color(COR_TEXTO_MUTED());
        let filtro = text_input("Procurar arquivo", &self.filtro)
            .id(text_input::Id::new("filtro"))
            .on_input(Msg::Filtro)
            .size(12)
            .padding([5, 8])
            .style(|t, s| text_input::Style {
                border: Border {
                    color: if matches!(s, text_input::Status::Focused) {
                        COR_AZUL_ACENTO()
                    } else {
                        COR_BORDA()
                    },
                    width: 1.0,
                    radius: 6.0.into(),
                },
                background: COR_PAINEL().into(),
                ..campo_sem_borda(t, s)
            });
        // Com o filtro, a lista vira os arquivos de todas as pastas cujo nome contém o texto.
        let lista: Element<Msg> = if self.filtro.trim().is_empty() {
            Column::with_children(linhas).into()
        } else {
            let termo = self.filtro.trim().to_lowercase();
            let achados = self.todos_arquivos.iter().filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().to_lowercase().contains(&termo))
            });
            let itens: Vec<Element<Msg>> = achados
                .take(300)
                .map(|p| {
                    let rel = self.relativo(p);
                    let pai = rel
                        .parent()
                        .map(|x| x.display().to_string())
                        .unwrap_or_default();
                    let nome = rel
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let ativo = aberto == Some(p.as_path());
                    button(column![
                        text(nome).size(13),
                        text(pai).size(11).color(COR_TEXTO_MUTED())
                    ])
                    .on_press(Msg::AbrirArquivo(p.clone()))
                    .width(Length::Fill)
                    .padding([3, 12])
                    .style(move |_, s| item_lista(ativo, s, 0.0))
                    .into()
                })
                .collect();
            if itens.is_empty() {
                container(
                    text("Nenhum arquivo com esse nome.")
                        .size(12)
                        .color(COR_TEXTO_MUTED()),
                )
                .padding([4, 12])
                .into()
            } else {
                Column::with_children(itens).into()
            }
        };
        container(column![
            container(column![titulo, filtro].spacing(8)).padding([10, 12]),
            tema::rolagem(lista)
                .height(Length::Fill)
                .style(rolagem_discreta)
        ])
        .width(LARGURA_ARVORE)
        .height(Length::Fill)
        .style(|_| fundo(COR_PAINEL_FAIXA()))
        .into()
    }

    fn abas_view(&self) -> Element<'_, Msg> {
        let abas = self.abas.iter().enumerate().map(|(i, a)| {
            let ativa = i == self.ativa;
            let nome = a
                .caminho
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let fechar = button(
                text(if a.sujo { "●" } else { "×" })
                    .size(13)
                    .color(COR_TEXTO_MUTED()),
            )
            .on_press(Msg::FecharAba(i))
            .padding([0, 4])
            .style(|_, s| item_lista(false, s, 3.0));
            let aba = button(
                row![text(nome).size(13), fechar]
                    .spacing(8)
                    .align_y(Alignment::Center),
            )
            .on_press(Msg::TrocarAba(i))
            .padding(iced::Padding {
                top: 6.0,
                right: 8.0,
                bottom: 6.0,
                left: 14.0,
            })
            .height(Length::Fill)
            .style(move |_, s| aba_estilo(ativa, s));
            // O traço de cima marca a aba ativa, como no artifact aprovado.
            let traco = container(Space::with_height(2))
                .width(Length::Fill)
                .style(move |_| {
                    fundo(if ativa {
                        COR_AZUL_ACENTO()
                    } else {
                        Color::TRANSPARENT
                    })
                });
            row![stack![aba, traco], linha_vertical()].into()
        });
        let arquivo = self.aba().map(|a| a.caminho.as_path());
        let rodar: Element<Msg> =
            if let Some(cmd) = execucao::comando_do_projeto(&self.arvore.raiz, arquivo) {
                botao_rodar("Rodar", VERDE, Msg::RodarProjeto, format!("{cmd}  (F5)"))
            } else {
                Space::with_width(0).into()
            };
        container(
            row![
                row(abas).height(Length::Fill),
                horizontal_space(),
                container(rodar).padding([0, 10])
            ]
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .height(ALTURA_ABAS)
        .style(|_| fundo(COR_CROMO_2()))
        .into()
    }

    fn editor_view(&self) -> Element<'_, Msg> {
        let Some(aba) = self.aba() else {
            let vazio = column![
                text("Nenhum arquivo aberto").size(15).font(NEGRITO),
                text("Escolha um arquivo na árvore, ou peça ao Claude Code no painel da direita.")
                    .size(13)
                    .color(COR_TEXTO_MUTED()),
                text("Ctrl+O abre outra pasta  ·  Ctrl+S salva")
                    .size(12)
                    .color(COR_TEXTO_MUTED()),
            ]
            .spacing(8)
            .align_x(Alignment::Center);
            return container(vazio)
                .center(Length::Fill)
                .style(|_| fundo(COR_PAINEL()))
                .into();
        };
        // Mesmo desenho do editor de macros do FeatherSheet: o text_editor do Iced 0.13 não expõe a
        // rolagem, então ele ganha a altura do texto e um scrollable nosso rola a calha e o código juntos.
        let total = aba.conteudo.line_count().max(1);
        let (linha_cursor, _) = aba.conteudo.cursor_position();
        let calha_px = total.to_string().len().max(2) as f32 * LARGURA_CARACTERE + 28.0;
        let visivel_x = self.tamanho.width
            - paineis::LARGURA_TRILHO
            - if self.lado.is_some() {
                LARGURA_ARVORE
            } else {
                0.0
            }
            - if self.agente_aberto {
                LARGURA_AGENTE
            } else {
                0.0
            }
            - calha_px
            - 6.0;
        let visivel_y = self.tamanho.height
            - cromo::BARRA_ALTURA
            - ALTURA_ABAS
            - if self.terminal_aberto {
                ALTURA_TERMINAL
            } else {
                0.0
            }
            - ALTURA_STATUS
            - 4.0;
        let largura = (aba.maior as f32 * LARGURA_CARACTERE + 2.0 * MARGEM_H + 32.0).max(visivel_x);
        let altura = (total as f32 * ALTURA_LINHA + 2.0 * MARGEM_V).max(visivel_y);
        let extensao = aba
            .caminho
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt");
        let cores = self.tema_codigo();
        let editor = text_editor(&aba.conteudo)
            .on_action(Msg::Editar)
            .key_binding(|k| {
                use text_editor::Binding;
                if k.status != text_editor::Status::Focused {
                    return None;
                }
                match k.key.as_ref() {
                    keyboard::Key::Named(keyboard::key::Named::Tab) => {
                        Some(Binding::Custom(Msg::Recuar(!k.modifiers.shift())))
                    }
                    keyboard::Key::Character(";") if k.modifiers.command() => {
                        Some(Binding::Custom(Msg::Comentar))
                    }
                    _ => Binding::from_key_press(k),
                }
            })
            .font(MONO)
            .size(TAMANHO_FONTE)
            .line_height(LineHeight::Absolute(ALTURA_LINHA.into()))
            .wrapping(Wrapping::None)
            .width(largura)
            .height(altura)
            .padding(iced::Padding {
                top: MARGEM_V,
                bottom: MARGEM_V,
                left: MARGEM_H,
                right: MARGEM_H,
            })
            .highlight(extensao, cores)
            .style(|_, _| text_editor::Style {
                background: Background::Color(COR_PAINEL()),
                border: Border::default(),
                icon: COR_TEXTO_MUTED(),
                placeholder: COR_TEXTO_MUTED(),
                value: COR_TEXTO(),
                selection: COR_AZUL_FUNDO(),
            });
        let horizontal = scrollable(editor)
            .style(rolagem_discreta)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new().width(6).scroller_width(6),
            ))
            .width(Length::Fill);
        let conteudo = row![calha(total, linha_cursor + 1, calha_px, altura), horizontal];
        container(
            tema::rolagem(conteudo)
                .id(scrollable::Id::new("editor"))
                .height(Length::Fill)
                .width(Length::Fill)
                .style(rolagem_discreta),
        )
        .style(|_| fundo(COR_PAINEL()))
        .into()
    }

    fn agente_view(&self) -> Element<'_, Msg> {
        let titulo = self
            .itens
            .iter()
            .find_map(|i| {
                if let Item::Eu(t) = i {
                    Some(t.chars().take(38).collect::<String>())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "Nova conversa".into());
        let topo = container(
            row![
                text(titulo).size(13).font(NEGRITO),
                horizontal_space(),
                botao_icone("history", Msg::Historico),
                botao_icone("info-circle", Msg::Detalhes),
                botao_icone("plus", Msg::NovaConversa),
            ]
            .spacing(2)
            .align_y(Alignment::Center),
        )
        .padding(iced::Padding {
            top: 0.0,
            right: 8.0,
            bottom: 0.0,
            left: 14.0,
        })
        .height(ALTURA_ABAS)
        .align_y(Alignment::Center)
        .style(|_| fundo(COR_CROMO_2()));

        let mut corpo = Column::new().spacing(12).padding([14, 14]);
        if self.detalhes {
            corpo = corpo.push(self.detalhes_view());
        }
        if let Some(lista) = &self.historico {
            corpo = corpo.push(historico_view(lista));
        } else if self.itens.is_empty() {
            corpo = corpo.push(
                column![
                    text("Peça uma mudança ou pergunte sobre o código.").size(13),
                    text("O Claude Code roda na pasta aberta, com a sua conta. Antes de gravar um arquivo ou rodar um comando, ele pede permissão aqui.").size(12).color(COR_TEXTO_MUTED()),
                ]
                .spacing(6),
            );
        }
        // Passos seguidos de ferramenta ficam juntos numa linha do tempo, com o traço à esquerda.
        let mut passos: Vec<Element<Msg>> = Vec::new();
        for (i, item) in self.itens.iter().enumerate() {
            if let Item::Ferramenta { frase, erro, .. } = item {
                passos.push(passo(frase, *erro));
                continue;
            }
            if !passos.is_empty() {
                corpo = corpo.push(linha_do_tempo(std::mem::take(&mut passos)));
            }
            corpo = corpo.push(item_view(i, item));
        }
        if !passos.is_empty() {
            corpo = corpo.push(linha_do_tempo(passos));
        }
        if self.ocupado {
            corpo = corpo.push(text("Trabalhando…").size(12).color(COR_TEXTO_MUTED()));
        }

        let chip: Element<Msg> = match self.aba() {
            Some(a) => {
                let (l, _) = a.conteudo.cursor_position();
                container(
                    text(format!("{}:{}", self.relativo(&a.caminho).display(), l + 1))
                        .font(MONO)
                        .size(11)
                        .color(COR_TEXTO_MUTED()),
                )
                .padding([1, 6])
                .style(|_| container::Style {
                    border: Border {
                        color: COR_BORDA(),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
                .into()
            }
            None => Space::with_height(0).into(),
        };
        let acao = if self.ocupado {
            button(container(icone_traco("player-stop", 13.0)).center(22))
                .on_press(Msg::Parar)
                .padding(0)
                .style(|_, s| item_lista(false, s, 5.0))
        } else {
            button(container(text("↑").size(14).color(COR_PAINEL())).center(22))
                .on_press(Msg::Enviar)
                .padding(0)
                .style(botao_enviar)
        };
        let rodape = row![
            self.bolinha(),
            text("Claude Code").size(12).color(COR_TEXTO_MUTED()),
            pick_list(Modo::TODOS, Some(self.modo), Msg::Modo)
                .text_size(12)
                .padding([2, 4])
                .style(lista_sem_borda)
                .menu_style(tema::estilo_menu_popup),
            horizontal_space(),
            text(nome_modelo(&self.info.modelo))
                .size(12)
                .color(COR_TEXTO_MUTED()),
            self.anel(),
            acao,
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let entrada = container(column![
            container(chip).padding(iced::Padding {
                top: 8.0,
                right: 10.0,
                bottom: 0.0,
                left: 10.0
            }),
            text_input("Peça uma mudança ou pergunte…", &self.pedido)
                .on_input(Msg::Pedido)
                .on_submit(Msg::Enviar)
                .size(13)
                .padding([6, 10])
                .style(campo_sem_borda),
            linha_horizontal(),
            container(rodape).padding([5, 8]),
        ])
        .style(|_| container::Style {
            border: Border {
                color: COR_BORDA(),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..fundo(COR_PAINEL())
        });

        container(column![
            topo,
            linha_horizontal(),
            tema::rolagem(corpo)
                .style(rolagem_discreta)
                .anchor_bottom()
                .height(Length::Fill),
            container(entrada).padding(iced::Padding {
                top: 0.0,
                right: 10.0,
                bottom: 10.0,
                left: 10.0
            }),
        ])
        .width(LARGURA_AGENTE)
        .height(Length::Fill)
        .style(|_| fundo(COR_PAINEL_FAIXA()))
        .into()
    }

    /// Estado do agente na bolinha ao lado do nome, com a frase na dica.
    fn estado(&self) -> (Color, &'static str) {
        let esperando = self.itens.iter().any(|i| {
            matches!(
                i,
                Item::Pedido {
                    resposta: Resposta::Pendente,
                    ..
                }
            )
        });
        match (self.ligado, self.ocupado) {
            (false, _) if matches!(self.itens.last(), Some(Item::Aviso(_))) => {
                (COR_ERRO(), "Parou com erro")
            }
            (false, _) => (COR_TEXTO_MUTED(), "Desligado: abre na primeira mensagem"),
            (true, _) if esperando => (AMARELO, "Esperando a sua permissão"),
            (true, true) => (COR_AZUL_ACENTO(), "Trabalhando"),
            (true, false) => (VERDE, "Pronto"),
        }
    }

    fn bolinha(&self) -> Element<'_, Msg> {
        let (cor, frase) = self.estado();
        tooltip(
            text("●").size(10).color(cor),
            container(text(frase).size(11))
                .padding([3, 8])
                .style(tema::estilo_dica),
            tooltip::Position::Top,
        )
        .into()
    }

    /// Anel do contexto usado, como no app do Claude; a dica mostra contexto, sessão e semana.
    fn anel(&self) -> Element<'_, Msg> {
        let (usados, janela) = self.contexto;
        let fracao = if janela > 0 {
            usados as f32 / janela as f32
        } else {
            0.0
        };
        let desenho = iced::widget::canvas(Anel(fracao)).width(16).height(16);
        let dica: Element<Msg> = if usados == 0 && self.uso.0.is_none() {
            text("O uso aparece depois da primeira resposta.")
                .size(11)
                .into()
        } else {
            container(self.barras_view()).width(240).into()
        };
        tooltip(
            desenho,
            container(dica).padding([8, 10]).style(tema::estilo_dica),
            tooltip::Position::Top,
        )
        .into()
    }

    fn barras_view(&self) -> Element<'_, Msg> {
        let mut c = Column::new().spacing(8);
        let (usados, janela) = self.contexto;
        if usados > 0 {
            let fracao = if janela > 0 {
                usados as f32 / janela as f32
            } else {
                0.0
            };
            let valor = if janela > 0 {
                format!("{} de {}", tokens(usados), tokens(janela))
            } else {
                tokens(usados)
            };
            c = c.push(barra("Contexto", fracao, valor));
        }
        if let Some(u) = self.uso.0 {
            c = c.push(barra("Sessão (5 h)", u, format!("{:.0}%", u * 100.0)));
        }
        if let Some(u) = self.uso.1 {
            c = c.push(barra("Semana (7 dias)", u, format!("{:.0}%", u * 100.0)));
        }
        c.into()
    }

    fn detalhes_view(&self) -> Element<'_, Msg> {
        let i = &self.info;
        let rotulo = |t: &'static str| text(t).size(11).font(NEGRITO).color(COR_TEXTO_MUTED());
        let mut c = Column::new().spacing(10).push(self.barras_view());
        if i.versao.is_empty() {
            c = c.push(
                text(
                    "Os detalhes aparecem depois da primeira mensagem, quando o Claude Code abre.",
                )
                .size(12)
                .color(COR_TEXTO_MUTED()),
            );
        } else {
            c = c.push(text(format!("{} · Claude Code {}", i.modelo, i.versao)).size(12));
            let mcp = i.mcp.iter().map(|(nome, estado)| {
                let (cor, frase) = match estado.as_str() {
                    "connected" => (VERDE, "conectado"),
                    "pending" => (AMARELO, "conectando"),
                    "needs-auth" => (AMARELO, "precisa de login"),
                    "failed" => (COR_ERRO(), "falhou"),
                    outro => (COR_TEXTO_MUTED(), outro),
                };
                row![
                    text("●").size(8).color(cor),
                    text(nome).size(12),
                    horizontal_space(),
                    text(frase).size(11).color(COR_TEXTO_MUTED())
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into()
            });
            c = c.push(
                column![
                    rotulo("SERVIDORES MCP"),
                    Column::with_children(mcp).spacing(3)
                ]
                .spacing(4),
            );
            c = c.push(
                column![
                    rotulo("PLUGINS"),
                    text(if i.plugins.is_empty() {
                        "Nenhum".into()
                    } else {
                        i.plugins.join(", ")
                    })
                    .size(12)
                ]
                .spacing(4),
            );
            c = c.push(
                text(format!(
                    "{} skills · {} comandos · agentes: {}",
                    i.skills,
                    i.comandos,
                    i.agentes.join(", ")
                ))
                .size(12)
                .color(COR_TEXTO_MUTED()),
            );
        }
        container(c)
            .padding(10)
            .width(Length::Fill)
            .style(|_| container::Style {
                border: Border {
                    color: COR_BORDA(),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..fundo(COR_PAINEL())
            })
            .into()
    }

    fn status_view(&self) -> Element<'_, Msg> {
        let (pos, lingua) = match self.aba() {
            Some(a) => {
                let (l, c) = a.conteudo.cursor_position();
                (
                    format!("Ln {}, Col {}", l + 1, c + 1),
                    linguagem(&a.caminho),
                )
            }
            None => (String::new(), String::new()),
        };
        let esquerda: Element<Msg> = if self.aviso.is_empty() {
            text(if self.inicio {
                String::new()
            } else {
                self.ramo.clone().unwrap_or_default()
            })
            .size(11)
            .into()
        } else {
            text(&self.aviso).size(11).color(COR_ERRO()).into()
        };
        container(
            row![
                esquerda,
                horizontal_space(),
                text(pos).size(11),
                text("UTF-8").size(11),
                text(lingua).size(11)
            ]
            .spacing(18)
            .align_y(Alignment::Center),
        )
        .padding([0, 12])
        .height(ALTURA_STATUS)
        .align_y(Alignment::Center)
        .style(|_| container::Style {
            text_color: Some(COR_TEXTO_MUTED()),
            ..fundo(COR_CROMO())
        })
        .into()
    }
}

fn item_view(i: usize, item: &Item) -> Element<'_, Msg> {
    match item {
        Item::Eu(t) => container(text(t).size(13))
            .padding([8, 10])
            .width(Length::Fill)
            .style(|_| container::Style {
                border: Border {
                    color: COR_BORDA(),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..fundo(COR_PAINEL())
            })
            .into(),
        // shortcut: resposta em texto puro; o widget markdown do Iced entra quando o bloco de código fizer falta.
        Item::Texto(md) => {
            let estilo = markdown::Style {
                inline_code_highlight: markdown::Highlight {
                    background: COR_HOVER().into(),
                    border: Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                },
                inline_code_padding: iced::Padding {
                    top: 0.0,
                    right: 3.0,
                    bottom: 0.0,
                    left: 3.0,
                },
                inline_code_color: COR_TEXTO(),
                link_color: COR_AZUL_ACENTO(),
            };
            markdown::view(
                md,
                markdown::Settings {
                    code_size: 12.into(),
                    ..markdown::Settings::with_text_size(13)
                },
                estilo,
            )
            .map(Msg::Link)
        }
        Item::Ferramenta { frase, erro, .. } => passo(frase, *erro),
        Item::Aviso(t) => text(t).size(12).color(COR_ERRO()).into(),
        Item::Pedido {
            frase,
            previa,
            resposta,
            ..
        } => {
            let mut cartao = Column::new().push(
                container(text(frase).size(12).font(MONO))
                    .padding([7, 10])
                    .width(Length::Fill),
            );
            if !previa.is_empty() {
                let linhas = previa.iter().map(|(s, l)| {
                    let (cor, faixa) = match s {
                        '-' => (COR_ERRO(), mistura_cor(COR_PAINEL(), COR_ERRO(), 0.18)),
                        '+' => (VERDE, mistura_cor(COR_PAINEL(), VERDE, 0.18)),
                        _ => (COR_TEXTO_MUTED(), Color::TRANSPARENT),
                    };
                    container(
                        text(format!("{s} {l}"))
                            .font(MONO)
                            .size(11)
                            .color(cor)
                            .wrapping(Wrapping::None),
                    )
                    .padding([1, 10])
                    .width(Length::Shrink)
                    .style(move |_| fundo(faixa))
                    .into()
                });
                // Linha comprida rola de lado em vez de quebrar: o diff tem que ser lido como está.
                let diff = scrollable(Column::with_children(linhas))
                    .direction(scrollable::Direction::Horizontal(
                        scrollable::Scrollbar::new().width(4).scroller_width(4),
                    ))
                    .style(rolagem_discreta);
                cartao = cartao.push(linha_horizontal()).push(diff);
            }
            let fim: Element<Msg> = match resposta {
                Resposta::Pendente => column![
                    opcao("1", "Permitir", Msg::Responder(i, Resposta::Gravou), true),
                    opcao(
                        "2",
                        "Permitir e não perguntar nesta sessão",
                        Msg::Responder(i, Resposta::Sempre),
                        false
                    ),
                    opcao(
                        "3",
                        "Não, explicar o que mudar",
                        Msg::Responder(i, Resposta::Recusou),
                        false
                    ),
                ]
                .padding(6)
                .into(),
                Resposta::Recusou => container(text("Recusado").size(12).color(COR_ERRO()))
                    .padding([6, 10])
                    .into(),
                _ => container(text("Permitido").size(12).color(VERDE))
                    .padding([6, 10])
                    .into(),
            };
            let pendente = *resposta == Resposta::Pendente;
            container(cartao.push(fim))
                .width(Length::Fill)
                .style(move |_| container::Style {
                    border: Border {
                        color: if pendente {
                            COR_AZUL_ACENTO()
                        } else {
                            COR_BORDA()
                        },
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..fundo(COR_PAINEL())
                })
                .into()
        }
    }
}

fn passo(frase: &str, erro: bool) -> Element<'_, Msg> {
    row![
        text("●")
            .size(8)
            .color(if erro { COR_ERRO() } else { VERDE }),
        text(frase).size(12).color(COR_TEXTO_MUTED())
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn linha_do_tempo(passos: Vec<Element<'_, Msg>>) -> Element<'_, Msg> {
    Column::with_children(passos)
        .spacing(6)
        .padding(iced::Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 4.0,
        })
        .into()
}

/// Números das linhas: três textos (antes, a linha do cursor, depois) com a mesma métrica do editor.
fn calha<'a>(total: usize, linha_cursor: usize, largura: f32, altura: f32) -> Element<'a, Msg> {
    // Mesma largura em todos: o Iced alinha o bloco pela linha mais larga, não linha a linha.
    let largura_num = total.to_string().len();
    let numeros = |de: usize, ate: usize| {
        (de..=ate)
            .map(|n| format!("{n:>largura_num$}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let numero = |s: String, cor: Color| {
        text(s)
            .font(MONO)
            .size(TAMANHO_FONTE)
            .line_height(LineHeight::Absolute(ALTURA_LINHA.into()))
            .wrapping(Wrapping::None)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right)
            .color(cor)
    };
    let mut col = column![Space::with_height(MARGEM_V)];
    if linha_cursor > 1 {
        col = col.push(numero(numeros(1, linha_cursor - 1), COR_TEXTO_MUTED()));
    }
    col = col.push(numero(numeros(linha_cursor, linha_cursor), COR_TEXTO()));
    if linha_cursor < total {
        col = col.push(numero(numeros(linha_cursor + 1, total), COR_TEXTO_MUTED()));
    }
    container(col.padding(iced::Padding {
        top: 0.0,
        right: 12.0,
        bottom: 0.0,
        left: 4.0,
    }))
    .width(largura)
    .height(altura)
    .style(|_| fundo(COR_PAINEL()))
    .into()
}

fn opcao<'a>(tecla: &'a str, rotulo: &'a str, msg: Msg, foco: bool) -> Element<'a, Msg> {
    button(row![text(tecla).font(MONO).size(11), text(rotulo).size(12)].spacing(10))
        .on_press(msg)
        .width(Length::Fill)
        .padding([5, 8])
        .style(move |_, s| item_lista(foco, s, 4.0))
        .into()
}

/// "25 mil", "1,2 mi": tokens do jeito que se lê.
fn tokens(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{} mil", n / 1_000),
        _ => format!("{:.1} mi", n as f64 / 1e6)
            .replace(".0 ", " ")
            .replace('.', ","),
    }
}

struct Anel(f32);

impl<M> iced::widget::canvas::Program<M> for Anel {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &iced::Renderer,
        _: &iced::Theme,
        bounds: iced::Rectangle,
        _: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry> {
        use iced::widget::canvas::{path, Frame, Path, Stroke};
        let mut frame = Frame::new(renderer, bounds.size());
        let centro = frame.center();
        let raio = bounds.width.min(bounds.height) / 2.0 - 2.0;
        frame.stroke(
            &Path::circle(centro, raio),
            Stroke::default().with_width(2.5).with_color(COR_BORDA()),
        );
        let f = self.0.clamp(0.0, 1.0);
        if f > 0.0 {
            let cor = if f > 0.9 {
                COR_ERRO()
            } else if f > 0.75 {
                AMARELO
            } else {
                COR_AZUL_ACENTO()
            };
            let inicio = -std::f32::consts::FRAC_PI_2;
            let arco = Path::new(|b| {
                b.arc(path::Arc {
                    center: centro,
                    radius: raio,
                    start_angle: iced::Radians(inicio),
                    end_angle: iced::Radians(inicio + f * std::f32::consts::TAU),
                })
            });
            frame.stroke(
                &arco,
                Stroke::default()
                    .with_width(2.5)
                    .with_color(cor)
                    .with_line_cap(iced::widget::canvas::LineCap::Round),
            );
        }
        vec![frame.into_geometry()]
    }
}

/// `claude-opus-5-5[1m]` vira `Opus 5.5`.
fn nome_modelo(id: &str) -> String {
    let id = id
        .split('[')
        .next()
        .unwrap_or(id)
        .trim_start_matches("claude-");
    let mut partes = id.split('-');
    let Some(nome) = partes.next().filter(|n| !n.is_empty()) else {
        return String::new();
    };
    let versao: Vec<&str> = partes
        .take_while(|p| p.len() <= 2 && p.chars().all(|c| c.is_ascii_digit()))
        .collect();
    let mut nome: String = nome
        .chars()
        .take(1)
        .flat_map(char::to_uppercase)
        .chain(nome.chars().skip(1))
        .collect();
    if !versao.is_empty() {
        nome.push(' ');
        nome.push_str(&versao.join("."));
    }
    nome
}

/// Conversas guardadas desta pasta; clicar continua a conversa.
fn historico_view(lista: &[agente::Sessao]) -> Element<'_, Msg> {
    let agora = config::agora();
    let mut c = Column::new().spacing(2).push(
        text("CONVERSAS DESTA PASTA")
            .size(10)
            .font(NEGRITO)
            .color(COR_TEXTO_MUTED()),
    );
    if lista.is_empty() {
        c = c.push(
            text("Nenhuma conversa guardada ainda.")
                .size(12)
                .color(COR_TEXTO_MUTED()),
        );
    }
    for s in lista {
        c = c.push(
            button(
                column![
                    text(&s.titulo).size(13),
                    text(config::ha_quanto(s.quando, agora))
                        .size(11)
                        .color(COR_TEXTO_MUTED())
                ]
                .spacing(1),
            )
            .on_press(Msg::Retomar(s.id.clone()))
            .width(Length::Fill)
            .padding([6, 8])
            .style(|_, st| item_lista(false, st, 6.0)),
        );
    }
    c.into()
}

fn barra(rotulo: &str, fracao: f32, valor: String) -> Element<'_, Msg> {
    let u = fracao.clamp(0.0, 1.0);
    column![
        row![
            text(rotulo).size(11).color(COR_TEXTO_MUTED()),
            horizontal_space(),
            text(valor).size(11).color(COR_TEXTO_MUTED())
        ],
        progress_bar(0.0..=1.0, u)
            .height(4)
            .style(|_| progress_bar::Style {
                background: COR_BORDA().into(),
                bar: COR_AZUL_ACENTO().into(),
                border: Border {
                    radius: 2.0.into(),
                    ..Default::default()
                },
            }),
    ]
    .spacing(4)
    .into()
}

/// A pena da marca sem o quadrado de fundo, para ficar sobre a cor do painel.
fn pena(tamanho: f32) -> iced::widget::Svg<'static> {
    let svg = marca::svg_para(marca::produto_atual(), true);
    let sem_fundo = match (
        svg.find("<rect id=\"fundo-arredondado\""),
        svg.find("<g id=\"penas\""),
    ) {
        (Some(a), Some(b)) if a < b => format!("{}{}", &svg[..a], &svg[b..]),
        _ => svg,
    };
    iced::widget::svg(iced::widget::svg::Handle::from_memory(
        sem_fundo.into_bytes(),
    ))
    .width(tamanho)
    .height(tamanho)
}

/// Ramo atual lido do `.git/HEAD`, sem chamar o git.
fn ramo(pasta: &Path) -> Option<String> {
    let head = std::fs::read_to_string(pasta.join(".git/HEAD")).ok()?;
    Some(match head.trim().strip_prefix("ref: refs/heads/") {
        Some(r) => r.to_string(),
        None => head.trim().chars().take(7).collect(),
    })
}

fn linguagem(caminho: &Path) -> String {
    let ext = caminho
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "rs" => "Rust",
        "toml" => "TOML",
        "md" => "Markdown",
        "js" | "mjs" => "JavaScript",
        "ts" => "TypeScript",
        "py" => "Python",
        "json" => "JSON",
        "html" => "HTML",
        "css" => "CSS",
        "sh" => "Shell",
        "go" => "Go",
        "c" | "h" => "C",
        "cpp" | "hpp" => "C++",
        "" => "Texto",
        outra => return outra.to_uppercase(),
    }
    .into()
}

/// Fundo do terminal: mais escuro que o editor, para não se confundir com ele.
fn cor_terminal() -> Color {
    mistura_cor(
        COR_CROMO(),
        Color::BLACK,
        if tema::escuro() { 0.3 } else { 0.03 },
    )
}

fn botao_rodar<'a>(rotulo: &'a str, cor: Color, msg: Msg, dica: String) -> Element<'a, Msg> {
    let simbolo = text(if rotulo == "Parar" { "■" } else { "▶" })
        .size(11)
        .color(cor);
    let b = button(
        row![simbolo, text(rotulo).size(12)]
            .spacing(6)
            .align_y(Alignment::Center),
    )
    .on_press(msg)
    .padding([4, 10])
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
            color: COR_BORDA(),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    });
    tooltip(
        b,
        container(text(dica).font(MONO).size(11))
            .padding([4, 8])
            .style(|_| container::Style {
                border: Border {
                    color: COR_BORDA(),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..fundo(COR_PAINEL())
            }),
        tooltip::Position::Bottom,
    )
    .into()
}

fn mistura_cor(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: 1.0,
    }
}

fn fundo(cor: Color) -> container::Style {
    container::Style {
        background: Some(cor.into()),
        text_color: Some(COR_TEXTO()),
        ..Default::default()
    }
}

fn linha_vertical<'a>() -> Element<'a, Msg> {
    container(Space::with_width(1))
        .height(Length::Fill)
        .style(|_| fundo(COR_BORDA_SUAVE()))
        .into()
}

fn linha_horizontal<'a>() -> Element<'a, Msg> {
    container(Space::with_height(1))
        .width(Length::Fill)
        .style(|_| fundo(COR_BORDA_SUAVE()))
        .into()
}

fn botao_texto<'a>(conteudo: impl Into<Element<'a, Msg>>, msg: Msg) -> Element<'a, Msg> {
    button(conteudo)
        .on_press(msg)
        .padding([4, 8])
        .style(|_, s| item_lista(false, s, 4.0))
        .into()
}

fn botao_icone<'a>(nome: &str, msg: Msg) -> Element<'a, Msg> {
    button(container(icone_traco(nome, 14.0)).center(22))
        .on_press(msg)
        .padding(0)
        .style(|_, s| item_lista(false, s, 4.0))
        .into()
}

fn item_lista(ativo: bool, s: button::Status, raio: f32) -> button::Style {
    let fundo = match (ativo, s) {
        (true, _) => Some(COR_AZUL_FUNDO().into()),
        (_, button::Status::Hovered | button::Status::Pressed) => Some(COR_HOVER().into()),
        _ => None,
    };
    button::Style {
        background: fundo,
        text_color: if ativo {
            COR_AZUL_ACENTO()
        } else {
            COR_TEXTO()
        },
        border: Border {
            radius: raio.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn aba_estilo(ativa: bool, s: button::Status) -> button::Style {
    let fundo = if ativa {
        Some(COR_PAINEL().into())
    } else if matches!(s, button::Status::Hovered) {
        Some(COR_HOVER().into())
    } else {
        None
    };
    button::Style {
        background: fundo,
        text_color: if ativa {
            COR_TEXTO()
        } else {
            COR_TEXTO_MUTED()
        },
        ..Default::default()
    }
}

fn botao_enviar(_: &iced::Theme, s: button::Status) -> button::Style {
    let cor = if matches!(s, button::Status::Hovered) {
        mistura_cor(COR_AZUL_ACENTO(), COR_TEXTO(), 0.15)
    } else {
        COR_AZUL_ACENTO()
    };
    button::Style {
        background: Some(cor.into()),
        border: Border {
            radius: 5.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn campo_sem_borda(t: &iced::Theme, s: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Color::TRANSPARENT.into(),
        border: Border::default(),
        value: COR_TEXTO(),
        placeholder: COR_TEXTO_MUTED(),
        selection: COR_AZUL_FUNDO(),
        ..text_input::default(t, s)
    }
}

fn lista_sem_borda(_: &iced::Theme, s: pick_list::Status) -> pick_list::Style {
    pick_list::Style {
        text_color: COR_TEXTO_MUTED(),
        placeholder_color: COR_TEXTO_MUTED(),
        handle_color: COR_TEXTO_MUTED(),
        background: if matches!(s, pick_list::Status::Hovered | pick_list::Status::Opened) {
            COR_HOVER().into()
        } else {
            Color::TRANSPARENT.into()
        },
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
    }
}

/// Barra de rolagem sem trilho: só o puxador aparece, como nos editores de código.
fn rolagem_discreta(t: &iced::Theme, s: scrollable::Status) -> scrollable::Style {
    let mut estilo = scrollable::default(t, s);
    for trilho in [&mut estilo.vertical_rail, &mut estilo.horizontal_rail] {
        trilho.background = None;
        trilho.border = Border::default();
        trilho.scroller.color = COR_BORDA();
        trilho.scroller.border = Border {
            radius: 3.0.into(),
            ..Default::default()
        };
    }
    estilo
}

#[cfg(test)]
mod testes {
    #[test]
    fn nome_curto_do_modelo() {
        assert_eq!(super::nome_modelo("claude-opus-5-5[1m]"), "Opus 5.5");
        assert_eq!(
            super::nome_modelo("claude-sonnet-4-5-20250929"),
            "Sonnet 4.5"
        );
        assert_eq!(super::nome_modelo(""), "");
    }
}
