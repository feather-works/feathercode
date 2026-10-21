//! FeatherCode: editor de código da suíte FeatherWorks, com o Claude Code no painel lateral.
mod adaptadores;
mod agente;
mod arvore;
mod config;
mod edicao;
mod execucao;
mod inicio;
mod modelos;
mod paineis;
mod preferencias;
mod terminal;

use agente::{Item, Resposta, Sinal};
use feather_ui::cromo;
use feather_ui::icones::icone_traco;
use feather_ui::marca::{self, Produto};
use feather_ui::tema::{
    self, COR_AZUL_ACENTO, COR_AZUL_FUNDO, COR_BORDA, COR_BORDA_SUAVE, COR_CROMO, COR_CROMO_2,
    COR_ERRO, COR_FUNDO_JANELA, COR_HOVER, COR_PAINEL, COR_PAINEL_FAIXA, COR_TEXTO,
    COR_TEXTO_MUTED,
};
use paineis::Lado;
use pelicano::{
    button, canvas, column, container, horizontal_space, markdown, mouse_area, pick_list,
    progress_bar, row, scrollable, stack, text, text_editor, text_input, tooltip, Align, Border,
    Color, Content, CursorIcon, Direction, Element, Event, Font, Highlight, Key, Length, NamedKey,
    Padding, Point, Shadow, Size, Space, Status, Style, Subscription, Task, Theme,
    TooltipPosition, WindowCmd, WindowId,
};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::UnboundedSender;

const NEGRITO: Font = Font::SEMIBOLD;
const MONO: Font = Font::MONO;
/// Índice de "Roxo" em `tema::ACENTOS`: a cor do FeatherCode.
const ROXO: usize = 5;
const AMARELO: Color = Color::rgb(0.9, 0.7, 0.25);
const VERDE: Color = Color::rgb(0.42, 0.75, 0.5);
const LARGURA_ARVORE: f32 = 240.0;
const LARGURA_AGENTE: f32 = 380.0;
const ALTURA_ABAS: f32 = 36.0;
const ALTURA_TERMINAL: f32 = 190.0;
/// Vão entre os cartões dos painéis.
const VAO: f32 = 6.0;
const ALTURA_STATUS: f32 = 24.0;
const TAMANHO_FONTE: f32 = 13.0;
const ALTURA_LINHA: f32 = 20.0;
const MARGEM_V: f32 = 8.0;
const MARGEM_H: f32 = 12.0;
/// Tom das linhas alternadas. O pelicano mistura as cores em espaço linear, onde um pouco de
/// claro sobre o fundo escuro já aparece muito: 1% dá uns 6 tons de diferença.
const ALFA_LISTRAS: f32 = 0.01;

fn main() -> Result<(), pelicano::Error> {
    let config = config::Config::ler();
    tema::definir_tema(config.tema.escuro(), ROXO);
    // Pena da suíte na cor de destaque do FeatherCode.
    marca::definir_marca(Produto::FeatherSheet, true);
    let janela = marca::preparar_janela(pelicano::Settings {
        title: "FeatherCode".into(),
        size: Size::new(1380.0, 860.0),
        min_size: Some(Size::new(960.0, 560.0)),
        // A barra de título é desenhada pelo app, como no resto da suíte.
        decorations: false,
        app_id: Some("feathercode".into()),
        ..Default::default()
    });
    pelicano::run(janela, move || Code::novo(config))
}

struct Aba {
    caminho: PathBuf,
    conteudo: Content,
    /// `Content::version` da última vez que o arquivo foi lido ou salvo.
    salvo: u64,
}

impl Aba {
    fn sujo(&self) -> bool {
        self.conteudo.version() != self.salvo
    }
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
            Modo::Perguntar => "Perguntar antes",
            Modo::AceitarEdicoes => "Aceitar edições",
            Modo::Planejar => "Só planejar",
        })
    }
}

#[derive(Debug, Clone)]
enum Msg {
    AbrirPasta,
    EscolherArquivo,
    ArquivoEscolhido(Option<PathBuf>),
    SalvarTudo,
    AtualizarArvore,
    RecolherArvore,
    PastaEscolhida(Option<PathBuf>),
    Alternar(PathBuf),
    AbrirArquivo(PathBuf),
    TrocarAba(usize),
    FecharAba(usize),
    Salvar,
    /// Põe um texto no campo do chat (clique numa mensagem antiga).
    Pedido(String),
    /// Botão de clipe: escolhe imagens para mandar junto com o pedido.
    Anexar,
    Anexadas(Vec<PathBuf>),
    TirarImagem(usize),
    Enviar,
    Parar,
    NovaConversa,
    TrocarChat(u64),
    FecharChat(u64),
    EventoChat(u64, agente::Evento),
    Modo(Modo),
    Provedor(agente::Provedor),
    Agente(agente::Evento),
    Responder(usize, Resposta),
    /// Teclas 1, 2 e 3: respondem o pedido de permissão que está esperando.
    ResponderPendente(Resposta),
    Copiar(String),
    FocarTerminal,
    RodarProjeto,
    Term(u64, pelicano::TerminalEvent),
    NovoTerminal,
    FecharTerminal(u64),
    TrocarTerminal(usize),
    Tamanho(Size),
    Capturar,
    Detalhes,
    Historico,
    Retomar(String),
    IrInicio,
    Ini(inicio::Ini),
    Link(String),
    Arrastar,
    Minimizar,
    Maximizar,
    Fechar,
    Lado(Lado),
    AlternarAgente,
    AlternarTerminal,
    AlternarLinhas,
    Busca(String),
    BuscarAgora,
    Achados(Vec<paineis::Achado>),
    IrPara(PathBuf, usize),
    GitAtualizar,
    VerDiff(PathBuf),
    Diff(Result<paineis::Diff, String>),
    FecharDiff,
    /// Clique num commit do gráfico: abre ou fecha a lista de arquivos dele.
    AbrirCommit(String),
    ArquivosCommit(Result<(String, Vec<(String, PathBuf)>), String>),
    VerDiffCommit(String, PathBuf),
    /// pull, push ou fetch: rodam no terminal, que mostra a saída e pede a senha se precisar.
    GitRemoto(&'static str),
    MensagemCommit(String),
    GerarTituloCommit,
    TituloCommit(u64, Result<String, String>),
    Commit,
    Commitado(Result<(), String>),
    Git(Result<paineis::EstadoGit, String>),
    Menu(Option<usize>),
    Configuracoes(Option<usize>),
    Escape,
    TemaCodigo(usize),
    Filtro(String),
    ProcurarArquivo,
    Recuar(bool),
    Comentar,
    /// Começa (`Some`) ou termina (`None`) o arraste de uma divisa entre os painéis.
    Redimensionar(Option<Divisa>),
    /// O mouse andou durante o arraste (posição na janela).
    Mover(Point),
    /// Abre (`Some`, onde clicou) ou fecha o menu do botão direito do editor.
    MenuContexto(Option<Point>),
    Recortar,
    CopiarSelecao,
    Colar,
    Colado(Option<String>),
    RodarSelecao,
    /// Põe a seleção (ou a linha do cursor) como trecho no próximo pedido do chat.
    MandarTrecho,
    TirarTrecho(usize),
}

/// Vão entre painéis que dá para arrastar.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Divisa {
    Lado,
    Agente,
    Terminal,
}

struct Chat {
    conversa: u64,
    ligado: bool,
    canal: Option<UnboundedSender<String>>,
    /// Linhas escritas antes de o claude abrir.
    fila: Vec<String>,
    itens: Vec<Item>,
    ocupado: bool,
    pedido: Content,
    /// Imagens que vão com o próximo pedido: nome do arquivo e bloco `image` já em base64.
    imagens: Vec<(String, serde_json::Value)>,
    /// Trechos de código que vão com o próximo pedido: rótulo (`edicao.rs 300:500`) e o texto.
    trechos: Vec<(String, String)>,
    modo: Modo,
    /// Agente do painel; trocar começa uma conversa nova.
    provedor: agente::Provedor,
    uso: (Option<f32>, Option<f32>),
    info: agente::Info,
    /// Tokens no contexto e tamanho da janela do modelo.
    contexto: (u64, u64),
    detalhes: bool,
    proximo_controle: u64,
    retomar: Option<String>,
    historico: Option<Vec<agente::Sessao>>,
}

impl Chat {
    fn novo(id: u64, provedor: agente::Provedor, modo: Modo) -> Self {
        Self {
            conversa: id,
            ligado: false,
            canal: None,
            fila: Vec::new(),
            itens: Vec::new(),
            ocupado: false,
            pedido: Content::new(),
            imagens: Vec::new(),
            trechos: Vec::new(),
            modo,
            provedor,
            uso: (None, None),
            info: agente::Info::default(),
            contexto: (0, 0),
            detalhes: false,
            proximo_controle: 0,
            retomar: None,
            historico: None,
        }
    }
}

struct Code {
    /// Página inicial à mostra (nenhuma pasta aberta ainda, ou a pessoa voltou para ela).
    inicio: bool,
    config: config::Config,
    ini: inicio::Estado,
    arvore: arvore::Arvore,
    abas: Vec<Aba>,
    ativa: usize,
    chat: Chat,
    outros_chats: Vec<Chat>,
    proximo_chat: u64,
    /// Os agentes do seletor do rodapé (o `pick_list` empresta a lista).
    agentes: Vec<agente::Provedor>,
    terminais: terminal::Terminais,
    aviso: String,
    ramo: Option<String>,
    tamanho: Size,
    ultimo_clique_barra: Option<std::time::Instant>,
    /// Painel ao lado do trilho; `None` = recolhido.
    lado: Option<Lado>,
    agente_aberto: bool,
    terminal_aberto: bool,
    busca: String,
    mensagem_commit: String,
    geracao_titulo: u64,
    titulo_pendente: Option<(PathBuf, String)>,
    /// Diferença aberta no lugar do editor (clique numa mudança do painel Git).
    diff: Option<paineis::Diff>,
    /// Commit aberto no gráfico do Git: hash e arquivos mudados.
    commit_aberto: Option<(String, Vec<(String, PathBuf)>)>,
    achados: Option<Vec<paineis::Achado>>,
    buscando: bool,
    git: Option<Result<paineis::EstadoGit, String>>,
    menu: Option<usize>,
    configuracoes: Option<usize>,
    /// Texto do campo "Procurar arquivo" do explorador.
    filtro: String,
    /// Lida na primeira letra do filtro; vazia de novo quando a pasta muda.
    todos_arquivos: Vec<PathBuf>,
    /// Código de exemplo das configurações, pintado com o tema de cores escolhido.
    amostra: Content,
    /// Largura escolhida no arraste para o painel ao lado do trilho; `None` = a padrão de cada painel.
    largura_lado: Option<f32>,
    largura_agente: f32,
    altura_terminal: f32,
    arrastando: Option<Divisa>,
    /// Menu do botão direito do editor aberto nesse ponto.
    contexto: Option<Point>,
}

impl pelicano::App for Code {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        Code::update(self, msg)
    }

    fn view(&self) -> Element<'_, Msg> {
        Code::view(self)
    }

    fn subscription(&self) -> Subscription<Msg> {
        Code::subscription(self)
    }

    fn theme(&self) -> Theme {
        tema::tema()
    }

    fn on_event(&self, ev: &Event) -> Option<Msg> {
        match ev {
            Event::Resized(s) => Some(Msg::Tamanho(*s)),
            // Arquivo solto na janela vira anexo do chat (só imagens são aceitas).
            Event::FileDropped(p) => Some(Msg::Anexadas(vec![p.clone()])),
            _ => None,
        }
    }

    /// Atalhos: só chegam aqui com o foco fora do campo, do editor e do terminal (eles pegam a tecla).
    fn on_unhandled_event(&self, _: WindowId, ev: &Event) -> Option<Msg> {
        let Event::Key {
            key,
            modifiers: m,
            pressed: true,
            ..
        } = ev
        else {
            return None;
        };
        let ctrl = m.control_key() || m.super_key();
        let shift = m.shift_key();
        match key {
            Key::Named(NamedKey::F5) => Some(Msg::RodarProjeto),
            Key::Named(NamedKey::Escape) => Some(Msg::Escape),
            Key::Character(c) => match c.as_str() {
                "," if ctrl => Some(Msg::Configuracoes(Some(0))),
                "s" | "S" if ctrl && shift => Some(Msg::SalvarTudo),
                "s" if ctrl => Some(Msg::Salvar),
                "o" | "O" if ctrl && shift => Some(Msg::EscolherArquivo),
                "e" | "E" if ctrl && shift => Some(Msg::Lado(Lado::Explorador)),
                "g" | "G" if ctrl && shift => Some(Msg::Lado(Lado::Git)),
                "f" | "F" if ctrl && shift => Some(Msg::Lado(Lado::Buscar)),
                "`" if ctrl => Some(Msg::FocarTerminal),
                "o" if ctrl => Some(Msg::AbrirPasta),
                "b" if ctrl => Some(Msg::Lado(Lado::Explorador)),
                "p" if ctrl => Some(Msg::ProcurarArquivo),
                "j" if ctrl => Some(Msg::AlternarTerminal),
                "1" if m.is_empty() => Some(Msg::ResponderPendente(Resposta::Gravou)),
                "2" if m.is_empty() => Some(Msg::ResponderPendente(Resposta::Sempre)),
                "3" if m.is_empty() => Some(Msg::ResponderPendente(Resposta::Recusou)),
                _ => None,
            },
            _ => None,
        }
    }
}

impl Code {
    fn trocar_chat(&mut self, id: u64) {
        if let Some(i) = self.outros_chats.iter().position(|c| c.conversa == id) {
            std::mem::swap(&mut self.chat, &mut self.outros_chats[i]);
        }
    }

    fn novo(mut config: config::Config) -> (Self, Task<Msg>) {
        // Sem pasta na linha de comando, abre na página inicial.
        // Nos testes, o argumento é do executor de testes (`--quiet`, o filtro), não uma pasta.
        let pedida = std::env::args_os()
            .nth(1)
            .filter(|_| !cfg!(test))
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
            chat: Chat::novo(0, agente::Provedor::Claude, Modo::Perguntar),
            outros_chats: Vec::new(),
            proximo_chat: 1,
            agentes: vec![agente::Provedor::Claude],
            terminais: terminal::Terminais::default(),
            aviso: String::new(),
            ramo: ramo(&pasta),
            tamanho: Size::new(1380.0, 860.0),
            ultimo_clique_barra: None,
            lado: Some(Lado::Explorador),
            agente_aberto: true,
            terminal_aberto: true,
            busca: String::new(),
            mensagem_commit: String::new(),
            geracao_titulo: 0,
            titulo_pendente: None,
            diff: None,
            commit_aberto: None,
            achados: None,
            buscando: false,
            git: None,
            menu: None,
            configuracoes: None,
            filtro: String::new(),
            todos_arquivos: Vec::new(),
            amostra: Content::with_text(preferencias::AMOSTRA),
            largura_lado: None,
            largura_agente: LARGURA_AGENTE,
            altura_terminal: ALTURA_TERMINAL,
            arrastando: None,
            contexto: None,
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
        // FEATHERCODE_CAPTURAR_CONFIG=1 abre as configurações nessa seção (1 = Editor).
        if let Some(i) = std::env::var("FEATHERCODE_CAPTURAR_CONFIG")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            code.configuracoes = Some(i);
        }
        if std::env::var_os("FEATHERCODE_CAPTURAR_HISTORICO").is_some() {
            code.chat.historico = Some(agente::sessoes(&code.arvore.raiz));
        }
        if std::env::var_os("FEATHERCODE_CAPTURAR_EXEMPLO").is_some() {
            code.chat.itens = agente::exemplo();
            code.chat.uso = (Some(0.43), Some(0.63));
            code.chat.contexto = (420_000, 1_000_000);
            code.chat.info.modelo = "claude-opus-5-5[1m]".into();
        }
        code.atualizar_recentes();
        // FEATHERCODE_CAPTURAR_GIT=1 abre o painel Git na foto.
        // FEATHERCODE_CAPTURAR_DIFF=src/main.rs mostra a diferença desse arquivo.
        let mut git = if std::env::var_os("FEATHERCODE_CAPTURAR_GIT").is_some() {
            code.update(Msg::Lado(Lado::Git))
        } else {
            Task::none()
        };
        if let Some(arquivo) = std::env::var_os("FEATHERCODE_CAPTURAR_DIFF") {
            git = Task::batch([
                git,
                code.update(Msg::VerDiff(code.arvore.raiz.join(arquivo))),
            ]);
        }
        let capturar = if std::env::var_os("FEATHERCODE_CAPTURAR").is_some() {
            // O `sleep` nasce dentro do async: o `novo` roda antes do runtime do tokio.
            Task::perform(
                async { tokio::time::sleep(std::time::Duration::from_millis(900)).await },
                |_| Msg::Capturar,
            )
        } else {
            Task::none()
        };
        (code, Task::batch([inicio::conferir_claude(), git, capturar]))
    }

    fn subscription(&self) -> Subscription<Msg> {
        use pelicano::futures::StreamExt;
        let mut subs = Vec::new();
        for chat in std::iter::once(&self.chat).chain(self.outros_chats.iter()) {
            if chat.ligado {
                let id = chat.conversa;
                let conversa = agente::conversa(
                    chat.provedor,
                    self.arvore.raiz.clone(),
                    chat.modo.id(),
                    chat.retomar.clone(),
                );
                // O id vai no fluxo: o `map` da assinatura não pode capturar nada.
                subs.push(Subscription::run_with_id(
                    ("agente", id),
                    conversa.map(move |evento| Msg::EventoChat(id, evento)),
                ));
            }
        }
        subs.push(self.terminais.subscription());
        Subscription::batch(subs)
    }

    fn enviar(&mut self, linha: String) {
        match &self.chat.canal {
            Some(c) if c.send(linha.clone()).is_ok() => {}
            _ => self.chat.fila.push(linha),
        }
    }

    fn controle(&mut self, pedido: serde_json::Value) {
        self.chat.proximo_controle += 1;
        let linha = agente::controle(self.chat.proximo_controle, pedido);
        self.enviar(linha);
    }

    fn tema_codigo(&self) -> Highlight {
        match self
            .config
            .tema_codigo
            .and_then(|i| Highlight::ALL.get(i))
        {
            Some(t) => *t,
            None if tema::escuro() => Highlight::Base16Ocean,
            None => Highlight::InspiredGitHub,
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
        let (linha, _) = aba.conteudo.cursor();
        let mut s = format!(
            "\n\n<contexto_do_editor>\nArquivo aberto no FeatherCode: {} (cursor na linha {linha})",
            self.relativo(&aba.caminho).display(),
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
        for aba in self.abas.iter_mut().filter(|a| !a.sujo()) {
            // shortcut: aba com mudança não salva fica como está; avisar do conflito quando incomodar.
            if let Ok(t) = std::fs::read_to_string(&aba.caminho) {
                if t.trim_end() != aba.conteudo.text().trim_end() {
                    aba.conteudo = Content::with_text(&t);
                    aba.salvo = aba.conteudo.version();
                }
            }
        }
    }

    /// Desce o chat até a última mensagem, como o `anchor_bottom` fazia.
    fn rolar_chat() -> Task<Msg> {
        Task::snap_to("chat", Point::new(0.0, 1.0))
    }

    fn sinal(&mut self, s: Sinal) {
        match s {
            Sinal::Item(i) => self.chat.itens.push(i),
            Sinal::Resultado { id, erro } => {
                for item in self.chat.itens.iter_mut().rev() {
                    if let Item::Ferramenta { id: i, erro: e, .. } = item {
                        if *i == id {
                            *e = erro;
                            break;
                        }
                    }
                }
                self.recarregar_abas();
            }
            Sinal::Uso(cinco, sete) => {
                self.chat.uso = (cinco.or(self.chat.uso.0), sete.or(self.chat.uso.1))
            }
            Sinal::Inicio(i) => self.chat.info = i,
            Sinal::Contexto(n) => self.chat.contexto.0 = n,
            Sinal::JanelaContexto(n) => self.chat.contexto.1 = n,
            Sinal::FimDoTurno(erro) => {
                self.chat.ocupado = false;
                self.chat.itens.extend(erro.map(Item::Aviso));
                self.arvore.recarregar();
                self.ramo = ramo(&self.arvore.raiz);
                self.recarregar_abas();
            }
        }
    }

    fn abrir_pasta(&mut self, p: PathBuf) -> Task<Msg> {
        if self.abas.iter().any(Aba::sujo) {
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
        self.geracao_titulo += 1;
        self.titulo_pendente = None;
        self.mensagem_commit.clear();
        self.arvore = arvore::Arvore::nova(p);
        self.abas.clear();
        self.ativa = 0;
        self.inicio = false;
        self.aviso.clear();
        self.achados = None;
        self.filtro.clear();
        self.todos_arquivos.clear();
        self.git = None;
        self.outros_chats.clear();
        self.chat = Chat::novo(self.proximo_chat, self.chat.provedor, self.chat.modo);
        self.proximo_chat += 1;
        Task::none()
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        // Escolher qualquer coisa fecha o menu; o que chega sozinho (agente, git, busca) não.
        if !matches!(
            msg,
            Msg::Escape
                | Msg::Menu(_)
                | Msg::Capturar
                | Msg::Agente(_)
                | Msg::EventoChat(_, _)
                | Msg::Tamanho(_)
                | Msg::Mover(_)
                | Msg::Term(_, _)
                | Msg::Git(_)
                | Msg::Achados(_)
                | Msg::Ini(inicio::Ini::ClaudeVersao(_) | inicio::Ini::OutrosAgentes(_))
        ) {
            self.menu = None;
            self.contexto = None;
        }
        match msg {
            Msg::Menu(m) => self.menu = m,
            Msg::Redimensionar(d) => self.arrastando = d,
            Msg::MenuContexto(p) => self.contexto = p,
            Msg::CopiarSelecao | Msg::Recortar => {
                let Some(aba) = self.abas.get(self.ativa) else {
                    return Task::none();
                };
                let Some(sel) = aba.conteudo.selection() else {
                    return Task::none();
                };
                if matches!(msg, Msg::Recortar) {
                    aba.conteudo.insert("");
                }
                return Task::batch([Task::write_clipboard(sel), Task::focus("editor")]);
            }
            Msg::Colar => return Task::read_clipboard(Msg::Colado),
            Msg::Colado(texto) => {
                if let (Some(aba), Some(t)) = (self.abas.get(self.ativa), texto) {
                    aba.conteudo.insert(&t);
                }
                return Task::focus("editor");
            }
            Msg::RodarSelecao => {
                let Some(sel) = self.aba().and_then(|a| a.conteudo.selection()) else {
                    return Task::none();
                };
                self.terminal_aberto = true;
                match self.terminais.rodar(&self.arvore.raiz, sel.trim()) {
                    Ok(_) => return self.terminais.focar(),
                    Err(e) => self.aviso = e,
                }
            }
            Msg::MandarTrecho => {
                let Some(aba) = self.aba() else {
                    return Task::none();
                };
                let (ini, fim) = edicao::linhas_da_selecao(&aba.conteudo);
                let codigo: Vec<String> = (ini..=fim).filter_map(|i| aba.conteudo.line(i)).collect();
                let nome = aba
                    .caminho
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let rotulo = if ini == fim {
                    format!("{nome} {ini}")
                } else {
                    format!("{nome} {ini}:{fim}")
                };
                let texto = format!(
                    "Trecho de {}, linhas {ini} a {fim}:\n```\n{}\n```",
                    self.relativo(&aba.caminho).display(),
                    codigo.join("\n")
                );
                self.chat.trechos.push((rotulo, texto));
                self.agente_aberto = true;
                return Task::focus("pedido");
            }
            Msg::TirarTrecho(i) => {
                if i < self.chat.trechos.len() {
                    self.chat.trechos.remove(i);
                }
            }
            Msg::Mover(p) => {
                // shortcut: tamanhos valem até fechar o app; gravar na config se pedirem.
                // min/max em vez de clamp: janela minimizada tem tamanho 0 e o clamp entraria em pânico.
                let (w, h) = (self.tamanho.width, self.tamanho.height);
                match self.arrastando {
                    Some(Divisa::Lado) => {
                        self.largura_lado = Some((p.x - paineis::LARGURA_TRILHO).min(w / 2.0).max(160.0))
                    }
                    Some(Divisa::Agente) => {
                        self.largura_agente = (w - p.x - VAO).min(w / 2.0).max(280.0)
                    }
                    Some(Divisa::Terminal) => {
                        self.altura_terminal =
                            (h - ALTURA_STATUS - VAO - p.y).min(h - 260.0).max(80.0)
                    }
                    None => {}
                }
            }
            Msg::Configuracoes(secao) => self.configuracoes = secao,
            Msg::Escape => {
                if self.menu.is_some() || self.contexto.is_some() {
                    self.menu = None;
                    self.contexto = None;
                } else {
                    self.configuracoes = None;
                }
            }
            Msg::Filtro(f) => {
                if self.todos_arquivos.is_empty() && !f.trim().is_empty() {
                    self.todos_arquivos = paineis::arquivos(&self.arvore.raiz);
                }
                self.filtro = f;
            }
            Msg::ProcurarArquivo => {
                self.lado = Some(Lado::Explorador);
                return Task::focus("filtro");
            }
            Msg::Recuar(mais) => {
                let Some(aba) = self.abas.get(self.ativa) else {
                    return Task::none();
                };
                // Tab sem seleção só põe espaços no cursor, como em qualquer editor.
                if mais && aba.conteudo.selection().is_none() {
                    aba.conteudo.insert("    ");
                } else {
                    edicao::recuar(&aba.conteudo, mais);
                }
            }
            Msg::Comentar => {
                let Some(aba) = self.abas.get(self.ativa) else {
                    return Task::none();
                };
                let Some(marca) = edicao::marca_comentario(&aba.caminho) else {
                    self.aviso = "Não sei comentar este tipo de arquivo.".into();
                    return Task::none();
                };
                edicao::comentar(&aba.conteudo, marca);
            }
            Msg::Lado(l) => {
                self.lado = if self.lado == Some(l) && l != Lado::Buscar {
                    None
                } else {
                    Some(l)
                };
                match self.lado {
                    Some(Lado::Buscar) => return Task::focus("busca"),
                    Some(Lado::Git) => return self.update(Msg::GitAtualizar),
                    _ => {}
                }
            }
            Msg::AlternarAgente => self.agente_aberto = !self.agente_aberto,
            Msg::AlternarTerminal => self.terminal_aberto = !self.terminal_aberto,
            Msg::AlternarLinhas => {
                self.config.linhas_alternadas = !self.config.linhas_alternadas;
                self.config.gravar();
            }
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
                if let Some(aba) = self.abas.get(self.ativa) {
                    aba.conteudo.go_to_line(linha + 1);
                }
                return Task::batch([abrir, Task::focus("editor")]);
            }
            Msg::GitAtualizar => {
                return Task::perform(paineis::status_git(self.arvore.raiz.clone()), Msg::Git)
            }
            Msg::Git(r) => self.git = Some(r),
            Msg::GitRemoto(cmd) => {
                self.terminal_aberto = true;
                match self
                    .terminais
                    .rodar_git(&self.arvore.raiz, &format!("git {cmd}"))
                {
                    Ok(_) => return self.terminais.focar(),
                    Err(e) => self.aviso = e,
                }
            }
            Msg::VerDiff(p) => {
                return Task::perform(paineis::diff_git(self.arvore.raiz.clone(), p), Msg::Diff)
            }
            Msg::Diff(Ok(d)) => self.diff = Some(d),
            Msg::Diff(Err(e)) => self.aviso = e,
            Msg::FecharDiff => self.diff = None,
            Msg::AbrirCommit(h) => {
                if self.commit_aberto.as_ref().is_some_and(|(a, _)| *a == h) {
                    self.commit_aberto = None;
                } else {
                    return Task::perform(
                        paineis::arquivos_do_commit(self.arvore.raiz.clone(), h),
                        Msg::ArquivosCommit,
                    );
                }
            }
            Msg::ArquivosCommit(Ok(c)) => self.commit_aberto = Some(c),
            Msg::ArquivosCommit(Err(e)) => self.aviso = e,
            Msg::VerDiffCommit(h, p) => {
                return Task::perform(
                    paineis::diff_do_commit(self.arvore.raiz.clone(), h, p),
                    Msg::Diff,
                )
            }
            Msg::GerarTituloCommit => {
                if self.titulo_pendente.is_some() {
                    return Task::none();
                }
                self.geracao_titulo += 1;
                let id = self.geracao_titulo;
                self.titulo_pendente =
                    Some((self.arvore.raiz.clone(), self.mensagem_commit.clone()));
                return Task::perform(
                    paineis::titulo_por_ia(self.arvore.raiz.clone(), self.chat.provedor),
                    move |r| Msg::TituloCommit(id, r),
                );
            }
            Msg::TituloCommit(id, resultado) => {
                if id != self.geracao_titulo {
                    return Task::none();
                }
                if let Some((raiz, original)) = self.titulo_pendente.take() {
                    if raiz == self.arvore.raiz {
                        match resultado {
                            Ok(titulo) if self.mensagem_commit == original => self.mensagem_commit = titulo,
                            Ok(_) => self.aviso = "O título foi mantido porque você editou o campo durante a geração.".into(),
                            Err(e) => self.aviso = e,
                        }
                    }
                }
            }
            Msg::MensagemCommit(t) => self.mensagem_commit = t,
            Msg::Commit => {
                let msg = self.mensagem_commit.trim().to_string();
                if msg.is_empty() {
                    self.aviso = "Escreva a mensagem do commit.".into();
                } else {
                    return Task::perform(
                        paineis::commit(self.arvore.raiz.clone(), msg),
                        Msg::Commitado,
                    );
                }
            }
            Msg::Commitado(Ok(())) => {
                self.geracao_titulo += 1;
                self.titulo_pendente = None;
                self.mensagem_commit.clear();
                return self.update(Msg::GitAtualizar);
            }
            Msg::Commitado(Err(e)) => self.aviso = e,
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
                if self.abas.iter().any(Aba::sujo) {
                    self.aviso =
                        "Salve ou feche as abas com mudanças antes de voltar ao início.".into();
                } else {
                    self.inicio = true;
                    self.atualizar_recentes();
                }
            }
            Msg::PastaEscolhida(None) => {}
            Msg::EscolherArquivo => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Abrir arquivo")
                        .set_directory(&self.arvore.raiz)
                        .pick_file(),
                    |h| Msg::ArquivoEscolhido(h.map(|h| h.path().to_path_buf())),
                );
            }
            Msg::ArquivoEscolhido(Some(p)) => {
                let abrir = if self.inicio {
                    p.parent()
                        .map(|pai| self.abrir_pasta(pai.to_path_buf()))
                        .unwrap_or_else(Task::none)
                } else {
                    Task::none()
                };
                return Task::batch([abrir, Task::done(Msg::AbrirArquivo(p))]);
            }
            Msg::ArquivoEscolhido(None) => {}
            Msg::AtualizarArvore => {
                self.arvore.recarregar();
                self.todos_arquivos = if self.filtro.trim().is_empty() {
                    Vec::new()
                } else {
                    paineis::arquivos(&self.arvore.raiz)
                };
            }
            Msg::RecolherArvore => {
                self.arvore.recolher();
                self.filtro.clear();
                self.todos_arquivos.clear();
            }
            Msg::Alternar(p) => self.arvore.alternar(&p),
            Msg::AbrirArquivo(p) => {
                self.diff = None;
                if let Some(i) = self.abas.iter().position(|a| a.caminho == p) {
                    self.ativa = i;
                } else {
                    match std::fs::read_to_string(&p) {
                        Ok(t) => {
                            self.config.usar_arquivo(p.clone());
                            let conteudo = Content::with_text(&t);
                            self.abas.push(Aba {
                                caminho: p,
                                salvo: conteudo.version(),
                                conteudo,
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
            Msg::TrocarAba(i) => {
                self.ativa = i;
                self.diff = None;
            }
            Msg::FecharAba(i) if i >= self.abas.len() => {}
            Msg::FecharAba(i) => {
                // shortcut: fecha sem perguntar mesmo com mudança; pedir confirmação no diálogo da suíte depois.
                self.abas.remove(i);
                self.ativa = self.ativa.min(self.abas.len().saturating_sub(1));
            }
            Msg::SalvarTudo => {
                let mut erros = Vec::new();
                for aba in self.abas.iter_mut().filter(|a| a.sujo()) {
                    match std::fs::write(&aba.caminho, aba.conteudo.text()) {
                        Ok(()) => aba.salvo = aba.conteudo.version(),
                        Err(e) => erros.push(format!("{}: {e}", aba.caminho.display())),
                    }
                }
                self.aviso = if erros.is_empty() {
                    String::new()
                } else {
                    format!("Não deu para salvar: {}", erros.join("; "))
                };
            }
            Msg::Salvar => {
                if let Some(aba) = self.abas.get_mut(self.ativa) {
                    match std::fs::write(&aba.caminho, aba.conteudo.text()) {
                        Ok(()) => {
                            aba.salvo = aba.conteudo.version();
                            self.aviso.clear();
                        }
                        Err(e) => self.aviso = format!("Não deu para salvar: {e}"),
                    }
                }
            }
            Msg::Pedido(t) => {
                self.chat.pedido = Content::with_text(&t);
                return Task::focus("pedido");
            }
            Msg::Anexar => {
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Mandar imagem ao agente")
                        .add_filter("Imagem", &["png", "jpg", "jpeg", "gif", "webp"])
                        .pick_files(),
                    |h| {
                        Msg::Anexadas(
                            h.unwrap_or_default()
                                .iter()
                                .map(|h| h.path().to_path_buf())
                                .collect(),
                        )
                    },
                );
            }
            Msg::Anexadas(caminhos) => {
                // shortcut: só o Claude Code recebe imagem; o Codex aceita `localImage` no turn/start, ligar quando pedirem.
                if self.chat.provedor != agente::Provedor::Claude && !caminhos.is_empty() {
                    self.aviso = format!("O {} não recebe imagens por aqui.", self.chat.provedor);
                    return Task::none();
                }
                for c in caminhos {
                    match agente::bloco_imagem(&c) {
                        Ok(b) => {
                            let nome = c.file_name().unwrap_or_default().to_string_lossy();
                            self.chat.imagens.push((nome.into(), b));
                        }
                        Err(e) => self.aviso = e,
                    }
                }
            }
            Msg::TirarImagem(i) => {
                if i < self.chat.imagens.len() {
                    self.chat.imagens.remove(i);
                }
            }
            Msg::Enviar => {
                let pedido = self.chat.pedido.text().trim().to_string();
                let vazio = self.chat.imagens.is_empty() && self.chat.trechos.is_empty();
                if (pedido.is_empty() && vazio) || self.chat.ocupado {
                    return Task::none();
                }
                let imagens: Vec<_> = self.chat.imagens.drain(..).collect();
                let blocos: Vec<_> = imagens.iter().map(|(_, b)| b.clone()).collect();
                let trechos: Vec<_> = self.chat.trechos.drain(..).collect();
                let codigo: String = trechos.iter().map(|(_, t)| format!("\n\n{t}")).collect();
                let linha = agente::mensagem_com_imagens(
                    &format!("{pedido}{codigo}{}", self.contexto()),
                    &blocos,
                );
                let mut mostrado = pedido;
                for (rotulo, _) in &trechos {
                    mostrado.push_str(&format!("\n[{rotulo}]"));
                }
                for (nome, _) in &imagens {
                    mostrado.push_str(&format!("\n[imagem: {nome}]"));
                }
                self.chat.itens.push(Item::Eu(mostrado.trim_start().into()));
                self.chat.pedido = Content::new();
                self.chat.ocupado = true;
                self.chat.ligado = true;
                self.enviar(linha);
                return Self::rolar_chat();
            }
            Msg::Parar => self.controle(serde_json::json!({"subtype": "interrupt"})),
            Msg::NovaConversa => {
                // A conversa atual ainda vazia serve: não abre outra aba à toa.
                if self.chat.itens.is_empty() && !self.chat.ligado {
                    self.chat.historico = None;
                    return Task::none();
                }
                let novo = Chat::novo(self.proximo_chat, self.chat.provedor, self.chat.modo);
                self.proximo_chat += 1;
                self.outros_chats
                    .push(std::mem::replace(&mut self.chat, novo));
            }
            // Soltar o Chat solta a subscription, e o processo do agente morre junto.
            Msg::FecharChat(id) => {
                if id == self.chat.conversa {
                    self.chat = match self.outros_chats.pop() {
                        Some(c) => c,
                        None => {
                            self.proximo_chat += 1;
                            Chat::novo(self.proximo_chat - 1, self.chat.provedor, self.chat.modo)
                        }
                    };
                } else {
                    self.outros_chats.retain(|c| c.conversa != id);
                }
            }
            Msg::TrocarChat(id) => {
                self.trocar_chat(id);
            }
            Msg::EventoChat(id, evento) => {
                let anterior = self.chat.conversa;
                if id != anterior && !self.outros_chats.iter().any(|c| c.conversa == id) {
                    return Task::none();
                }
                self.trocar_chat(id);
                let tarefa = self.update(Msg::Agente(evento));
                self.trocar_chat(anterior);
                return if id == anterior {
                    Task::batch([tarefa, Self::rolar_chat()])
                } else {
                    tarefa
                };
            }
            Msg::Detalhes => self.chat.detalhes = !self.chat.detalhes,
            Msg::Historico => {
                self.chat.historico = match self.chat.historico {
                    Some(_) => None,
                    None => Some(agente::sessoes(&self.arvore.raiz)),
                };
            }
            Msg::Retomar(id) => {
                let _ = self.update(Msg::NovaConversa);
                for s in agente::sinais_da_sessao(&self.arvore.raiz, &id) {
                    self.sinal(s);
                }
                self.chat.ocupado = false;
                self.chat.retomar = Some(id);
            }
            Msg::Provedor(p) => {
                if p != self.chat.provedor {
                    let _ = self.update(Msg::NovaConversa);
                    self.chat.provedor = p;
                    self.chat.info = agente::Info::default();
                    self.chat.uso = (None, None);
                }
            }
            Msg::Modo(m) => {
                self.chat.modo = m;
                if self.chat.provedor == agente::Provedor::Antigravity && self.chat.canal.is_some()
                {
                    self.chat.itens.push(Item::Aviso(
                        "O Antigravity só troca de modo numa conversa nova (botão +).".into(),
                    ));
                }
                if self.chat.canal.is_some() {
                    self.controle(
                        serde_json::json!({"subtype": "set_permission_mode", "mode": m.id()}),
                    );
                }
            }
            Msg::Agente(agente::Evento::Pronto(canal)) => {
                for linha in self.chat.fila.drain(..) {
                    let _ = canal.send(linha);
                }
                self.chat.canal = Some(canal);
            }
            Msg::Agente(agente::Evento::Mensagem(v)) => {
                for s in agente::interpretar(&v) {
                    self.sinal(s);
                }
            }
            Msg::Agente(agente::Evento::Encerrado(motivo)) => {
                self.chat.itens.extend(motivo.map(Item::Aviso));
                self.chat.ligado = false;
                self.chat.canal = None;
                self.chat.ocupado = false;
            }
            Msg::Responder(i, r) => {
                let Some(Item::Pedido {
                    request_id,
                    input,
                    sugestoes,
                    resposta,
                    ..
                }) = self.chat.itens.get_mut(i)
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
            Msg::ResponderPendente(r) => {
                let pendente = self.chat.itens.iter().position(|i| {
                    matches!(
                        i,
                        Item::Pedido {
                            resposta: Resposta::Pendente,
                            ..
                        }
                    )
                });
                if let Some(i) = pendente {
                    return self.update(Msg::Responder(i, r));
                }
            }
            Msg::Copiar(t) => return Task::write_clipboard(t),
            Msg::FocarTerminal => {
                self.terminal_aberto = true;
                if self.terminais.ativo().is_some() {
                    return self.terminais.focar();
                }
                return self.update(Msg::NovoTerminal);
            }
            Msg::NovoTerminal => {
                self.terminal_aberto = true;
                match self.terminais.novo(&self.arvore.raiz, None) {
                    Ok(_) => return self.terminais.focar(),
                    Err(e) => self.aviso = e,
                }
            }
            Msg::FecharTerminal(id) => self.terminais.fechar(id),
            Msg::TrocarTerminal(i) => {
                self.terminais.ativa = i;
                return self.terminais.focar();
            }
            Msg::Term(id, ev) => self.terminais.tratar(id, ev),
            Msg::RodarProjeto => {
                let arquivo = self.aba().map(|a| a.caminho.clone());
                match execucao::comando_do_projeto(&self.arvore.raiz, arquivo.as_deref()) {
                    Some(cmd) => {
                        self.terminal_aberto = true;
                        match self.terminais.rodar(&self.arvore.raiz, &cmd) {
                            Ok(_) => return self.terminais.focar(),
                            Err(e) => self.aviso = e,
                        }
                    }
                    None => {
                        self.aviso =
                            "Não sei como rodar esta pasta. Digite o comando no terminal.".into()
                    }
                }
            }
            Msg::Link(url) => {
                #[cfg(target_os = "linux")]
                let abrir = std::process::Command::new("xdg-open").arg(&url).spawn();
                #[cfg(target_os = "windows")]
                let abrir = std::process::Command::new("cmd")
                    .args(["/C", "start", "", &url])
                    .spawn();
                #[cfg(target_os = "macos")]
                let abrir = std::process::Command::new("open").arg(&url).spawn();
                if let Err(e) = abrir {
                    self.aviso = format!("Não deu para abrir o link: {e}");
                }
            }
            // A foto sai do mesmo desenho da janela, sem passar pela tela.
            Msg::Capturar => {
                if let Some(caminho) = std::env::var_os("FEATHERCODE_CAPTURAR") {
                    let png = pelicano::capture(
                        &self.view(),
                        self.tamanho,
                        1.0,
                        &tema::tema(),
                        &mut pelicano::Renderer::new(),
                        None,
                    );
                    if let Err(e) = std::fs::write(&caminho, png) {
                        eprintln!("[captura] {e}");
                    }
                }
                return Task::window(WindowCmd::Exit);
            }
            Msg::Tamanho(s) => self.tamanho = s,
            Msg::Arrastar => {
                return feather_ui::janelas::arrastar_ou_maximizar(
                    WindowId::MAIN,
                    &mut self.ultimo_clique_barra,
                );
            }
            Msg::Minimizar => return Task::window(WindowCmd::Minimize),
            Msg::Maximizar => return Task::window(WindowCmd::ToggleMaximize),
            Msg::Fechar => return Task::window(WindowCmd::Close),
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Msg> {
        let nome = container(
            row![
                // A barra mostra apenas a pena, os menus e as ações.
                container(pena(27.0)).width(paineis::LARGURA_MARCA),
                self.menus_view(),
                horizontal_space(),
                botao_texto(
                    row![
                        icone_traco("folder-open", 14.0),
                        text("Abrir pasta").size(12)
                    ]
                    .spacing(6)
                    .align(Align::Center),
                    Msg::AbrirPasta
                ),
            ]
            .spacing(10)
            .align(Align::Center),
        )
        .padding([0, 12])
        .width(Length::FILL)
        .height(cromo::BARRA_ALTURA)
        .align_y(Align::Center)
        // Mesmo fundo dos vãos entre os cartões: topo, trilho e rodapé formam a moldura escura.
        .style(|_| fundo(COR_FUNDO_JANELA()));
        let barra = cromo::barra_integrada(
            nome.into(),
            cromo::AcoesJanela {
                arrastar: Msg::Arrastar,
                minimizar: Msg::Minimizar,
                maximizar: Msg::Maximizar,
                fechar: Msg::Fechar,
            },
        );

        let janela: Element<Msg> = if let Some(secao) = self.configuracoes {
            column![barra, self.preferencias_view(secao), self.status_view()].into()
        } else if self.inicio {
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
        // Durante o arraste, uma camada na janela toda segue o mouse até soltar.
        let janela = match self.arrastando {
            Some(d) => stack![
                janela,
                mouse_area(Space::new(Length::FILL, Length::FILL))
                    .on_move(Msg::Mover)
                    .on_release(Msg::Redimensionar(None))
                    .cursor_icon(cursor_da_divisa(d))
            ]
            .width(Length::FILL)
            .height(Length::FILL)
            .into(),
            None => janela,
        };
        let janela = match self.menu_aberto_view().or_else(|| self.menu_contexto_view()) {
            Some(menu) => stack![janela, menu]
                .width(Length::FILL)
                .height(Length::FILL)
                .into(),
            None => janela,
        };
        cromo::moldura_janela(janela)
    }

    fn area_de_trabalho<'a>(&'a self, barra: Element<'a, Msg>) -> Element<'a, Msg> {
        // Cada painel é um cartão de cantos arredondados; o fundo da janela aparece nos vãos.
        let editor = cartao(
            column![
                self.abas_view(),
                match &self.diff {
                    Some(d) => self.diff_view(d),
                    None => self.editor_view(),
                }
            ]
            .height(Length::FILL),
            COR_PAINEL(),
            Length::FILL,
        );
        // Os vãos entre os cartões são as alças de arrastar.
        let mut centro = column![editor]
            .width(Length::FILL)
            .height(Length::FILL);
        if self.terminal_aberto {
            centro = centro
                .push(alca(Divisa::Terminal))
                .push(cartao(self.terminal_view(), cor_terminal(), Length::Shrink));
        }
        let mut cartoes = row![].height(Length::FILL);
        if let Some(lado) = self.lado_view() {
            cartoes = cartoes
                .push(cartao(lado, COR_PAINEL_FAIXA(), Length::FILL))
                .push(alca(Divisa::Lado));
        }
        cartoes = cartoes.push(centro);
        if self.agente_aberto {
            cartoes = cartoes
                .push(alca(Divisa::Agente))
                .push(cartao(self.agente_view(), COR_PAINEL_FAIXA(), Length::FILL));
        }
        let meio = row![
            self.trilho_view(),
            container(cartoes)
                .padding(Padding {
                    top: 0.0,
                    right: VAO,
                    bottom: VAO,
                    left: 0.0,
                })
                .width(Length::FILL)
                .height(Length::FILL)
        ]
        .height(Length::FILL);
        column![barra, meio, self.status_view()]
            .height(Length::FILL)
            .into()
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
                Space::new(12.0, 0.0).into()
            };
            let ativo = aberto == Some(l.caminho.as_path());
            let msg = if l.pasta {
                Msg::Alternar(l.caminho)
            } else {
                Msg::AbrirArquivo(l.caminho)
            };
            button(
                row![
                    Space::new(l.nivel as f32 * 14.0, 0.0),
                    seta,
                    text(nome).size(13).no_wrap()
                ]
                .spacing(4)
                .width(Length::FILL)
                .align(Align::Center),
            )
            .on_press(msg)
            .width(Length::FILL)
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
        let filtro = text_input("Procurar arquivo", self.filtro.as_str())
            .id("filtro")
            .on_input(Msg::Filtro)
            .size(12)
            .padding([5, 8])
            .style(campo(COR_PAINEL()));
        // Com o filtro, a lista vira os arquivos de todas as pastas cujo nome contém o texto.
        let lista: Element<Msg> = if self.filtro.trim().is_empty() {
            column(linhas).width(Length::FILL).into()
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
                    ]
                    .width(Length::FILL))
                    .on_press(Msg::AbrirArquivo(p.clone()))
                    .width(Length::FILL)
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
                column(itens).width(Length::FILL).into()
            }
        };
        container(column![
            container(
                column![
                    row![
                        container(titulo).width(Length::FILL),
                        tooltip(
                            botao_icone("refresh", Msg::AtualizarArvore),
                            "Atualizar arquivos",
                            TooltipPosition::Bottom
                        )
                        .style(tema::estilo_dica),
                        tooltip(
                            botao_icone("chevron-up", Msg::RecolherArvore),
                            "Recolher tudo",
                            TooltipPosition::Bottom
                        )
                        .style(tema::estilo_dica),
                    ]
                    .spacing(4)
                    .align(Align::Center),
                    filtro
                ]
                .spacing(8)
            )
            .padding([10, 12])
            .width(Length::FILL),
            rolagem(lista).height(Length::FILL)
        ])
        .width(self.largura_lado())
        .height(Length::FILL)
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
                text(if a.sujo() { "●" } else { "×" })
                    .size(13)
                    .color(COR_TEXTO_MUTED()),
            )
            .on_press(Msg::FecharAba(i))
            .padding([0, 4])
            .style(|_, s| item_lista(false, s, 3.0));
            let aba = button(
                row![text(nome).size(13).no_wrap(), fechar]
                    .spacing(8)
                    .align(Align::Center),
            )
            .on_press(Msg::TrocarAba(i))
            .padding(Padding {
                top: 6.0,
                right: 8.0,
                bottom: 6.0,
                left: 14.0,
            })
            .height(Length::FILL)
            .style(move |_, s| aba_estilo(ativa, s));
            // O traço embaixo marca a aba ativa.
            let traco = container(Space::new(0.0, 2.0))
                .width(Length::FILL)
                .style(move |_| {
                    fundo(if ativa {
                        COR_AZUL_ACENTO()
                    } else {
                        Color::TRANSPARENT
                    })
                });
            stack![
                aba,
                column![Space::new(0.0, Length::FILL), traco].height(Length::FILL)
            ]
            .height(Length::FILL)
            .into()
        });
        let arquivo = self.aba().map(|a| a.caminho.as_path());
        let rodar: Element<Msg> =
            if let Some(cmd) = execucao::comando_do_projeto(&self.arvore.raiz, arquivo) {
                botao_rodar("Rodar", VERDE, Msg::RodarProjeto, format!("{cmd}  (F5)"))
            } else {
                Space::new(0.0, 0.0).into()
            };
        container(
            row![
                row(abas).height(Length::FILL),
                horizontal_space(),
                container(rodar).padding([0, 10])
            ]
            .height(Length::FILL)
            .align(Align::Center),
        )
        .width(Length::FILL)
        .height(ALTURA_ABAS)
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
            .align(Align::Center);
            return container(vazio)
                .center()
                .width(Length::FILL)
                .height(Length::FILL)
                .style(|_| fundo(COR_PAINEL()))
                .into();
        };
        let extensao = aba
            .caminho
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt");
        // O editor do pelicano rola, numera as linhas e desfaz sozinho; o app só cuida dos atalhos.
        let mut editor = text_editor(&aba.conteudo)
            .id("editor")
            // O chat também é um editor: sem clique, nenhum dos dois pega a digitação.
            .idle_focus(false)
            .on_key(|k, m| {
                let ctrl = m.control_key() || m.super_key();
                match k {
                    Key::Named(NamedKey::Tab) if !ctrl => Some(Msg::Recuar(!m.shift_key())),
                    Key::Character(c) if ctrl && matches!(c.as_str(), ";" | "/") => {
                        Some(Msg::Comentar)
                    }
                    _ => None,
                }
            })
            .font(MONO)
            .size(TAMANHO_FONTE)
            .line_height(ALTURA_LINHA)
            .wrap(false)
            .line_numbers(true)
            .padding(Padding {
                top: MARGEM_V,
                bottom: MARGEM_V,
                left: 4.0,
                right: MARGEM_H,
            })
            .highlight(extensao, self.tema_codigo())
            // Linha do cursor: um pouco mais clara no tema escuro e mais escura no claro (a cor
            // do texto, quase transparente, faz as duas coisas).
            .current_line(COR_TEXTO().alpha(0.05))
            .style(|_, _| Style::default().background(COR_PAINEL()).text_color(COR_TEXTO()));
        if self.config.linhas_alternadas {
            editor = editor.stripes(COR_TEXTO().alpha(ALFA_LISTRAS));
        }
        // O editor do pelicano não usa o botão direito: ele chega aqui e abre o menu.
        mouse_area(
            container(editor)
                .width(Length::FILL)
                .height(Length::FILL)
                .style(|_| fundo(COR_PAINEL())),
        )
        .on_right_press_at(|p| Msg::MenuContexto(Some(p)))
        .into()
    }

    /// Diferença contra o último commit, em linha (como o modo inline do VS Code).
    fn diff_view<'a>(&'a self, d: &'a paineis::Diff) -> Element<'a, Msg> {
        const MAX: usize = 5000;
        let (mais, menos) = d.linhas.iter().fold((0, 0), |(m, n), l| match l.tipo {
            '+' => (m + 1, n),
            '-' => (m, n + 1),
            _ => (m, n),
        });
        let cab = container(
            row![
                text(self.relativo(&d.caminho).display().to_string())
                    .size(13)
                    .font(NEGRITO),
                text(match &d.commit {
                    Some(h) => format!("no commit {h}"),
                    None => "alterações desde o último commit".into(),
                })
                .size(12)
                .color(COR_TEXTO_MUTED()),
                text(format!("+{mais}")).size(12).font(MONO).color(VERDE),
                text(format!("−{menos}"))
                    .size(12)
                    .font(MONO)
                    .color(COR_ERRO()),
                horizontal_space(),
                botao_texto(
                    row![
                        icone_traco("file-code", 14.0),
                        text("Abrir arquivo").size(12)
                    ]
                    .spacing(6)
                    .align(Align::Center),
                    Msg::AbrirArquivo(d.caminho.clone()),
                ),
                botao_icone("x", Msg::FecharDiff),
            ]
            .spacing(10)
            .align(Align::Center),
        )
        .padding([0, 12])
        .width(Length::FILL)
        .height(32.0)
        .align_y(Align::Center)
        .style(|_| fundo(COR_CROMO_2()));
        let numero = |n: Option<usize>| {
            container(
                text(n.map_or(String::new(), |n| n.to_string()))
                    .size(TAMANHO_FONTE - 1.0)
                    .font(MONO)
                    .color(COR_TEXTO_MUTED()),
            )
            .width(44.0)
            .align_x(Align::End)
        };
        let mut lista = column![].width(Length::FILL);
        for l in d.linhas.iter().take(MAX) {
            let (fundo_linha, sinal, cor_sinal) = match l.tipo {
                '+' => (VERDE.alpha(0.16), "+", VERDE),
                '-' => (COR_ERRO().alpha(0.16), "−", COR_ERRO()),
                '@' => (COR_AZUL_ACENTO().alpha(0.10), "", COR_TEXTO_MUTED()),
                _ => (Color::TRANSPARENT, "", COR_TEXTO_MUTED()),
            };
            let corpo: Element<Msg> = if l.tipo == '@' {
                text(if l.texto.is_empty() {
                    "⋯".to_string()
                } else {
                    format!("⋯  {}", l.texto)
                })
                .size(TAMANHO_FONTE - 1.0)
                .font(MONO)
                .color(COR_TEXTO_MUTED())
                .no_wrap()
                .into()
            } else {
                text(l.texto.as_str())
                    .size(TAMANHO_FONTE)
                    .font(MONO)
                    .no_wrap()
                    .into()
            };
            lista = lista.push(
                container(
                    row![
                        numero(l.antes),
                        numero(l.depois),
                        container(text(sinal).size(TAMANHO_FONTE).font(MONO).color(cor_sinal))
                            .width(22.0)
                            .align_x(Align::Center),
                        corpo
                    ]
                    .spacing(6)
                    .align(Align::Center),
                )
                .width(Length::FILL)
                .height(ALTURA_LINHA)
                .align_y(Align::Center)
                .clip(true)
                .style(move |_| fundo(fundo_linha)),
            );
        }
        let corpo: Element<Msg> = if d.linhas.is_empty() {
            container(
                text("Sem diferença de texto (arquivo binário ou vazio).")
                    .size(13)
                    .color(COR_TEXTO_MUTED()),
            )
            .center()
            .width(Length::FILL)
            .height(Length::FILL)
            .into()
        } else {
            if d.linhas.len() > MAX {
                lista = lista.push(
                    container(
                        text(format!("Mostrando as primeiras {MAX} linhas da diferença."))
                            .size(12)
                            .color(COR_TEXTO_MUTED()),
                    )
                    .padding(10),
                );
            }
            rolagem(lista.padding([6, 0]))
                .height(Length::FILL)
                .width(Length::FILL)
                .into()
        };
        column![
            cab,
            linha_horizontal(),
            container(corpo)
                .width(Length::FILL)
                .height(Length::FILL)
                .style(|_| fundo(COR_PAINEL()))
        ]
        .width(Length::FILL)
        .height(Length::FILL)
        .into()
    }

    /// Os agentes que dá para escolher: o Claude Code sempre (é o padrão), os outros se instalados.
    fn agentes_instalados(&self) -> Vec<agente::Provedor> {
        agente::Provedor::TODOS
            .into_iter()
            .filter(|p| {
                *p == agente::Provedor::Claude
                    || *p == self.chat.provedor
                    || self
                        .ini
                        .outros
                        .iter()
                        .any(|(nome, _)| *nome == p.to_string())
            })
            .collect()
    }

    fn explicacao_agente(&self) -> String {
        match self.chat.provedor {
            agente::Provedor::Claude => "O Claude Code roda na pasta aberta, com a sua conta. Antes de gravar um arquivo ou rodar um comando, ele pede permissão aqui.".into(),
            agente::Provedor::Codex => "O Codex roda na pasta aberta, com a sua conta da OpenAI. No modo \"Perguntar antes\", ele pede permissão aqui antes de gravar ou rodar comandos.".into(),
            agente::Provedor::Antigravity => {
                let mut t = String::from("O Antigravity roda na pasta aberta, com a sua conta do Google. Por aqui ele não consegue pedir permissão: para gravar arquivos, escolha \"Aceitar edições\" antes de mandar a primeira mensagem.");
                let raiz = self.arvore.raiz.to_string_lossy();
                if raiz.starts_with("/run/media/") || raiz.starts_with("/media/") {
                    t.push_str(" Atenção: o agy instalado pelo snap não enxerga pastas em /run/media; nesta pasta ele não consegue ler os arquivos. Instale o agy sem snap para usar aqui.");
                }
                t
            }
        }
    }

    fn agente_view(&self) -> Element<'_, Msg> {
        let titulo = self
            .chat
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
                text(titulo).size(13).font(NEGRITO).ellipsis(),
                horizontal_space(),
                // O histórico lê as sessões gravadas pelo Claude Code; os outros guardam do jeito deles.
                if self.chat.provedor == agente::Provedor::Claude {
                    botao_icone("history", Msg::Historico)
                } else {
                    Space::new(0.0, 0.0).into()
                },
                botao_icone("info-circle", Msg::Detalhes),
                botao_icone("plus", Msg::NovaConversa),
            ]
            .spacing(2)
            .align(Align::Center),
        )
        .padding(Padding {
            top: 0.0,
            right: 8.0,
            bottom: 0.0,
            left: 14.0,
        })
        .width(Length::FILL)
        .height(ALTURA_ABAS)
        .align_y(Align::Center);

        let mut chats: Vec<&Chat> = std::iter::once(&self.chat)
            .chain(self.outros_chats.iter())
            .collect();
        chats.sort_by_key(|c| c.conversa);
        let mut abas_chat = row![].spacing(4);
        for chat in chats {
            let pendente = chat.itens.iter().any(|i| {
                matches!(
                    i,
                    Item::Pedido {
                        resposta: Resposta::Pendente,
                        ..
                    }
                )
            });
            let estado = if pendente {
                "Permissão"
            } else if chat.ocupado {
                "Rodando…"
            } else {
                "Pronto"
            };
            let titulo = chat
                .itens
                .iter()
                .find_map(|i| match i {
                    Item::Eu(t) => Some(t.chars().take(18).collect::<String>()),
                    _ => None,
                })
                .unwrap_or_else(|| format!("Chat {}", chat.conversa + 1));
            let ativo = chat.conversa == self.chat.conversa;
            let fechar = button(text("×").size(12).color(COR_TEXTO_MUTED()))
                .on_press(Msg::FecharChat(chat.conversa))
                .padding([0, 4])
                .style(|_, s| item_lista(false, s, 3.0));
            abas_chat = abas_chat.push(
                button(
                    row![text(format!("{titulo} · {estado}")).size(11).no_wrap(), fechar]
                        .spacing(6)
                        .align(Align::Center),
                )
                .on_press(Msg::TrocarChat(chat.conversa))
                .padding([2, 8])
                .style(move |_, s| item_lista(ativo, s, 4.0)),
            );
        }
        // Com uma conversa só, a barra de abas não aparece.
        let abas_chat: Element<Msg> = if self.outros_chats.is_empty() {
            Space::new(0.0, 0.0).into()
        } else {
            container(
                scrollable(abas_chat)
                    .direction(Direction::Horizontal)
                    .scrollbar_width(3.0)
                    .scrollbar_color(COR_BORDA())
                    .width(Length::FILL),
            )
            .padding([4, 8])
            .width(Length::FILL)
            .into()
        };
        let mut corpo = column![].spacing(12).padding([14, 14]).width(Length::FILL);
        if self.chat.detalhes {
            corpo = corpo.push(self.detalhes_view());
        }
        if let Some(lista) = &self.chat.historico {
            corpo = corpo.push(historico_view(lista));
        } else if self.chat.itens.is_empty() {
            corpo = corpo.push(
                column![
                    text("Peça uma mudança ou pergunte sobre o código.").size(13),
                    text(self.explicacao_agente())
                        .size(12)
                        .color(COR_TEXTO_MUTED()),
                ]
                .spacing(6)
                .width(Length::FILL),
            );
        }
        // Passos seguidos de ferramenta ficam juntos numa linha do tempo, com o traço à esquerda.
        let mut passos: Vec<Element<Msg>> = Vec::new();
        // Rótulo de quem fala a cada troca: "Você" ou o nome do agente.
        let mut falou_eu: Option<bool> = None;
        for (i, item) in self.chat.itens.iter().enumerate() {
            let eu = matches!(item, Item::Eu(_));
            if !matches!(item, Item::Aviso(_)) && falou_eu != Some(eu) {
                if !passos.is_empty() {
                    corpo = corpo.push(linha_do_tempo(std::mem::take(&mut passos)));
                }
                let quem = if eu {
                    "Você".to_string()
                } else {
                    self.chat.provedor.to_string()
                };
                corpo = corpo.push(rotulo_de_quem(quem, eu));
                falou_eu = Some(eu);
            }
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
        if self.chat.ocupado {
            corpo = corpo.push(text("Trabalhando…").size(12).color(COR_TEXTO_MUTED()));
        }

        let chip: Element<Msg> = match self.aba() {
            Some(a) => {
                let (l, _) = a.conteudo.cursor();
                container(
                    text(format!("{}:{l}", self.relativo(&a.caminho).display()))
                        .font(MONO)
                        .size(11)
                        .color(COR_TEXTO_MUTED())
                        .ellipsis(),
                )
                .padding([1, 6])
                .style(|_| Style::default().border(Border::rounded(4.0).color(COR_BORDA(), 1.0)))
                .into()
            }
            None => Space::new(0.0, 0.0).into(),
        };
        let acao = if self.chat.ocupado {
            button(
                container(icone_traco("player-stop", 13.0))
                    .center()
                    .width(22.0)
                    .height(22.0),
            )
            .on_press(Msg::Parar)
            .padding(0)
            .style(|_, s| item_lista(false, s, 5.0))
        } else {
            button(
                container(text("↑").size(14).color(COR_PAINEL()))
                    .center()
                    .width(22.0)
                    .height(22.0),
            )
            .on_press(Msg::Enviar)
            .padding(0)
            .style(botao_enviar)
        };
        let clipe: Element<Msg> = if self.chat.provedor == agente::Provedor::Claude {
            tooltip(
                botao_icone("photo", Msg::Anexar),
                "Mandar imagem (ou solte o arquivo na janela)",
                TooltipPosition::Top,
            )
            .padding([3, 8])
            .style(tema::estilo_dica)
            .into()
        } else {
            Space::new(0.0, 0.0).into()
        };
        let agente = pick_list(&self.agentes, Some(self.chat.provedor), Msg::Provedor)
            .size(12)
            .padding([2, 4])
            .style(lista_sem_borda)
            .handle_color(COR_TEXTO_MUTED());
        let modo = pick_list(&Modo::TODOS, Some(self.chat.modo), Msg::Modo)
            .size(12)
            .padding([2, 4])
            .style(lista_sem_borda)
            .handle_color(COR_TEXTO_MUTED());
        let fim = row![
            text(nome_modelo(&self.chat.info.modelo))
                .size(12)
                .color(COR_TEXTO_MUTED())
                .no_wrap(),
            self.anel(),
            acao,
        ]
        .spacing(4)
        .align(Align::Center);
        // Painel estreito (arrastado): os seletores sobem para uma linha própria, senão cobrem o botão.
        let rodape: Element<Msg> = if self.largura_agente >= LARGURA_AGENTE {
            row![self.bolinha(), clipe, agente, modo, horizontal_space(), fim]
                .spacing(4)
                .align(Align::Center)
                .into()
        } else {
            column![
                row![agente, modo].spacing(4).align(Align::Center),
                row![self.bolinha(), clipe, horizontal_space(), fim]
                    .spacing(4)
                    .align(Align::Center),
            ]
            .spacing(4)
            .width(Length::FILL)
            .into()
        };
        // Cresce com o texto até umas 8 linhas; dali em diante rola por dentro.
        let linhas = self.chat.pedido.line_count().clamp(1, 8) as f32;
        let pedido = text_editor(&self.chat.pedido)
            .id("pedido")
            .idle_focus(false)
            .placeholder("Peça uma mudança ou pergunte…")
            // Enter manda; Shift+Enter cai no padrão, que é quebrar a linha.
            .on_key(|k, m| {
                (matches!(k, Key::Named(NamedKey::Enter)) && !m.shift_key()).then_some(Msg::Enviar)
            })
            .size(13.0)
            .padding([6, 10])
            .style(|_, _| Style::default().text_color(COR_TEXTO()));
        let entrada = container(column![
            container(chip).padding(Padding {
                top: 8.0,
                right: 10.0,
                bottom: 0.0,
                left: 10.0
            }),
            self.anexos_view(),
            container(pedido)
                .width(Length::FILL)
                .height(linhas * 19.0 + 12.0),
            linha_horizontal(),
            container(rodape).padding([5, 8]).width(Length::FILL),
        ])
        .width(Length::FILL)
        .style(|_| fundo(COR_PAINEL()).border(Border::rounded(6.0).color(COR_BORDA(), 1.0)));

        container(column![
            topo,
            abas_chat,
            linha_horizontal(),
            rolagem(corpo).id("chat").height(Length::FILL),
            container(entrada)
                .padding(Padding {
                    top: 0.0,
                    right: 10.0,
                    bottom: 10.0,
                    left: 10.0
                })
                .width(Length::FILL),
        ])
        .width(self.largura_agente)
        .height(Length::FILL)
        .style(|_| fundo(COR_PAINEL_FAIXA()))
        .into()
    }

    /// Imagens anexadas ao próximo pedido, cada uma com o × para tirar.
    fn anexos_view<'a>(&'a self) -> Element<'a, Msg> {
        if self.chat.imagens.is_empty() && self.chat.trechos.is_empty() {
            return Space::new(0.0, 0.0).into();
        }
        let chip = |icone: &str, nome: &'a str, tirar: Msg| -> Element<'a, Msg> {
            container(
                row![
                    icone_traco(icone, 12.0),
                    text(nome).size(11).font(MONO),
                    button(text("×").size(12).color(COR_TEXTO_MUTED()))
                        .on_press(tirar)
                        .padding([0, 4])
                        .style(|_, s| item_lista(false, s, 3.0)),
                ]
                .spacing(4)
                .align(Align::Center),
            )
            .padding([1, 6])
            .style(|_| Style::default().border(Border::rounded(4.0).color(COR_BORDA(), 1.0)))
            .into()
        };
        let trechos = self.chat.trechos.iter().enumerate();
        let imagens = self.chat.imagens.iter().enumerate();
        let chips: Vec<Element<Msg>> = trechos
            .map(|(i, (rotulo, _))| chip("code", rotulo, Msg::TirarTrecho(i)))
            .chain(imagens.map(|(i, (nome, _))| chip("photo", nome, Msg::TirarImagem(i))))
            .collect();
        container(row(chips).spacing(6).wrap())
            .padding(Padding {
                top: 6.0,
                right: 10.0,
                bottom: 0.0,
                left: 10.0,
            })
            .width(Length::FILL)
            .into()
    }

    /// Estado do agente na bolinha ao lado do nome, com a frase na dica.
    fn estado(&self) -> (Color, &'static str) {
        let esperando = self.chat.itens.iter().any(|i| {
            matches!(
                i,
                Item::Pedido {
                    resposta: Resposta::Pendente,
                    ..
                }
            )
        });
        match (self.chat.ligado, self.chat.ocupado) {
            (false, _) if matches!(self.chat.itens.last(), Some(Item::Aviso(_))) => {
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
        tooltip(text("●").size(10).color(cor), frase, TooltipPosition::Top)
            .padding([3, 8])
            .style(tema::estilo_dica)
            .into()
    }

    /// Anel do contexto usado, como no app do Claude; a dica mostra contexto, sessão e semana.
    fn anel(&self) -> Element<'_, Msg> {
        let (usados, janela) = self.chat.contexto;
        let fracao = if janela > 0 {
            usados as f32 / janela as f32
        } else {
            0.0
        };
        let desenho = canvas(16.0, 16.0, move |r, area, _| anel(r, area, fracao));
        // shortcut: a dica do pelicano é só texto; as barras de uso ficam no painel de detalhes (ⓘ).
        let dica = if usados == 0 && self.chat.uso.0.is_none() {
            "O uso aparece depois da primeira resposta.".to_string()
        } else {
            self.usos()
                .into_iter()
                .map(|(rotulo, _, valor)| format!("{rotulo}: {valor}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        tooltip(desenho, dica, TooltipPosition::Top)
            .padding([8, 10])
            .style(tema::estilo_dica)
            .into()
    }

    /// Contexto, sessão de 5 h e semana: (rótulo, fração usada, valor escrito).
    fn usos(&self) -> Vec<(&'static str, f32, String)> {
        let mut v = Vec::new();
        let (usados, janela) = self.chat.contexto;
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
            v.push(("Contexto", fracao, valor));
        }
        if let Some(u) = self.chat.uso.0 {
            v.push(("Sessão (5 h)", u, format!("{:.0}%", u * 100.0)));
        }
        if let Some(u) = self.chat.uso.1 {
            v.push(("Semana (7 dias)", u, format!("{:.0}%", u * 100.0)));
        }
        v
    }

    fn barras_view(&self) -> Element<'_, Msg> {
        column(
            self.usos()
                .into_iter()
                .map(|(rotulo, fracao, valor)| barra(rotulo, fracao, valor)),
        )
        .spacing(8)
        .width(Length::FILL)
        .into()
    }

    fn detalhes_view(&self) -> Element<'_, Msg> {
        let i = &self.chat.info;
        let rotulo = |t: &'static str| text(t).size(11).font(NEGRITO).color(COR_TEXTO_MUTED());
        let mut c = column![self.barras_view()].spacing(10).width(Length::FILL);
        if i.versao.is_empty() {
            c = c.push(
                text("Os detalhes aparecem depois da primeira mensagem, quando o agente abre.")
                    .size(12)
                    .color(COR_TEXTO_MUTED()),
            );
        } else {
            let quem = match self.chat.provedor {
                agente::Provedor::Claude => format!("Claude Code {}", i.versao),
                _ => i.versao.clone(),
            };
            c = c.push(text(format!("{} · {quem}", i.modelo)).size(12));
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
                    text(nome.as_str()).size(12),
                    horizontal_space(),
                    text(frase).size(11).color(COR_TEXTO_MUTED())
                ]
                .spacing(8)
                .align(Align::Center)
                .width(Length::FILL)
                .into()
            });
            c = c.push(
                column![
                    rotulo("SERVIDORES MCP"),
                    column(mcp).spacing(3).width(Length::FILL)
                ]
                .spacing(4)
                .width(Length::FILL),
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
            .width(Length::FILL)
            .style(|_| fundo(COR_PAINEL()).border(Border::rounded(6.0).color(COR_BORDA(), 1.0)))
            .into()
    }

    fn status_view(&self) -> Element<'_, Msg> {
        let (pos, lingua) = match self.aba() {
            Some(a) => {
                let (l, c) = a.conteudo.cursor();
                (format!("Ln {l}, Col {c}"), linguagem(&a.caminho))
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
            text(self.aviso.as_str())
                .size(11)
                .color(COR_ERRO())
                .ellipsis()
                .into()
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
            .align(Align::Center),
        )
        .padding([0, 12])
        .width(Length::FILL)
        .height(ALTURA_STATUS)
        .align_y(Align::Center)
        .style(|_| fundo(COR_FUNDO_JANELA()).text_color(COR_TEXTO_MUTED()))
        .into()
    }
}

fn item_view(i: usize, item: &Item) -> Element<'_, Msg> {
    match item {
        // Clicar na mensagem a devolve ao campo, para mandar de novo ou corrigir.
        Item::Eu(t) => mouse_area(
            container(text(t.as_str()).size(13))
                .padding([8, 10])
                .width(Length::FILL)
                .style(|_| {
                    fundo(mistura_cor(COR_PAINEL(), COR_AZUL_ACENTO(), 0.08)).border(
                        Border::rounded(6.0)
                            .color(mistura_cor(COR_BORDA(), COR_AZUL_ACENTO(), 0.4), 1.0),
                    )
                }),
        )
        .on_press(Msg::Pedido(t.clone()))
        .cursor_icon(CursorIcon::Pointer)
        .into(),
        Item::Texto(fonte) => {
            let copiar = tooltip(
                botao_icone("copy", Msg::Copiar(fonte.clone())),
                "Copiar resposta",
                TooltipPosition::Top,
            )
            .padding([3, 8])
            .style(tema::estilo_dica);
            column![markdown(fonte, 13.0, Msg::Link), copiar]
                .spacing(4)
                .width(Length::FILL)
                .into()
        }
        Item::Ferramenta { frase, erro, .. } => passo(frase, *erro),
        Item::Aviso(t) => text(t.as_str()).size(12).color(COR_ERRO()).into(),
        Item::Pedido {
            frase,
            previa,
            resposta,
            ..
        } => {
            let mut cartao = column![container(text(frase.as_str()).size(12).font(MONO))
                .padding([7, 10])
                .width(Length::FILL)]
            .width(Length::FILL);
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
                            .no_wrap(),
                    )
                    .padding([1, 10])
                    .style(move |_| fundo(faixa))
                    .into()
                });
                // Linha comprida rola de lado em vez de quebrar: o diff tem que ser lido como está.
                let diff = scrollable(column(linhas))
                    .direction(Direction::Horizontal)
                    .scrollbar_width(4.0)
                    .scrollbar_color(COR_BORDA())
                    .width(Length::FILL);
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
                .width(Length::FILL)
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
                .width(Length::FILL)
                .style(move |_| {
                    fundo(COR_PAINEL()).border(Border::rounded(6.0).color(
                        if pendente {
                            COR_AZUL_ACENTO()
                        } else {
                            COR_BORDA()
                        },
                        1.0,
                    ))
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
    .align(Align::Center)
    .into()
}

fn linha_do_tempo(passos: Vec<Element<'_, Msg>>) -> Element<'_, Msg> {
    column(passos)
        .spacing(6)
        .padding(Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 4.0,
        })
        .width(Length::FILL)
        .into()
}

fn opcao<'a>(tecla: &'a str, rotulo: &'a str, msg: Msg, foco: bool) -> Element<'a, Msg> {
    button(
        row![text(tecla).font(MONO).size(11), text(rotulo).size(12)]
            .spacing(10)
            .width(Length::FILL),
    )
        .on_press(msg)
        .width(Length::FILL)
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

/// O anel: o círculo cinza e, por cima, o arco do quanto já foi usado.
fn anel(r: &mut pelicano::Renderer, area: pelicano::Rect, fracao: f32) {
    use pelicano::{LineCap, Path, PathBuilder, Stroke};
    let centro = Point::new(area.x + area.width / 2.0, area.y + area.height / 2.0);
    let raio = area.width.min(area.height) / 2.0 - 2.0;
    if let Some(c) = Path::circle(centro, raio) {
        r.stroke_path(&c, &Stroke::new(2.5), COR_BORDA());
    }
    let f = fracao.clamp(0.0, 1.0);
    if f <= 0.0 {
        return;
    }
    let cor = if f > 0.9 {
        COR_ERRO()
    } else if f > 0.75 {
        AMARELO
    } else {
        COR_AZUL_ACENTO()
    };
    let inicio = -std::f32::consts::FRAC_PI_2;
    let mut arco = PathBuilder::new();
    arco.arc(
        centro,
        Point::new(raio, raio),
        inicio,
        inicio + f * std::f32::consts::TAU,
    );
    if let Some(arco) = arco.build() {
        r.stroke_path(&arco, &Stroke::new(2.5).cap(LineCap::Round), cor);
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
    let mut c = column![text("CONVERSAS DESTA PASTA")
        .size(10)
        .font(NEGRITO)
        .color(COR_TEXTO_MUTED())]
    .spacing(2)
    .width(Length::FILL);
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
                    text(s.titulo.as_str()).size(13),
                    text(config::ha_quanto(s.quando, agora))
                        .size(11)
                        .color(COR_TEXTO_MUTED())
                ]
                .spacing(1)
                .width(Length::FILL),
            )
            .on_press(Msg::Retomar(s.id.clone()))
            .width(Length::FILL)
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
        ]
        .width(Length::FILL),
        progress_bar(0.0..=1.0, u)
            .width(Length::FILL)
            .height(4.0)
            .color(COR_AZUL_ACENTO()),
    ]
    .spacing(4)
    .width(Length::FILL)
    .into()
}

/// A pena da marca sem o quadrado de fundo, para ficar sobre a cor do painel.
fn pena<'a>(tamanho: f32) -> Element<'a, Msg> {
    let svg = marca::svg_para(marca::produto_atual(), true);
    let sem_fundo = match (
        svg.find("<rect id=\"fundo-arredondado\""),
        svg.find("<g id=\"penas\""),
    ) {
        (Some(a), Some(b)) if a < b => format!("{}{}", &svg[..a], &svg[b..]),
        _ => svg,
    };
    match pelicano::Svg::from_bytes(sem_fundo.as_bytes()) {
        Some(s) => pelicano::svg(s).width(tamanho).height(tamanho).into(),
        None => Space::new(tamanho, tamanho).into(),
    }
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
            .align(Align::Center),
    )
    .on_press(msg)
    .padding([4, 10])
    .style(move |_, s| {
        Style::default()
            .background(if matches!(s, Status::Hovered) {
                COR_HOVER()
            } else {
                COR_PAINEL()
            })
            .text_color(COR_TEXTO())
            .border(Border::rounded(6.0).color(COR_BORDA(), 1.0))
    });
    tooltip(b, dica, TooltipPosition::Bottom)
        .padding([4, 8])
        .style(tema::estilo_dica)
        .into()
}

fn cursor_da_divisa(d: Divisa) -> CursorIcon {
    if d == Divisa::Terminal {
        CursorIcon::RowResize
    } else {
        CursorIcon::ColResize
    }
}

/// O vão de `VAO` entre dois cartões; arrastar muda o tamanho do painel.
fn alca<'a>(d: Divisa) -> Element<'a, Msg> {
    let espaco = if d == Divisa::Terminal {
        Space::new(Length::FILL, VAO)
    } else {
        Space::new(VAO, Length::FILL)
    };
    mouse_area(espaco)
        .on_press(Msg::Redimensionar(Some(d)))
        .cursor_icon(cursor_da_divisa(d))
        .into()
}

fn mistura_cor(a: Color, b: Color, t: f32) -> Color {
    Color {
        a: 1.0,
        ..a.mix(b, t)
    }
}

fn fundo(cor: Color) -> Style {
    Style::default().background(cor).text_color(COR_TEXTO())
}

fn linha_vertical<'a>() -> Element<'a, Msg> {
    container(Space::new(1.0, 0.0))
        .height(Length::FILL)
        .style(|_| fundo(COR_BORDA_SUAVE()))
        .into()
}

fn linha_horizontal<'a>() -> Element<'a, Msg> {
    container(Space::new(0.0, 1.0))
        .width(Length::FILL)
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
    button(
        container(icone_traco(nome, 14.0))
            .center()
            .width(22.0)
            .height(22.0),
    )
    .on_press(msg)
    .padding(0)
    .style(|_, s| item_lista(false, s, 4.0))
    .into()
}

fn item_lista(ativo: bool, s: Status, raio: f32) -> Style {
    let estilo = Style::default()
        .text_color(if ativo {
            COR_AZUL_ACENTO()
        } else {
            COR_TEXTO()
        })
        .border(Border::rounded(raio));
    match (ativo, s) {
        (true, _) => estilo.background(COR_AZUL_FUNDO()),
        (_, Status::Hovered | Status::Pressed) => estilo.background(COR_HOVER()),
        _ => estilo,
    }
}

/// Painel em cartão: cantos arredondados e borda suave. `cor` é o fundo do painel, para os cantos
/// não ficarem quadrados por baixo do que ele desenha.
fn cartao<'a>(conteudo: impl Into<Element<'a, Msg>>, cor: Color, altura: Length) -> Element<'a, Msg> {
    container(conteudo)
        .padding(1)
        .clip(true)
        .height(altura)
        .style(move |_| fundo(cor).border(Border::rounded(8.0).color(COR_BORDA_SUAVE(), 1.0)))
        .into()
}

fn aba_estilo(ativa: bool, s: Status) -> Style {
    let estilo = Style::default().text_color(if ativa {
        COR_TEXTO()
    } else {
        COR_TEXTO_MUTED()
    });
    if !ativa && matches!(s, Status::Hovered) {
        estilo.background(COR_HOVER())
    } else {
        estilo
    }
}

fn botao_enviar(_: &Theme, s: Status) -> Style {
    let cor = if matches!(s, Status::Hovered) {
        mistura_cor(COR_AZUL_ACENTO(), COR_TEXTO(), 0.15)
    } else {
        COR_AZUL_ACENTO()
    };
    Style::default().background(cor).border(Border::rounded(5.0))
}

/// "Você" ou o nome do agente, em cima de cada troca do chat.
fn rotulo_de_quem<'a>(quem: String, eu: bool) -> Element<'a, Msg> {
    row![
        text("●")
            .size(8)
            .color(if eu { COR_AZUL_ACENTO() } else { VERDE }),
        text(quem).size(11).font(NEGRITO).color(COR_TEXTO_MUTED()),
    ]
    .spacing(6)
    .align(Align::Center)
    .into()
}

/// Campo de texto com borda que acende no foco, sobre o fundo `cor`.
fn campo(cor: Color) -> impl Fn(&Theme, Status) -> Style {
    move |_, s| {
        let borda = if s == Status::Focused {
            COR_AZUL_ACENTO()
        } else {
            COR_BORDA()
        };
        fundo(cor).border(Border::rounded(6.0).color(borda, 1.0))
    }
}

fn lista_sem_borda(_: &Theme, s: Status) -> Style {
    let estilo = Style::default()
        .text_color(COR_TEXTO_MUTED())
        .border(Border::rounded(4.0));
    if matches!(s, Status::Hovered | Status::Pressed | Status::Focused) {
        estilo.background(COR_HOVER())
    } else {
        estilo
    }
}

/// Rolagem com a barra fina e discreta, como nos editores de código.
fn rolagem<'a>(conteudo: impl Into<Element<'a, Msg>>) -> pelicano::Scrollable<'a, Msg> {
    tema::rolagem(conteudo)
        .scrollbar_width(6.0)
        .scrollbar_color(COR_BORDA())
}

#[cfg(test)]
mod testes {
    #[test]
    fn chats_recebem_eventos_sem_perder_a_aba_ativa() {
        use super::*;
        let (mut code, _) = Code::novo(config::Config::ler());
        code.chat.pedido = Content::with_text("rascunho do primeiro");
        code.chat.ocupado = true;
        code.chat.ligado = true;
        let primeiro = code.chat.conversa;
        let _ = code.update(Msg::NovaConversa);
        let segundo = code.chat.conversa;
        code.chat.pedido = Content::with_text("rascunho do segundo");
        assert_ne!(primeiro, segundo);
        assert!(code.outros_chats[0].ligado);
        let (canal, mut receptor) = tokio::sync::mpsc::unbounded_channel();
        let _ = code.update(Msg::EventoChat(primeiro, agente::Evento::Pronto(canal)));
        assert_eq!(code.chat.conversa, segundo);
        assert!(code.chat.canal.is_none());
        let _ = code.update(Msg::TrocarChat(primeiro));
        assert_eq!(code.chat.pedido.text().trim_end(), "rascunho do primeiro");
        code.enviar("mensagem do primeiro".into());
        assert_eq!(receptor.try_recv().unwrap(), "mensagem do primeiro");
        let _ = code.update(Msg::TrocarChat(segundo));
        let _ = code.update(Msg::EventoChat(primeiro, agente::Evento::Encerrado(None)));
        assert_eq!(code.chat.conversa, segundo);
        assert_eq!(code.chat.pedido.text().trim_end(), "rascunho do segundo");
        assert!(!code.outros_chats[0].ocupado);
        assert!(!code.outros_chats[0].ligado);
    }

    #[test]
    fn trecho_vai_no_pedido_com_o_rotulo() {
        use super::*;
        let (mut code, _) = Code::novo(config::Config::ler());
        let conteudo = Content::with_text("fn a() {}\nfn b() {}\n");
        code.abas.push(Aba {
            caminho: code.arvore.raiz.join("x.rs"),
            salvo: conteudo.version(),
            conteudo,
        });
        let _ = code.update(Msg::MandarTrecho);
        assert_eq!(code.chat.trechos[0].0, "x.rs 1");
        let _ = code.update(Msg::Enviar);
        assert!(code.chat.trechos.is_empty());
        assert!(matches!(code.chat.itens.last(), Some(Item::Eu(t)) if t == "[x.rs 1]"));
        let linha = &code.chat.fila[0];
        assert!(linha.contains("linhas 1 a 1") && linha.contains("fn a() {}"));
    }

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
