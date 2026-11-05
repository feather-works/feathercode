#![cfg_attr(windows, windows_subsystem = "windows")]
//! FeatherCode: editor de código da suíte FeatherWorks, com o Claude Code no painel lateral.
mod adaptadores;
mod agente;
mod arvore;
mod completar;
mod config;
mod edicao;
mod execucao;
mod inicio;
mod modelos;
mod paineis;
mod preferencias;
mod problemas;
mod sistema;
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
        // O X chega como CloseRequested: o app pergunta antes de perder mudança não salva.
        close_on_request: false,
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
    /// Marca do texto no disco da última vez que foi lido ou salvo: diz se outro (o agente) mexeu.
    disco: u64,
    /// O arquivo mudou no disco enquanto a aba tinha mudança não salva.
    conflito: bool,
    /// Markdown mostrado formatado em vez do editor.
    previa: bool,
    /// Em que lado do editor dividido a aba aparece: 1 = esquerda, 2 = direita, 3 = nos dois.
    grupos: u8,
}

impl Aba {
    fn nova(caminho: PathBuf, texto: &str) -> Self {
        let conteudo = Content::with_text(texto);
        Aba {
            caminho,
            salvo: conteudo.version(),
            disco: marca_texto(texto),
            conteudo,
            conflito: false,
            previa: false,
            grupos: 1,
        }
    }

    fn sujo(&self) -> bool {
        self.conteudo.version() != self.salvo
    }

    fn salvar(&mut self) -> std::io::Result<()> {
        let texto = self.conteudo.text();
        std::fs::write(&self.caminho, &texto)?;
        self.salvo = self.conteudo.version();
        self.disco = marca_texto(&texto);
        self.conflito = false;
        Ok(())
    }

    /// Troca o texto pelo que está no disco e esquece a mudança local.
    fn recarregar(&mut self, texto: &str) {
        let (previa, grupos) = (self.previa, self.grupos);
        *self = Aba::nova(std::mem::take(&mut self.caminho), texto);
        self.previa = previa;
        self.grupos = grupos;
    }

    fn markdown(&self) -> bool {
        matches!(
            self.caminho.extension().and_then(|e| e.to_str()),
            Some("md" | "markdown")
        )
    }
}

/// Marca curta de um texto, para comparar com o disco sem guardar uma cópia.
fn marca_texto(t: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.trim_end().hash(&mut h);
    h.finish()
}

/// Quantas vezes `termo` aparece, para a barra de localizar.
fn contar(texto: &str, termo: &str, maiusculas: bool) -> usize {
    if termo.is_empty() {
        0
    } else if maiusculas {
        texto.matches(termo).count()
    } else {
        texto.to_lowercase().matches(&termo.to_lowercase()).count()
    }
}

/// Na resposta do agente, `src/x.rs:42` vira link que abre o arquivo na linha.
/// Só liga caminhos de arquivos que existem na pasta; blocos de código ficam como estão.
fn ligar_caminhos(texto: &str, raiz: &Path) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut bloco = false;
    for (n, linha) in texto.split('\n').enumerate() {
        if n > 0 {
            saida.push('\n');
        }
        if linha.trim_start().starts_with("```") {
            bloco = !bloco;
        }
        if bloco || linha.trim_start().starts_with("```") {
            saida.push_str(linha);
            continue;
        }
        for (k, palavra) in linha.split(' ').enumerate() {
            if k > 0 {
                saida.push(' ');
            }
            saida.push_str(&ligar_palavra(palavra, raiz));
        }
    }
    saida
}

fn ligar_palavra(palavra: &str, raiz: &Path) -> String {
    const VOLTA: &[char] = &['(', ')', '[', ']', '{', '}', ',', ';', ':', '!', '?', '"', '\'', '*', '.'];
    if palavra.contains("](") || palavra.contains("://") {
        return palavra.to_string();
    }
    // O ponto só sai do fim: no começo ele é de `./src`.
    let miolo = palavra
        .trim_start_matches(|c| c != '.' && VOLTA.contains(&c))
        .trim_end_matches(VOLTA);
    let caminho_linha = miolo.trim_matches('`');
    let (caminho, linha) = match caminho_linha.split_once(':') {
        Some((c, resto)) => (c, resto.split(':').next()),
        None => (caminho_linha, None),
    };
    let linha_ok = linha.is_none_or(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_digit()));
    let parece = caminho.contains('.') || caminho.contains('/');
    if miolo.is_empty() || !linha_ok || !parece || !raiz.join(caminho).is_file() {
        return palavra.to_string();
    }
    let ini = palavra.find(miolo).unwrap_or(0);
    let destino = match linha {
        Some(l) => format!("{caminho}:{l}"),
        None => caminho.to_string(),
    };
    format!(
        "{}[{caminho_linha}](arquivo:{destino}){}",
        &palavra[..ini],
        &palavra[ini + miolo.len()..]
    )
}

/// O texto sem os links de arquivo postos por [`ligar_caminhos`], para copiar.
fn sem_links(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(i) = resto.find("](arquivo:") {
        let Some(fim) = resto[i..].find(')') else { break };
        let ini = resto[..i].rfind('[').unwrap_or(i);
        saida.push_str(&resto[..ini]);
        saida.push_str(&resto[ini + 1..i]);
        resto = &resto[i + fim + 1..];
    }
    saida.push_str(resto);
    saida
}

/// Um traço de 2 px embaixo do filho, da largura dele: a aba ativa. Num `stack`, o traço
/// enchia a barra toda, e a primeira aba escondia as outras e o botão Rodar.
struct Sublinhado<'a> {
    filho: Element<'a, Msg>,
    cor: Color,
}

impl pelicano::Widget<Msg> for Sublinhado<'_> {
    fn size(&self) -> (Length, Length) {
        self.filho.widget().size()
    }
    fn layout(&self, cx: &mut pelicano::LayoutCx, l: pelicano::Limits) -> pelicano::Node {
        let n = cx.child(0, &self.filho, l);
        pelicano::Node {
            rect: n.rect,
            key: cx.key(),
            children: vec![n],
            ..Default::default()
        }
    }
    fn draw(&self, n: &pelicano::Node, cx: &mut pelicano::DrawCx) {
        self.filho.widget().draw(&n.children[0], cx);
        let r = n.rect;
        let traco = pelicano::Rect {
            y: r.y + r.height - 2.0,
            height: 2.0,
            ..r
        };
        cx.renderer
            .fill_quad(traco, Some(&self.cor.into()), Border::default(), None);
    }
    fn event(
        &self,
        n: &pelicano::Node,
        ev: &Event,
        cx: &mut pelicano::EventCx<Msg>,
    ) -> pelicano::Captured {
        self.filho.widget().event(&n.children[0], ev, cx)
    }
    fn opaque_at(&self, n: &pelicano::Node, p: Point) -> bool {
        self.filho.widget().opaque_at(&n.children[0], p)
    }
}

/// Avisa o clique (com o ponto, na janela) sem tomar o evento: o filho continua recebendo.
/// O editor pega o clique esquerdo antes de uma `mouse_area`; assim o app sabe qual lado foi clicado.
struct AoClicar<'a> {
    filho: Element<'a, Msg>,
    msg: Box<dyn Fn(Point) -> Msg + 'a>,
}

fn ao_clicar<'a>(filho: impl Into<Element<'a, Msg>>, msg: impl Fn(Point) -> Msg + 'a) -> Element<'a, Msg> {
    Element::new(AoClicar {
        filho: filho.into(),
        msg: Box::new(msg),
    })
}

impl pelicano::Widget<Msg> for AoClicar<'_> {
    fn size(&self) -> (Length, Length) {
        self.filho.widget().size()
    }
    fn layout(&self, cx: &mut pelicano::LayoutCx, l: pelicano::Limits) -> pelicano::Node {
        let n = cx.child(0, &self.filho, l);
        pelicano::Node {
            rect: n.rect,
            key: cx.key(),
            children: vec![n],
            ..Default::default()
        }
    }
    fn draw(&self, n: &pelicano::Node, cx: &mut pelicano::DrawCx) {
        self.filho.widget().draw(&n.children[0], cx);
    }
    fn event(
        &self,
        n: &pelicano::Node,
        ev: &Event,
        cx: &mut pelicano::EventCx<Msg>,
    ) -> pelicano::Captured {
        // Os dois botões: o menu de uma aba do outro lado também precisa do foco antes.
        if let (Event::MousePressed(_), Some(p)) = (ev, cx.cursor) {
            if n.rect.contains(p) {
                cx.publish((self.msg)(p));
            }
        }
        self.filho.widget().event(&n.children[0], ev, cx)
    }
    fn opaque_at(&self, n: &pelicano::Node, p: Point) -> bool {
        self.filho.widget().opaque_at(&n.children[0], p)
    }
}

/// Pergunta que espera resposta num diálogo.
#[derive(Debug, Clone, PartialEq)]
enum Confirmar {
    /// Aba com mudança não salva.
    FecharAba(usize),
    /// Fechar a janela com abas não salvas.
    Sair,
    Excluir(PathBuf),
}

/// Diálogo que pede um nome (explorador).
#[derive(Debug, Clone, PartialEq)]
enum PedirNome {
    NovoArquivo(PathBuf),
    NovaPasta(PathBuf),
    Renomear(PathBuf),
}

/// Onde o menu do botão direito foi aberto.
#[derive(Debug, Clone, PartialEq)]
enum Alvo {
    Editor,
    /// Item da árvore: caminho e se é pasta.
    Arvore(PathBuf, bool),
    Aba(usize),
    /// A seta do botão Commit.
    Commit,
}

/// Barra no topo do editor: localizar (e substituir) ou ir para a linha.
#[derive(Debug, Clone, Default)]
struct Barra {
    linha: bool,
    substituir: bool,
    termo: String,
    novo: String,
    maiusculas: bool,
    /// Não achou nada na última busca.
    nada: bool,
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
    /// Arquivo ▸ Novo arquivo…: pergunta nome e pasta, cria vazio e abre.
    NovoArquivo,
    NovoArquivoEscolhido(Option<PathBuf>),
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
    /// A beirada da janela sem moldura do sistema: redimensionar (cromo::moldura_redimensionavel).
    Puxar(pelicano::ResizeDirection),
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
    /// Abre (`Some`, onde clicou e em quê) ou fecha o menu do botão direito.
    MenuContexto(Option<(Point, Alvo)>),
    /// Resposta do diálogo de confirmação: `Some(true)` salva, `Some(false)` descarta, `None` cancela.
    Confirmado(Option<bool>),
    PedirNome(PedirNome),
    Nome(String),
    NomeOk,
    FecharDialogo,
    CopiarCaminho(PathBuf, bool),
    AbrirTerminalEm(PathBuf),
    Excluir(PathBuf),
    /// Fecha as abas sem mudança da lista; as com mudança ficam.
    FecharAbas(Vec<usize>),
    /// Abre a barra do editor: (ir para linha, com substituir).
    AbrirBarra(bool, bool),
    TermoBarra(String),
    NovoBarra(String),
    Maiusculas,
    Proxima,
    TrocarUma,
    TrocarTodas,
    FecharBarra,
    /// Aba em conflito com o disco: ver a diferença, ficar com a minha, usar a do disco.
    VerConflito,
    ManterMinha,
    UsarDisco,
    Previa,
    AbrirNoNavegador,
    CommitEPush,
    /// Commit da pasta inteira, mesmo com algo preparado.
    CommitTudo,
    /// Prepara (`true`) ou tira da preparação um arquivo, ou todos (`None`).
    Preparar(Option<PathBuf>, bool),
    Preparado(Result<(), String>),
    /// Enter no editor: quebra a linha mantendo o recuo.
    NovaLinha,
    /// O texto do editor mudou: recalcula as sugestões do autocompletar.
    Editou,
    AceitarSugestao,
    MoverSugestao(isize),
    FecharSugestoes,
    /// Mostra pontos nos espaços e setas nos tabs.
    AlternarInvisiveis,
    /// Roda o `cargo check` e enche a lista de Problemas.
    Checar,
    Checado(PathBuf, Result<Vec<problemas::Problema>, String>),
    /// Painel de baixo: `true` mostra Problemas, `false` o terminal.
    VerProblemas(bool),
    CorrigirComAgente,
    AlternarSalvarSozinho,
    SalvarSozinho,
    /// Diferença em linha (`true`) ou lado a lado.
    DiffEmLinha(bool),
    /// Divide o editor (Ctrl+\): o arquivo atual abre também do outro lado.
    Dividir,
    /// Volta a um lado só; as abas da direita passam para a esquerda.
    Juntar,
    /// O teclado passa para esse lado (0 = esquerda, 1 = direita).
    FocarGrupo(usize),
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
    /// Entre os dois lados do editor dividido.
    Editor,
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
    /// Menu do botão direito aberto nesse ponto.
    contexto: Option<(Point, Alvo)>,
    confirmar: Option<Confirmar>,
    /// Diálogo de nome aberto e o texto digitado.
    pedir_nome: Option<(PedirNome, String)>,
    barra: Option<Barra>,
    /// Faz o push quando o commit em andamento terminar.
    push_depois: bool,
    problemas: Vec<problemas::Problema>,
    /// Autocompletar: as palavras, a escolhida e onde foram pedidas (aba, linha, coluna).
    sugestoes: Vec<String>,
    sugestao_sel: usize,
    sugestao_em: (usize, usize, usize),
    /// Editor dividido em dois lados.
    dividido: bool,
    /// Lado com o teclado; `ativa` é a aba dele e `outra_ativa` a do outro lado.
    foco: usize,
    outra_ativa: usize,
    /// Parte da largura que fica com o lado esquerdo.
    fracao_divisao: f32,
    checando: bool,
    /// O painel de baixo mostra Problemas em vez do terminal.
    vendo_problemas: bool,
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
            Event::CloseRequested => Some(Msg::Fechar),
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
                "f" if ctrl => Some(Msg::AbrirBarra(false, false)),
                "h" if ctrl => Some(Msg::AbrirBarra(false, true)),
                "g" if ctrl => Some(Msg::AbrirBarra(true, false)),
                "\\" if ctrl => Some(Msg::Dividir),
                "o" | "O" if ctrl && shift => Some(Msg::EscolherArquivo),
                "n" if ctrl => Some(Msg::NovoArquivo),
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
        // "Abrir com" (Windows e .desktop) passa um arquivo: abre a pasta dele e o arquivo.
        let arquivo_pedido = pedida.clone().filter(|p| p.is_file());
        let pedida = match &arquivo_pedido {
            Some(a) => a.parent().map(PathBuf::from),
            None => pedida,
        };
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
            confirmar: None,
            pedir_nome: None,
            barra: None,
            push_depois: false,
            problemas: Vec::new(),
            sugestoes: Vec::new(),
            sugestao_sel: 0,
            sugestao_em: (0, 0, 0),
            dividido: false,
            foco: 0,
            outra_ativa: 0,
            fracao_divisao: 0.5,
            checando: false,
            vendo_problemas: false,
        };
        let mut code = code;
        if !code.inicio {
            if let Err(e) = code.terminais.novo(&code.arvore.raiz.clone(), None) {
                code.aviso = e;
            }
        }
        if let Some(a) = arquivo_pedido {
            let _ = code.update(Msg::AbrirArquivo(a));
        }
        // Modo de captura: FEATHERCODE_CAPTURAR=foto.png fotografa a janela e fecha;
        // FEATHERCODE_CAPTURAR_ARQUIVO abre um arquivo e FEATHERCODE_CAPTURAR_EXEMPLO=1 põe uma conversa de exemplo.
        if let Some(arquivo) = std::env::var_os("FEATHERCODE_CAPTURAR_ARQUIVO") {
            let _ = code.update(Msg::AbrirArquivo(code.arvore.raiz.join(arquivo)));
        }
        // FEATHERCODE_CAPTURAR_DIVIDIR=<caminho> divide o editor e abre esse arquivo à direita.
        if let Some(arquivo) = std::env::var_os("FEATHERCODE_CAPTURAR_DIVIDIR") {
            let _ = code.update(Msg::Dividir);
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
        // FEATHERCODE_CAPTURAR_BARRA=termo abre localizar e substituir com o termo.
        if let Ok(termo) = std::env::var("FEATHERCODE_CAPTURAR_BARRA") {
            let _ = code.update(Msg::AbrirBarra(false, true));
            let _ = code.update(Msg::TermoBarra(termo));
        }
        // FEATHERCODE_CAPTURAR_PROBLEMAS=1 mostra a aba Problemas com dois exemplos no arquivo aberto.
        if let (Some(_), Some(aba)) = (std::env::var_os("FEATHERCODE_CAPTURAR_PROBLEMAS"), code.abas.first()) {
            let exemplo = |linha, erro, mensagem: &str| problemas::Problema {
                caminho: aba.caminho.clone(),
                linha,
                coluna: 5,
                fim: Some(12),
                erro,
                mensagem: mensagem.into(),
            };
            code.problemas = vec![
                exemplo(2, true, "mismatched types: expected `usize`, found `String`"),
                exemplo(6, false, "function `velho` is never used"),
            ];
            code.vendo_problemas = true;
        }
        // FEATHERCODE_CAPTURAR_PREVIA=1 mostra o Markdown aberto formatado.
        if std::env::var_os("FEATHERCODE_CAPTURAR_PREVIA").is_some() {
            let _ = code.update(Msg::Previa);
        }
        // FEATHERCODE_CAPTURAR_DIALOGO=1 mostra a pergunta de fechar a aba aberta.
        if std::env::var_os("FEATHERCODE_CAPTURAR_DIALOGO").is_some() && !code.abas.is_empty() {
            code.confirmar = Some(Confirmar::FecharAba(0));
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
        // Depois do Git e do diff: qualquer mensagem fecha os menus.
        // FEATHERCODE_CAPTURAR_MENU_COMMIT=1 deixa aberto o menu da seta do Commit.
        if std::env::var_os("FEATHERCODE_CAPTURAR_MENU_COMMIT").is_some() {
            code.contexto = Some((Point::new(paineis::LARGURA_TRILHO + 150.0, 140.0), Alvo::Commit));
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
        if self.config.salvar_sozinho && self.abas.iter().any(Aba::sujo) {
            subs.push(
                Subscription::every(std::time::Duration::from_secs(2)).map(|_| Msg::SalvarSozinho),
            );
        }
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

    /// Lê do disco as abas que outro (o agente) alterou: sem mudança local, recarrega;
    /// com mudança local, marca o conflito e a pessoa escolhe na faixa do editor.
    fn recarregar_abas(&mut self) {
        for aba in self.abas.iter_mut() {
            let Ok(t) = std::fs::read_to_string(&aba.caminho) else {
                continue;
            };
            if marca_texto(&t) == aba.disco {
                continue;
            }
            if !aba.sujo() {
                aba.recarregar(&t);
            } else if t.trim_end() != aba.conteudo.text().trim_end() {
                aba.conflito = true;
            } else {
                // O disco ficou igual ao que está na aba: não há o que escolher.
                aba.disco = marca_texto(&t);
            }
        }
    }

    /// Salva as abas com mudança; devolve os erros. `pular_conflito`: o salvar sozinho não
    /// passa por cima do que o agente gravou.
    fn salvar_abas(&mut self, pular_conflito: bool) -> Vec<String> {
        self.abas
            .iter_mut()
            .filter(|a| a.sujo() && !(pular_conflito && a.conflito))
            .filter_map(|a| a.salvar().err().map(|e| format!("{}: {e}", a.caminho.display())))
            .collect()
    }

    /// Desce o chat até a última mensagem, como o `anchor_bottom` fazia.
    fn rolar_chat() -> Task<Msg> {
        Task::snap_to("chat", Point::new(0.0, 1.0))
    }

    fn sinal(&mut self, s: Sinal) {
        match s {
            Sinal::Item(Item::Texto(t)) => {
                let t = ligar_caminhos(&t, &self.arvore.raiz);
                self.chat.itens.push(Item::Texto(t))
            }
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
        self.dividido = false;
        self.foco = 0;
        self.outra_ativa = 0;
        self.inicio = false;
        self.aviso.clear();
        self.achados = None;
        self.filtro.clear();
        self.todos_arquivos.clear();
        self.git = None;
        self.problemas.clear();
        self.vendo_problemas = false;
        self.outros_chats.clear();
        self.chat = Chat::novo(self.proximo_chat, self.chat.provedor, self.chat.modo);
        self.proximo_chat += 1;
        Task::none()
    }

    /// O bit do lado com o foco em `Aba::grupos`.
    fn bit(&self) -> u8 {
        1 << self.foco
    }

    /// A aba só está no lado com o foco: fechar tira de vez (e pergunta se tem mudança).
    fn so_aqui(&self, i: usize) -> bool {
        self.abas[i].grupos & !self.bit() == 0
    }

    /// Fecha a aba no lado com o foco; se ela não está no outro lado, sai de vez.
    fn fechar_aba(&mut self, i: usize) {
        if self.so_aqui(i) {
            self.remover_aba(i);
        } else {
            self.abas[i].grupos &= !self.bit();
            self.arrumar_grupos(i);
        }
    }

    /// Tira a aba dos dois lados.
    fn remover_aba(&mut self, i: usize) {
        self.abas.remove(i);
        for a in [&mut self.ativa, &mut self.outra_ativa] {
            if *a > i {
                *a -= 1;
            }
        }
        self.arrumar_grupos(i);
    }

    /// Depois de fechar perto de `i`: cada lado fica com uma aba ativa dele; lado vazio junta.
    fn arrumar_grupos(&mut self, i: usize) {
        let perto = |abas: &[Aba], g: usize, desde: usize| {
            let lista: Vec<usize> = (0..abas.len()).filter(|&j| abas[j].grupos & (1 << g) != 0).collect();
            lista.iter().copied().find(|&j| j >= desde).or(lista.last().copied())
        };
        let dela = |abas: &[Aba], j: usize, g: usize| abas.get(j).is_some_and(|a| a.grupos & (1 << g) != 0);
        if self.dividido {
            let aqui = perto(&self.abas, self.foco, i);
            let la = perto(&self.abas, 1 - self.foco, self.outra_ativa);
            if aqui.is_none() || la.is_none() {
                if aqui.is_none() {
                    self.ativa = self.outra_ativa;
                }
                self.juntar();
                return;
            }
            if !dela(&self.abas, self.outra_ativa, 1 - self.foco) {
                self.outra_ativa = la.unwrap_or(0);
            }
        }
        if !dela(&self.abas, self.ativa, self.foco) {
            self.ativa = perto(&self.abas, self.foco, i).unwrap_or(0);
        }
    }

    /// Volta a um lado só: as abas dos dois lados ficam todas à esquerda.
    fn juntar(&mut self) {
        for a in &mut self.abas {
            a.grupos = 1;
        }
        self.dividido = false;
        self.foco = 0;
        self.ativa = self.ativa.min(self.abas.len().saturating_sub(1));
    }

    fn focar_editor(&self) -> Task<Msg> {
        Task::focus(("editor", self.foco))
    }

    /// Exclui de vez (não há lixeira por aqui) e fecha as abas que estavam lá dentro.
    fn excluir(&mut self, p: &Path) {
        let r = if p.is_dir() {
            std::fs::remove_dir_all(p)
        } else {
            std::fs::remove_file(p)
        };
        match r {
            Ok(()) => {
                for i in (0..self.abas.len()).rev() {
                    if self.abas[i].caminho.starts_with(p) {
                        self.remover_aba(i);
                    }
                }
                let _ = self.update(Msg::AtualizarArvore);
            }
            Err(e) => self.aviso = format!("Não deu para excluir {}: {e}", self.relativo(p).display()),
        }
    }

    /// Cria ou renomeia pelo diálogo de nome; devolve o arquivo novo, para abrir.
    fn aplicar_nome(&mut self, acao: &PedirNome, nome: &str) -> Result<Option<PathBuf>, String> {
        let relativo = Path::new(nome);
        if nome.is_empty()
            || relativo.is_absolute()
            || relativo.components().any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err("Nome inválido.".into());
        }
        let destino = match acao {
            PedirNome::NovoArquivo(pasta) | PedirNome::NovaPasta(pasta) => pasta.join(relativo),
            PedirNome::Renomear(p) => p.with_file_name(relativo),
        };
        if destino.exists() {
            return Err(format!("Já existe {}.", self.relativo(&destino).display()));
        }
        let erro = |e: std::io::Error| format!("Não deu para criar {nome}: {e}");
        match acao {
            PedirNome::NovoArquivo(_) => {
                if let Some(pai) = destino.parent() {
                    std::fs::create_dir_all(pai).map_err(erro)?;
                }
                std::fs::write(&destino, "").map_err(erro)?;
                self.arvore.mostrar(&destino);
                Ok(Some(destino))
            }
            PedirNome::NovaPasta(_) => {
                std::fs::create_dir_all(&destino).map_err(erro)?;
                self.arvore.mostrar(&destino);
                Ok(None)
            }
            PedirNome::Renomear(velho) => {
                std::fs::rename(velho, &destino)
                    .map_err(|e| format!("Não deu para renomear: {e}"))?;
                for aba in &mut self.abas {
                    if let Ok(resto) = aba.caminho.strip_prefix(velho) {
                        aba.caminho = destino.join(resto);
                    }
                }
                Ok(None)
            }
        }
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
                | Msg::Diff(_)
                | Msg::ArquivosCommit(_)
                | Msg::TituloCommit(..)
                | Msg::Commitado(_)
                | Msg::Preparado(_)
                | Msg::Checado(..)
                | Msg::SalvarSozinho
                | Msg::Ini(inicio::Ini::ClaudeVersao(_) | inicio::Ini::OutrosAgentes(_))
        ) {
            self.menu = None;
            self.contexto = None;
        }
        match msg {
            Msg::Menu(m) => self.menu = m,
            Msg::Redimensionar(d) => self.arrastando = d,
            Msg::MenuContexto(p) => self.contexto = p,
            Msg::Confirmado(resposta) => {
                let Some(pergunta) = self.confirmar.take() else {
                    return Task::none();
                };
                let Some(salvar) = resposta else {
                    return Task::none();
                };
                match pergunta {
                    Confirmar::FecharAba(i) if i < self.abas.len() => {
                        if salvar {
                            if let Err(e) = self.abas[i].salvar() {
                                self.aviso = format!("Não deu para salvar: {e}");
                                return Task::none();
                            }
                        }
                        self.fechar_aba(i);
                    }
                    Confirmar::FecharAba(_) => {}
                    Confirmar::Sair => {
                        if salvar {
                            let erros = self.salvar_abas(false);
                            if !erros.is_empty() {
                                self.aviso = format!("Não deu para salvar: {}", erros.join("; "));
                                return Task::none();
                            }
                        }
                        return Task::window(WindowCmd::Close);
                    }
                    Confirmar::Excluir(p) => self.excluir(&p),
                }
            }
            Msg::Excluir(p) => self.confirmar = Some(Confirmar::Excluir(p)),
            Msg::PedirNome(acao) => {
                let texto = match &acao {
                    PedirNome::Renomear(p) => p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                self.pedir_nome = Some((acao, texto));
                return Task::focus("nome");
            }
            Msg::Nome(t) => {
                if let Some((_, texto)) = &mut self.pedir_nome {
                    *texto = t;
                }
            }
            Msg::NomeOk => {
                let Some((acao, nome)) = self.pedir_nome.clone() else {
                    return Task::none();
                };
                match self.aplicar_nome(&acao, nome.trim()) {
                    Ok(abrir) => {
                        self.pedir_nome = None;
                        let _ = self.update(Msg::AtualizarArvore);
                        if let Some(p) = abrir {
                            return self.update(Msg::AbrirArquivo(p));
                        }
                    }
                    Err(e) => self.aviso = e,
                }
            }
            Msg::FecharDialogo => {
                self.confirmar = None;
                self.pedir_nome = None;
            }
            Msg::CopiarCaminho(p, relativo) => {
                let p = if relativo { self.relativo(&p) } else { &p };
                return Task::write_clipboard(p.display().to_string());
            }
            Msg::AbrirTerminalEm(pasta) => {
                self.terminal_aberto = true;
                match self.terminais.novo(&pasta, None) {
                    Ok(_) => return self.terminais.focar(),
                    Err(e) => self.aviso = e,
                }
            }
            Msg::AbrirBarra(linha, substituir) => {
                let Some(aba) = self.aba() else {
                    return Task::none();
                };
                let selecao = aba
                    .conteudo
                    .selection()
                    .filter(|s| !s.is_empty() && !s.contains('\n'));
                let mut b = self.barra.take().unwrap_or_default();
                if linha != b.linha {
                    b.termo.clear();
                }
                if let (false, Some(sel)) = (linha, selecao) {
                    b.termo = sel;
                }
                b.linha = linha;
                b.substituir = substituir && !linha;
                b.nada = false;
                self.barra = Some(b);
                return Task::focus("barra");
            }
            Msg::TermoBarra(t) => {
                if let Some(b) = &mut self.barra {
                    b.termo = t;
                    b.nada = false;
                }
            }
            Msg::NovoBarra(t) => {
                if let Some(b) = &mut self.barra {
                    b.novo = t;
                }
            }
            Msg::Maiusculas => {
                if let Some(b) = &mut self.barra {
                    b.maiusculas = !b.maiusculas;
                }
            }
            Msg::Proxima | Msg::TrocarUma | Msg::TrocarTodas => {
                let (Some(b), Some(aba)) = (self.barra.as_mut(), self.abas.get(self.ativa)) else {
                    return Task::none();
                };
                if b.linha {
                    match b.termo.trim().parse::<usize>() {
                        Ok(n) if n > 0 => {
                            aba.conteudo.go_to_line(n);
                            self.barra = None;
                            return self.focar_editor();
                        }
                        _ => b.nada = true,
                    }
                    return Task::none();
                }
                let c = &aba.conteudo;
                b.nada = match msg {
                    Msg::TrocarUma => !c.replace_next(&b.termo, &b.novo, b.maiusculas),
                    Msg::TrocarTodas => c.replace_all(&b.termo, &b.novo, b.maiusculas) == 0,
                    _ => !c.find_next(&b.termo, b.maiusculas),
                };
            }
            Msg::FecharBarra => {
                self.barra = None;
                return self.focar_editor();
            }
            Msg::VerConflito => {
                if let Some(aba) = self.aba() {
                    return Task::perform(
                        paineis::diff_textos(aba.caminho.clone(), aba.conteudo.text()),
                        Msg::Diff,
                    );
                }
            }
            Msg::ManterMinha | Msg::UsarDisco => {
                let Some(aba) = self.abas.get_mut(self.ativa) else {
                    return Task::none();
                };
                match std::fs::read_to_string(&aba.caminho) {
                    Ok(t) if matches!(msg, Msg::UsarDisco) => aba.recarregar(&t),
                    // A do disco fica para trás: o próximo Salvar grava a minha por cima.
                    Ok(t) => {
                        aba.disco = marca_texto(&t);
                        aba.conflito = false;
                    }
                    Err(e) => self.aviso = format!("Não deu para ler o arquivo: {e}"),
                }
                self.diff = None;
            }
            Msg::Previa => {
                if let Some(aba) = self.abas.get_mut(self.ativa) {
                    aba.previa = !aba.previa && aba.markdown();
                }
            }
            Msg::AbrirNoNavegador => {
                if let Some(aba) = self.aba() {
                    let caminho = aba.caminho.display().to_string();
                    return self.update(Msg::Link(caminho));
                }
            }
            Msg::Checar => {
                if self.checando || !problemas::disponivel(&self.arvore.raiz) {
                    return Task::none();
                }
                self.checando = true;
                let raiz = self.arvore.raiz.clone();
                return Task::perform(problemas::checar(raiz.clone()), move |r| {
                    Msg::Checado(raiz.clone(), r)
                });
            }
            Msg::Checado(raiz, r) => {
                self.checando = false;
                // Resposta de outra pasta (a pessoa trocou durante o check) não vale.
                if raiz == self.arvore.raiz {
                    match r {
                        Ok(v) => self.problemas = v,
                        Err(e) => self.aviso = e,
                    }
                }
            }
            Msg::VerProblemas(ver) => {
                self.vendo_problemas = ver;
                self.terminal_aberto = true;
                if ver && self.problemas.is_empty() {
                    return self.update(Msg::Checar);
                }
            }
            Msg::CorrigirComAgente => {
                let lista: Vec<String> = self
                    .problemas
                    .iter()
                    .map(|p| {
                        let tipo = if p.erro { "erro" } else { "aviso" };
                        format!("- {} {tipo}: {}", p.onde(&self.arvore.raiz), p.mensagem)
                    })
                    .collect();
                if lista.is_empty() {
                    return Task::none();
                }
                self.chat.pedido = Content::with_text(&format!(
                    "Corrija estes problemas do cargo check:\n{}",
                    lista.join("\n")
                ));
                self.agente_aberto = true;
                return Task::focus("pedido");
            }
            Msg::CommitEPush => {
                self.push_depois = true;
                return self.update(Msg::Commit);
            }
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
                return Task::batch([Task::write_clipboard(sel), self.focar_editor()]);
            }
            Msg::Colar => return Task::read_clipboard(Msg::Colado),
            Msg::Colado(texto) => {
                if let (Some(aba), Some(t)) = (self.abas.get(self.ativa), texto) {
                    aba.conteudo.insert(&t);
                }
                return self.focar_editor();
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
                    Some(Divisa::Editor) => {
                        let x0 = paineis::LARGURA_TRILHO
                            + if self.lado.is_some() { self.largura_lado() + VAO } else { 0.0 };
                        let w = self.largura_editor().max(1.0);
                        self.fracao_divisao = ((p.x - x0) / w).clamp(0.2, 0.8);
                    }
                    Some(Divisa::Agente) => {
                        self.largura_agente = (w - p.x - VAO).min(w / 2.0).max(280.0)
                    }
                    Some(Divisa::Terminal) => {
                        let puxado = h - ALTURA_STATUS - VAO - p.y;
                        // Fechado, só abre depois de puxar um pouco: um clique na beirada não abre.
                        if !self.terminal_aberto && puxado > 40.0 {
                            self.terminal_aberto = true;
                            if self.terminais.ativo().is_none() {
                                if let Err(e) = self.terminais.novo(&self.arvore.raiz, None) {
                                    self.aviso = e;
                                }
                            }
                        }
                        self.altura_terminal = puxado.min(h - 260.0).max(80.0)
                    }
                    None => {}
                }
            }
            Msg::Configuracoes(secao) => self.configuracoes = secao,
            Msg::Escape => {
                if self.menu.is_some() || self.contexto.is_some() {
                    self.menu = None;
                    self.contexto = None;
                } else if self.confirmar.is_some() || self.pedir_nome.is_some() {
                    self.confirmar = None;
                    self.pedir_nome = None;
                } else if self.barra.is_some() {
                    return self.update(Msg::FecharBarra);
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
                    aba.conteudo.insert(&edicao::detectar_recuo(&aba.conteudo).unidade());
                } else {
                    edicao::recuar(&aba.conteudo, mais);
                }
            }
            Msg::NovaLinha => {
                let Some(aba) = self.abas.get(self.ativa) else {
                    return Task::none();
                };
                let c = &aba.conteudo;
                let (l, col) = c.cursor();
                let linha = c.line(l).unwrap_or_default();
                c.insert(&edicao::quebra_com_recuo(&linha, col, edicao::detectar_recuo(c)));
            }
            Msg::Editou => {
                let Some(aba) = self.abas.get(self.ativa) else {
                    return Task::none();
                };
                let (l, col) = aba.conteudo.cursor();
                let linha = aba.conteudo.line(l).unwrap_or_default();
                let prefixo = completar::prefixo(&linha, col);
                self.sugestoes = completar::sugestoes(&aba.conteudo, &aba.caminho, &prefixo);
                self.sugestao_sel = 0;
                self.sugestao_em = (self.ativa, l, col);
            }
            Msg::AceitarSugestao => {
                let escolhida = self.sugestoes.get(self.sugestao_sel).cloned();
                self.sugestoes.clear();
                if let (Some(palavra), Some(aba)) = (escolhida, self.abas.get(self.ativa)) {
                    let (l, col) = aba.conteudo.cursor();
                    let linha = aba.conteudo.line(l).unwrap_or_default();
                    let prefixo = completar::prefixo(&linha, col);
                    if let Some(resto) = palavra.strip_prefix(prefixo.as_str()) {
                        aba.conteudo.insert(resto);
                    }
                }
            }
            Msg::MoverSugestao(passo) => {
                let n = self.sugestoes.len() as isize;
                if n > 0 {
                    self.sugestao_sel = (self.sugestao_sel as isize + passo).rem_euclid(n) as usize;
                }
            }
            Msg::FecharSugestoes => self.sugestoes.clear(),
            Msg::AlternarInvisiveis => {
                self.config.mostrar_invisiveis = !self.config.mostrar_invisiveis;
                self.config.gravar();
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
                return Task::batch([abrir, self.focar_editor()]);
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
            Msg::Commit | Msg::CommitTudo => {
                let msg_commit = self.mensagem_commit.trim().to_string();
                // Com algo preparado, o commit leva só o preparado (como o VS Code).
                let tudo = matches!(msg, Msg::CommitTudo)
                    || !matches!(&self.git, Some(Ok(g)) if !g.preparadas.is_empty());
                if msg_commit.is_empty() {
                    self.push_depois = false;
                    self.aviso = "Escreva a mensagem do commit.".into();
                } else {
                    return Task::perform(
                        paineis::commit(self.arvore.raiz.clone(), msg_commit, tudo),
                        Msg::Commitado,
                    );
                }
            }
            Msg::Preparar(caminho, sim) => {
                return Task::perform(
                    paineis::preparar(self.arvore.raiz.clone(), caminho, sim),
                    Msg::Preparado,
                );
            }
            Msg::Preparado(r) => {
                if let Err(e) = r {
                    self.aviso = e;
                }
                return self.update(Msg::GitAtualizar);
            }
            Msg::Commitado(Ok(())) => {
                self.geracao_titulo += 1;
                self.titulo_pendente = None;
                self.mensagem_commit.clear();
                let atualizar = self.update(Msg::GitAtualizar);
                if std::mem::take(&mut self.push_depois) {
                    return Task::batch([atualizar, self.update(Msg::GitRemoto("push"))]);
                }
                return atualizar;
            }
            Msg::Commitado(Err(e)) => {
                self.push_depois = false;
                self.aviso = e;
            }
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
                        // O primeiro filtro é o que vem marcado: editor de código abre qualquer texto.
                        .add_filter("Todos os arquivos", &["*"])
                        .add_filter("Rust", &["rs"])
                        .add_filter("Python", &["py"])
                        .add_filter("JavaScript e TypeScript", &["js", "jsx", "ts", "tsx", "mjs", "cjs"])
                        .add_filter("C, C++ e C#", &["c", "h", "cpp", "hpp", "cc", "cs"])
                        .add_filter("Go, Java e Kotlin", &["go", "java", "kt"])
                        .add_filter("Web", &["html", "htm", "css", "scss", "xml", "svg"])
                        .add_filter("Configuração", &["json", "toml", "yaml", "yml", "ini", "cfg", "env"])
                        .add_filter("Scripts", &["sh", "bash", "ps1", "bat", "lua", "rb", "php", "sql"])
                        .add_filter("Texto e Markdown", &["txt", "md", "log", "csv"])
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
            Msg::NovoArquivo => {
                // Começa na pasta do arquivo aberto (como o VS Code), senão na raiz do projeto.
                let pasta = self
                    .aba()
                    .and_then(|a| a.caminho.parent().map(Path::to_path_buf))
                    .unwrap_or_else(|| self.arvore.raiz.clone());
                return Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Novo arquivo")
                        .set_directory(pasta)
                        .set_file_name("sem_titulo.txt")
                        .save_file(),
                    |h| Msg::NovoArquivoEscolhido(h.map(|h| h.path().to_path_buf())),
                );
            }
            Msg::NovoArquivoEscolhido(Some(p)) => {
                // Se o nome já existe, só abre: nunca esvazia um arquivo que já tinha conteúdo.
                if !p.exists() {
                    if let Err(e) = std::fs::write(&p, "") {
                        self.aviso = format!("Não deu para criar {}: {e}", p.display());
                        return Task::none();
                    }
                }
                let abrir = if self.inicio {
                    p.parent()
                        .map(|pai| self.abrir_pasta(pai.to_path_buf()))
                        .unwrap_or_else(Task::none)
                } else {
                    Task::none()
                };
                return Task::batch([abrir, Task::done(Msg::AtualizarArvore), Task::done(Msg::AbrirArquivo(p))]);
            }
            Msg::NovoArquivoEscolhido(None) => {}
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
                    self.abas[i].grupos |= self.bit();
                    self.ativa = i;
                } else {
                    match std::fs::read_to_string(&p) {
                        Ok(t) => {
                            self.config.usar_arquivo(p.clone());
                            let mut aba = Aba::nova(p, &t);
                            aba.grupos = self.bit();
                            self.abas.push(aba);
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
            Msg::FecharAba(i) if self.abas[i].sujo() && self.so_aqui(i) => {
                self.confirmar = Some(Confirmar::FecharAba(i))
            }
            Msg::FecharAba(i) => self.fechar_aba(i),
            Msg::FecharAbas(mut lista) => {
                lista.sort_unstable();
                lista.retain(|&i| i < self.abas.len());
                let mut ficaram = 0;
                for i in lista.into_iter().rev() {
                    if self.abas[i].sujo() && self.so_aqui(i) {
                        ficaram += 1;
                    } else {
                        self.fechar_aba(i);
                    }
                }
                if ficaram > 0 {
                    self.aviso = format!("{ficaram} aba(s) com mudança não salva ficaram abertas.");
                }
            }
            Msg::SalvarTudo => {
                let erros = self.salvar_abas(false);
                if erros.is_empty() {
                    self.aviso.clear();
                    return self.update(Msg::Checar);
                }
                self.aviso = format!("Não deu para salvar: {}", erros.join("; "));
            }
            Msg::SalvarSozinho => {
                let erros = self.salvar_abas(true);
                if !erros.is_empty() {
                    self.aviso = format!("Não deu para salvar: {}", erros.join("; "));
                }
            }
            Msg::Dividir => {
                if self.abas.get(self.ativa).is_none() {
                    return Task::none();
                }
                // Já dividido: o arquivo vai também para o outro lado. O foco vai junto.
                let outro = if self.dividido { 1 - self.foco } else { 1 };
                self.abas[self.ativa].grupos |= 1 << outro;
                self.dividido = true;
                self.outra_ativa = self.ativa;
                self.foco = outro;
                self.diff = None;
                return self.focar_editor();
            }
            Msg::Juntar => {
                self.juntar();
                return self.focar_editor();
            }
            Msg::FocarGrupo(g) => {
                if self.dividido && g < 2 && g != self.foco {
                    std::mem::swap(&mut self.ativa, &mut self.outra_ativa);
                    self.foco = g;
                    self.diff = None;
                }
            }
            Msg::DiffEmLinha(sim) => {
                self.config.diff_em_linha = sim;
                self.config.gravar();
            }
            Msg::AlternarSalvarSozinho => {
                self.config.salvar_sozinho = !self.config.salvar_sozinho;
                self.config.gravar();
            }
            Msg::Salvar => {
                if let Some(aba) = self.abas.get_mut(self.ativa) {
                    match aba.salvar() {
                        Ok(()) => {
                            self.aviso.clear();
                            // Os erros aparecem sem rodar o projeto: o check só confere, não gera o programa.
                            return self.update(Msg::Checar);
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
            Msg::Link(url) if url.starts_with("arquivo:") => {
                let resto = &url["arquivo:".len()..];
                let (caminho, linha) = match resto.rsplit_once(':') {
                    Some((c, l)) if l.bytes().all(|b| b.is_ascii_digit()) => {
                        (c, l.parse::<usize>().unwrap_or(1))
                    }
                    _ => (resto, 1),
                };
                return self.update(Msg::IrPara(self.arvore.raiz.join(caminho), linha.saturating_sub(1)));
            }
            Msg::Link(url) => {
                if let Err(e) = sistema::abrir_link(&url) {
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
            Msg::Puxar(d) => return Task::window(WindowCmd::DragResize(d)),
            Msg::Maximizar => return Task::window(WindowCmd::ToggleMaximize),
            Msg::Fechar if self.abas.iter().any(Aba::sujo) => {
                self.confirmar = Some(Confirmar::Sair)
            }
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
        let janela = self.com_dialogo(janela);
        // Com as beiradas de redimensionar: sem a moldura do sistema, só mudava de tamanho maximizando.
        cromo::moldura_redimensionavel(janela, Msg::Puxar)
    }

    /// Diálogo da suíte por cima da janela: confirmar (fechar, sair, excluir) ou pedir um nome.
    fn com_dialogo<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        use feather_ui::dialogo::Dialogo;
        let nome = |p: &Path| self.relativo(p).display().to_string();
        let ajustar = |d: Dialogo<'a, Msg>| {
            d.ao_fechar(Msg::FecharDialogo)
                .topo(cromo::BARRA_ALTURA + 60.0)
                .altura_janela(self.tamanho.height)
                .largura_janela(self.tamanho.width)
        };
        if let Some(c) = &self.confirmar {
            let (titulo, frase, salvar, descartar) = match c {
                Confirmar::FecharAba(i) => (
                    "Salvar as mudanças?",
                    format!(
                        "{} tem mudanças que não foram salvas.",
                        self.abas.get(*i).map(|a| nome(&a.caminho)).unwrap_or_default()
                    ),
                    Some("Salvar"),
                    "Não salvar",
                ),
                Confirmar::Sair => {
                    let sujas: Vec<String> =
                        self.abas.iter().filter(|a| a.sujo()).map(|a| nome(&a.caminho)).collect();
                    (
                        "Sair sem salvar?",
                        format!("Estas abas têm mudanças não salvas: {}.", sujas.join(", ")),
                        Some("Salvar e sair"),
                        "Sair sem salvar",
                    )
                }
                Confirmar::Excluir(p) => (
                    "Excluir?",
                    format!(
                        "{} vai ser apagado de vez. Não vai para a lixeira.",
                        nome(p)
                    ),
                    None,
                    "Excluir",
                ),
            };
            let d = ajustar(Dialogo::new(titulo, text(frase).size(13)).icone("alert-triangle"))
                .secundario("Cancelar", Some(Msg::Confirmado(None)));
            let d = match salvar {
                Some(s) => d
                    .secundario(descartar, Some(Msg::Confirmado(Some(false))))
                    .primario(s, Some(Msg::Confirmado(Some(true)))),
                None => d.primario(descartar, Some(Msg::Confirmado(Some(true)))),
            };
            return d.sobre(base);
        }
        if let Some((acao, texto)) = &self.pedir_nome {
            let (titulo, onde, botao) = match acao {
                PedirNome::NovoArquivo(p) => ("Novo arquivo", p, "Criar"),
                PedirNome::NovaPasta(p) => ("Nova pasta", p, "Criar"),
                PedirNome::Renomear(p) => ("Renomear", p, "Renomear"),
            };
            let onde = match acao {
                PedirNome::Renomear(_) => format!("Novo nome para {}", nome(onde)),
                _ if *onde == self.arvore.raiz => "Na pasta do projeto".into(),
                _ => format!("Em {}", nome(onde)),
            };
            let corpo = column![
                text(onde).size(12).color(COR_TEXTO_MUTED()),
                text_input("nome", texto.as_str())
                    .id("nome")
                    .on_input(Msg::Nome)
                    .on_submit(Msg::NomeOk)
                    .size(13)
                    .padding([6, 8])
                    .style(campo(COR_PAINEL())),
            ]
            .spacing(8);
            return ajustar(Dialogo::new(titulo, corpo))
                .secundario("Cancelar", Some(Msg::FecharDialogo))
                .primario(botao, Some(Msg::NomeOk))
                .sobre(base);
        }
        base
    }

    fn area_de_trabalho<'a>(&'a self, barra: Element<'a, Msg>) -> Element<'a, Msg> {
        // Cada painel é um cartão de cantos arredondados; o fundo da janela aparece nos vãos.
        let editor: Element<Msg> = if self.dividido {
            // Cada lado é um cartão; o clique em qualquer ponto de um lado leva o teclado para ele.
            let lado = |g: usize, parte: f32| {
                container(ao_clicar(
                    cartao(self.grupo_view(g), COR_PAINEL(), Length::FILL),
                    move |_| Msg::FocarGrupo(g),
                ))
                .width(Length::Fill((parte * 1000.0).max(1.0) as u16))
                .height(Length::FILL)
            };
            row![
                lado(0, self.fracao_divisao),
                alca(Divisa::Editor),
                lado(1, 1.0 - self.fracao_divisao)
            ]
            .height(Length::FILL)
            .into()
        } else {
            cartao(self.grupo_view(0), COR_PAINEL(), Length::FILL)
        };
        // Os vãos entre os cartões são as alças de arrastar.
        let mut centro = column![editor]
            .width(Length::FILL)
            .height(Length::FILL);
        if self.terminal_aberto {
            centro = centro
                .push(alca(Divisa::Terminal))
                .push(cartao(self.terminal_view(), cor_terminal(), Length::Shrink));
        } else {
            // Fechado, a beirada de baixo continua arrastável: puxar para cima abre o terminal.
            centro = centro.push(alca(Divisa::Terminal));
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
            let icone = paineis::icone_arquivo(&l.caminho, l.pasta.then_some(l.aberta));
            let alvo = Alvo::Arvore(l.caminho.clone(), l.pasta);
            let ativo = aberto == Some(l.caminho.as_path());
            let msg = if l.pasta {
                Msg::Alternar(l.caminho)
            } else {
                Msg::AbrirArquivo(l.caminho)
            };
            mouse_area(
                button(
                    row![
                        Space::new(l.nivel as f32 * 14.0, 0.0),
                        seta,
                        icone,
                        text(nome).size(13).no_wrap()
                    ]
                    .spacing(4)
                    .width(Length::FILL)
                    .align(Align::Center),
                )
                .on_press(msg)
                .width(Length::FILL)
                .padding([3, 12])
                .style(move |_, s| item_lista(ativo, s, 0.0)),
            )
            .on_right_press_at(move |p| Msg::MenuContexto(Some((p, alvo.clone()))))
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
                            botao_icone(
                                "file-plus",
                                Msg::PedirNome(PedirNome::NovoArquivo(self.arvore.raiz.clone()))
                            ),
                            "Novo arquivo",
                            TooltipPosition::Bottom
                        )
                        .style(tema::estilo_dica),
                        tooltip(
                            botao_icone(
                                "folder",
                                Msg::PedirNome(PedirNome::NovaPasta(self.arvore.raiz.clone()))
                            ),
                            "Nova pasta",
                            TooltipPosition::Bottom
                        )
                        .style(tema::estilo_dica),
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

    /// A aba ativa do lado `g` (o com foco usa `ativa`; o outro, `outra_ativa`).
    fn ativa_do(&self, g: usize) -> usize {
        if g == self.foco {
            self.ativa
        } else {
            self.outra_ativa
        }
    }

    /// Um lado do editor: abas, faixas (só no lado com foco) e o editor ou a diferença.
    fn grupo_view(&self, g: usize) -> Element<'_, Msg> {
        let foco = g == self.foco;
        let mut c = column![self.abas_view(g)].height(Length::FILL);
        if foco {
            c = c.push(self.conflito_view());
        }
        let corpo = match &self.diff {
            Some(d) if foco => self.diff_view(d),
            _ => self.editor_view(g),
        };
        // A caixinha de localizar flutua por cima do editor, sem empurrar o texto.
        let corpo: Element<Msg> = match self.barra_view().filter(|_| foco && self.diff.is_none()) {
            Some(caixa) => stack![corpo, caixa]
                .width(Length::FILL)
                .height(Length::FILL)
                .into(),
            None => corpo,
        };
        c.push(corpo).into()
    }

    fn abas_view(&self, g: usize) -> Element<'_, Msg> {
        let ativa_aqui = self.ativa_do(g);
        // Com o editor dividido, só o lado com o teclado tem o traço.
        let traco = g == self.foco;
        let abas = self.abas.iter().enumerate().filter(|(_, a)| a.grupos & (1 << g) != 0).map(|(i, a)| {
            let ativa = i == ativa_aqui;
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
            let aba: Element<Msg> = if ativa && traco {
                Element::new(Sublinhado {
                    filho: aba.into(),
                    cor: COR_AZUL_ACENTO(),
                })
            } else {
                aba.into()
            };
            mouse_area(aba)
            .on_right_press_at(move |p| Msg::MenuContexto(Some((p, Alvo::Aba(i)))))
            .into()
        });
        let arquivo = self.aba().map(|a| a.caminho.as_path());
        let rodar: Element<Msg> = match execucao::comando_do_projeto(&self.arvore.raiz, arquivo) {
            Some(cmd) if g == 0 => {
                botao_rodar("Rodar", VERDE, Msg::RodarProjeto, format!("{cmd}  (F5)"))
            }
            _ => Space::new(0.0, 0.0).into(),
        };
        let dividir: Element<Msg> = match (self.dividido, g) {
            (false, _) if !self.abas.is_empty() => tooltip(
                botao_icone("columns", Msg::Dividir),
                "Dividir o editor (Ctrl+\\)",
                TooltipPosition::Bottom,
            )
            .style(tema::estilo_dica)
            .into(),
            (true, 1) => tooltip(
                botao_icone("x", Msg::Juntar),
                "Juntar os dois lados",
                TooltipPosition::Bottom,
            )
            .style(tema::estilo_dica)
            .into(),
            _ => Space::new(0.0, 0.0).into(),
        };
        let extra: Element<Msg> = match self.abas.get(ativa_aqui) {
            Some(a) if a.markdown() => botao_texto(
                row![
                    icone_traco(if a.previa { "code" } else { "eye" }, 14.0),
                    text(if a.previa { "Editar" } else { "Prévia" }).size(12)
                ]
                .spacing(6)
                .align(Align::Center),
                Msg::Previa,
            ),
            Some(a)
                if matches!(
                    a.caminho.extension().and_then(|e| e.to_str()),
                    Some("html" | "htm")
                ) =>
            {
                botao_texto(text("Abrir no navegador").size(12), Msg::AbrirNoNavegador)
            }
            _ => Space::new(0.0, 0.0).into(),
        };
        container(
            row![
                row(abas).height(Length::FILL),
                horizontal_space(),
                extra,
                dividir,
                container(rodar).padding([0, 10])
            ]
            .height(Length::FILL)
            .align(Align::Center),
        )
        .width(Length::FILL)
        .height(ALTURA_ABAS)
        .into()
    }

    fn editor_view(&self, g: usize) -> Element<'_, Msg> {
        let Some(aba) = self.abas.get(self.ativa_do(g)) else {
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
        if aba.previa {
            return container(
                rolagem(
                    container(markdown(&aba.conteudo.text(), 14.0, Msg::Link))
                        .padding([16, 24])
                        .width(Length::FILL),
                )
                .height(Length::FILL),
            )
            .width(Length::FILL)
            .height(Length::FILL)
            .style(|_| fundo(COR_PAINEL()))
            .into();
        }
        let extensao = aba
            .caminho
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt");
        let barra_aberta = self.barra.is_some();
        // A lista só vale onde foi pedida: o cursor andou (clique, seta), ela some.
        let (l, col) = aba.conteudo.cursor();
        let sugerindo = g == self.foco
            && !self.sugestoes.is_empty()
            && self.sugestao_em == (self.ativa, l, col);
        // O editor do pelicano rola, numera as linhas e desfaz sozinho; o app só cuida dos atalhos.
        let mut editor = text_editor(&aba.conteudo)
            .id(("editor", g))
            .on_change(Msg::Editou)
            // O chat também é um editor: sem clique, nenhum dos dois pega a digitação.
            .idle_focus(false)
            .on_key(move |k, m| {
                let ctrl = m.control_key() || m.super_key();
                match k {
                    Key::Named(NamedKey::Tab | NamedKey::Enter) if sugerindo && !ctrl => {
                        Some(Msg::AceitarSugestao)
                    }
                    Key::Named(NamedKey::ArrowDown) if sugerindo => Some(Msg::MoverSugestao(1)),
                    Key::Named(NamedKey::ArrowUp) if sugerindo => Some(Msg::MoverSugestao(-1)),
                    Key::Named(NamedKey::Escape) if sugerindo => Some(Msg::FecharSugestoes),
                    Key::Named(NamedKey::Tab) if !ctrl => Some(Msg::Recuar(!m.shift_key())),
                    Key::Character(c) if ctrl && matches!(c.as_str(), ";" | "/") => {
                        Some(Msg::Comentar)
                    }
                    Key::Character(c) if ctrl && !m.shift_key() => match c.as_str() {
                        "f" => Some(Msg::AbrirBarra(false, false)),
                        "h" => Some(Msg::AbrirBarra(false, true)),
                        "g" => Some(Msg::AbrirBarra(true, false)),
                        "\\" => Some(Msg::Dividir),
                        _ => None,
                    },
                    Key::Named(NamedKey::Escape) if barra_aberta => Some(Msg::FecharBarra),
                    Key::Named(NamedKey::Enter) if !ctrl && !m.alt_key() => Some(Msg::NovaLinha),
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
        if self.config.mostrar_invisiveis {
            editor = editor.invisibles(true);
        }
        if sugerindo {
            editor = editor.completions(self.sugestoes.clone(), self.sugestao_sel);
        }
        let avisos: Vec<pelicano::Warning> = self
            .problemas
            .iter()
            .filter(|p| p.caminho == aba.caminho)
            .filter_map(|p| Some(p.sublinhado(&aba.conteudo.line(p.linha)?)))
            .collect();
        if !avisos.is_empty() {
            editor = editor.warnings(avisos);
        }
        // O editor do pelicano não usa o botão direito: ele chega aqui e abre o menu.
        mouse_area(
            container(editor)
                .width(Length::FILL)
                .height(Length::FILL)
                .style(|_| fundo(COR_PAINEL())),
        )
        .on_right_press_at(|p| Msg::MenuContexto(Some((p, Alvo::Editor))))
        .into()
    }

    /// Faixa acima do editor quando o agente mudou no disco um arquivo com mudança não salva.
    fn conflito_view(&self) -> Element<'_, Msg> {
        if !self.aba().is_some_and(|a| a.conflito) {
            return Space::new(0.0, 0.0).into();
        }
        container(
            row![
                icone_traco("alert-triangle", 14.0),
                text("Este arquivo mudou no disco (o agente gravou) e a aba tem mudança não salva.")
                    .size(12)
                    .width(Length::FILL),
                botao_texto(text("Ver diferença").size(12), Msg::VerConflito),
                botao_texto(text("Ficar com a minha").size(12), Msg::ManterMinha),
                botao_texto(text("Usar a do disco").size(12), Msg::UsarDisco),
            ]
            .spacing(8)
            .align(Align::Center),
        )
        .padding([4, 12])
        .width(Length::FILL)
        .style(|_| fundo(AMARELO.alpha(0.18)))
        .into()
    }

    /// Caixinha flutuante no canto do editor: localizar/substituir (Ctrl+F, Ctrl+H) ou ir para
    /// a linha (Ctrl+G), como no VS Code.
    fn barra_view(&self) -> Option<Element<'_, Msg>> {
        let (Some(b), Some(aba)) = (&self.barra, self.aba()) else {
            return None;
        };
        let estilo = campo(if b.nada {
            mistura_cor(COR_PAINEL(), COR_ERRO(), 0.25)
        } else {
            COR_PAINEL()
        });
        let dica = com_dica;
        let fechar = dica(botao_icone("x", Msg::FecharBarra), "Fechar (Esc)");
        let corpo: Element<Msg> = if b.linha {
            row![
                text_input("Ir para a linha", b.termo.as_str())
                .id("barra")
                .on_input(Msg::TermoBarra)
                .on_submit(Msg::Proxima)
                .size(12)
                .padding([4, 8])
                .width(160.0)
                .style(estilo),
                text(format!("1 a {}", aba.conteudo.line_count()))
                    .size(11)
                    .color(COR_TEXTO_MUTED()),
                fechar,
            ]
            .spacing(6)
            .align(Align::Center)
            .into()
        } else {
            let achados = contar(&aba.conteudo.text(), &b.termo, b.maiusculas);
            let contagem = match achados {
                0 if b.termo.is_empty() => String::new(),
                0 => "nenhum".into(),
                n => n.to_string(),
            };
            let abrir_troca = dica(
                botao_icone(
                    if b.substituir { "chevron-down" } else { "chevron-right" },
                    Msg::AbrirBarra(false, !b.substituir),
                ),
                "Substituir (Ctrl+H)",
            );
            let maiusculas = dica(
                button(text("Aa").size(12).font(NEGRITO).color(if b.maiusculas {
                    COR_AZUL_ACENTO()
                } else {
                    COR_TEXTO_MUTED()
                }))
                .on_press(Msg::Maiusculas)
                .padding([2, 5])
                .style(move |_, s| item_lista(b.maiusculas, s, 4.0))
                .into(),
                "Diferenciar maiúsculas",
            );
            let mut linhas = column![row![
                abrir_troca,
                text_input("Localizar", b.termo.as_str())
                    .id("barra")
                    .on_input(Msg::TermoBarra)
                    .on_submit(Msg::Proxima)
                    .size(12)
                    .padding([4, 8])
                    .width(200.0)
                    .style(estilo),
                maiusculas,
                container(text(contagem).size(11).color(COR_TEXTO_MUTED()))
                    .width(46.0)
                    .align_x(Align::Center),
                dica(botao_icone("arrow-down", Msg::Proxima), "Próximo (Enter)"),
                fechar,
            ]
            .spacing(4)
            .align(Align::Center)]
            .spacing(4);
            if b.substituir {
                linhas = linhas.push(
                    row![
                        Space::new(24.0, 0.0),
                        text_input("Substituir por", b.novo.as_str())
                            .on_input(Msg::NovoBarra)
                            .on_submit(Msg::TrocarUma)
                            .size(12)
                            .padding([4, 8])
                            .width(200.0)
                            .style(campo(COR_PAINEL())),
                        dica(botao_icone("arrow-forward-up", Msg::TrocarUma), "Substituir (Enter)"),
                        dica(botao_icone("checks", Msg::TrocarTodas), "Substituir todos"),
                    ]
                    .spacing(4)
                    .align(Align::Center),
                );
            }
            linhas.into()
        };
        let caixa = container(corpo).padding([6, 8]).style(|_| {
            fundo(COR_CROMO_2())
                .border(Border::rounded(8.0).color(COR_BORDA(), 1.0))
                .shadow(Shadow {
                    color: Color::rgba(0.0, 0.0, 0.0, 0.3),
                    offset: Point::new(0.0, 6.0),
                    blur: 18.0,
                })
        });
        // Canto de cima à direita, longe da barra de rolagem.
        Some(
            container(caixa)
                .width(Length::FILL)
                .align_x(Align::End)
                .padding(Padding {
                    top: 8.0,
                    right: 22.0,
                    bottom: 0.0,
                    left: 0.0,
                })
                .into(),
        )
    }

    /// Diferença contra o último commit, em linha (como o modo inline do VS Code).
    fn diff_view<'a>(&'a self, d: &'a paineis::Diff) -> Element<'a, Msg> {
        const MAX: usize = 5000;
        // Lado a lado precisa de espaço: em editor estreito volta para a vista em linha. Com o
        // Git e o agente abertos o editor tem uns 560 px; 900 (o do desenho) nunca daria lado a lado.
        let lado = !self.config.diff_em_linha && self.largura_editor() >= 500.0;
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
                    // O conflito com o disco manda uma frase pronta em vez de um hash.
                    Some(h) if h.contains(' ') => h.clone(),
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
                botao_modo("Em linha", !lado, Msg::DiffEmLinha(true)),
                botao_modo("Lado a lado", lado, Msg::DiffEmLinha(false)),
                tooltip(
                    botao_icone("file-code", Msg::AbrirArquivo(d.caminho.clone())),
                    "Abrir arquivo",
                    TooltipPosition::Bottom
                )
                .style(tema::estilo_dica),
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
        if lado {
            lista = self.diff_lado_a_lado(d, MAX);
        }
        for l in d.linhas.iter().take(if lado { 0 } else { MAX }) {
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

    /// Largura aproximada do cartão do editor, para decidir a vista da diferença.
    fn largura_editor(&self) -> f32 {
        let mut w = self.tamanho.width - paineis::LARGURA_TRILHO - VAO;
        if self.lado.is_some() {
            w -= self.largura_lado() + VAO;
        }
        if self.agente_aberto {
            w -= self.largura_agente + VAO;
        }
        w
    }

    /// Antes à esquerda e depois à direita; onde um lado não tem linha, a faixa hachurada.
    fn diff_lado_a_lado<'a>(&'a self, d: &'a paineis::Diff, max: usize) -> pelicano::Column<'a, Msg> {
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
        let lado = |l: Option<&'a paineis::LinhaDiff>, antes: bool| -> Element<'a, Msg> {
            let Some(l) = l else {
                return canvas(Length::FILL, ALTURA_LINHA, |r, area, _| hachura(r, area)).into();
            };
            let cor = match l.tipo {
                '+' => VERDE.alpha(0.16),
                '-' => COR_ERRO().alpha(0.16),
                _ => Color::TRANSPARENT,
            };
            container(
                row![
                    numero(if antes { l.antes } else { l.depois }),
                    text(l.texto.as_str()).size(TAMANHO_FONTE).font(MONO).no_wrap()
                ]
                .spacing(12)
                .align(Align::Center),
            )
            .width(Length::FILL)
            .height(ALTURA_LINHA)
            .align_y(Align::Center)
            .clip(true)
            .style(move |_| fundo(cor))
            .into()
        };
        let mut lista = column![].width(Length::FILL);
        for (antes, depois) in paineis::lado_a_lado(&d.linhas).into_iter().take(max) {
            if let Some(l) = antes.filter(|l| l.tipo == '@') {
                lista = lista.push(
                    container(
                        text(format!("⋯  {}", l.texto))
                            .size(TAMANHO_FONTE - 1.0)
                            .font(MONO)
                            .color(COR_TEXTO_MUTED())
                            .no_wrap(),
                    )
                    .padding([0, 12])
                    .width(Length::FILL)
                    .height(ALTURA_LINHA)
                    .align_y(Align::Center)
                    .clip(true)
                    .style(|_| fundo(COR_AZUL_ACENTO().alpha(0.10))),
                );
                continue;
            }
            lista = lista.push(
                row![lado(antes, true), linha_vertical(), lado(depois, false)]
                    .width(Length::FILL)
                    .height(ALTURA_LINHA),
            );
        }
        lista
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
        let (pos, lingua, recuo) = match self.aba() {
            Some(a) => {
                let (l, c) = a.conteudo.cursor();
                let recuo = edicao::detectar_recuo(&a.conteudo).to_string();
                (format!("Ln {l}, Col {c}"), linguagem(&a.caminho), recuo)
            }
            None => (String::new(), String::new(), String::new()),
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
                // Clicar mostra (ou esconde) os espaços e tabs no editor.
                button(text(recuo).size(11).color(COR_TEXTO_MUTED()))
                    .on_press(Msg::AlternarInvisiveis)
                    .padding([0, 4])
                    .style(|_, s| item_lista(false, s, 4.0)),
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
                botao_icone("copy", Msg::Copiar(sem_links(fonte))),
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
    // Divisa::Editor fica na vertical, como Lado e Agente: arrasta na horizontal.
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

fn com_dica<'a>(e: Element<'a, Msg>, dica: &'static str) -> Element<'a, Msg> {
    tooltip(e, dica, TooltipPosition::Bottom)
        .style(tema::estilo_dica)
        .into()
}

/// Riscos diagonais na área: onde a vista lado a lado não tem linha.
fn hachura(r: &mut pelicano::Renderer, a: pelicano::Rect) {
    use pelicano::{PathBuilder, Stroke};
    let mut p = PathBuilder::new();
    let mut d = -a.height;
    while d < a.width {
        // Risco de (d, embaixo) até (d + altura, em cima), cortado nas bordas.
        let (mut x0, mut y0) = (a.x + d, a.y + a.height);
        let (mut x1, mut y1) = (a.x + d + a.height, a.y);
        if x0 < a.x {
            y0 -= a.x - x0;
            x0 = a.x;
        }
        if x1 > a.x + a.width {
            y1 += x1 - (a.x + a.width);
            x1 = a.x + a.width;
        }
        p.move_to(Point::new(x0, y0)).line_to(Point::new(x1, y1));
        d += 7.0;
    }
    if let Some(p) = p.build() {
        r.stroke_path(&p, &Stroke::new(1.0), COR_BORDA_SUAVE());
    }
}

/// Botão de escolha (Em linha | Lado a lado): o escolhido fica com o fundo do acento.
fn botao_modo<'a>(rotulo: &'a str, escolhido: bool, msg: Msg) -> Element<'a, Msg> {
    button(text(rotulo).size(12).color(if escolhido {
        COR_AZUL_ACENTO()
    } else {
        COR_TEXTO_MUTED()
    }))
    .on_press(msg)
    .padding([2, 8])
    .style(move |_, s| item_lista(escolhido, s, 5.0))
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
    fn novo_arquivo_cria_vazio_e_nao_esvazia_o_que_ja_existe() {
        use super::*;
        let pasta = std::env::temp_dir().join(format!("fc-novo-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let (mut code, _) = Code::novo(config::Config::ler());
        let novo = pasta.join("novo.rs");
        let _ = code.update(Msg::NovoArquivoEscolhido(Some(novo.clone())));
        assert_eq!(std::fs::read_to_string(&novo).unwrap(), "");
        let existente = pasta.join("existente.rs");
        std::fs::write(&existente, "fn main() {}").unwrap();
        let _ = code.update(Msg::NovoArquivoEscolhido(Some(existente.clone())));
        assert_eq!(std::fs::read_to_string(&existente).unwrap(), "fn main() {}");
        std::fs::remove_dir_all(pasta).unwrap();
    }

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
        code.abas.push(Aba::nova(code.arvore.raiz.join("x.rs"), "fn a() {}\nfn b() {}\n"));
        let _ = code.update(Msg::MandarTrecho);
        assert_eq!(code.chat.trechos[0].0, "x.rs 1");
        let _ = code.update(Msg::Enviar);
        assert!(code.chat.trechos.is_empty());
        assert!(matches!(code.chat.itens.last(), Some(Item::Eu(t)) if t == "[x.rs 1]"));
        let linha = &code.chat.fila[0];
        assert!(linha.contains("linhas 1 a 1") && linha.contains("fn a() {}"));
    }

    #[test]
    fn aba_suja_pergunta_ao_fechar_e_marca_conflito_com_o_disco() {
        use super::*;
        let pasta = std::env::temp_dir().join(format!("feathercode-conflito-t-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let (a, b) = (pasta.join("a.txt"), pasta.join("b.txt"));
        std::fs::write(&a, "um").unwrap();
        std::fs::write(&b, "um").unwrap();
        let (mut code, _) = Code::novo(config::Config::ler());
        // Sem AbrirArquivo: ele gravaria o arquivo nos recentes da config de verdade.
        code.abas.push(Aba::nova(a.clone(), "um"));
        code.abas.push(Aba::nova(b.clone(), "um"));
        code.abas[0].conteudo.insert("minha ");
        // O agente grava nos dois: a aba limpa recarrega, a suja fica em conflito.
        std::fs::write(&a, "do agente").unwrap();
        std::fs::write(&b, "do agente").unwrap();
        code.recarregar_abas();
        assert!(code.abas[0].conflito && code.abas[0].sujo());
        assert!(!code.abas[1].conflito);
        assert_eq!(code.abas[1].conteudo.text().trim_end(), "do agente");
        let _ = code.update(Msg::FecharAba(0));
        assert_eq!(code.confirmar, Some(Confirmar::FecharAba(0)));
        assert_eq!(code.abas.len(), 2);
        let _ = code.update(Msg::Confirmado(Some(false)));
        assert_eq!(code.abas.len(), 1);
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "do agente");
        assert!(code.aplicar_nome(&PedirNome::NovoArquivo(pasta.clone()), "../fora.txt").is_err());
        assert!(code.aplicar_nome(&PedirNome::Renomear(b.clone()), "a.txt").is_err());
        assert_eq!(
            code.aplicar_nome(&PedirNome::NovoArquivo(pasta.clone()), "sub/c.rs"),
            Ok(Some(pasta.join("sub/c.rs")))
        );
        std::fs::remove_dir_all(&pasta).unwrap();
    }

    #[test]
    fn dividir_fechar_de_um_lado_e_juntar() {
        use super::*;
        let (mut code, _) = Code::novo(config::Config::ler());
        let raiz = code.arvore.raiz.clone();
        for n in ["a.rs", "b.rs"] {
            code.abas.push(Aba::nova(raiz.join(n), ""));
        }
        code.ativa = 1;
        let _ = code.update(Msg::Dividir);
        // b.rs nos dois lados, o teclado à direita.
        assert!(code.dividido && code.foco == 1);
        assert_eq!((code.abas[0].grupos, code.abas[1].grupos), (1, 3));
        // Fechar à direita não fecha à esquerda; lado vazio junta.
        let _ = code.update(Msg::FecharAba(1));
        assert_eq!(code.abas.len(), 2);
        assert!(!code.dividido && code.foco == 0);
        assert_eq!(code.ativa, 1);
        // Aba suja que está só num lado pergunta; nos dois lados, só sai daquele.
        let _ = code.update(Msg::Dividir);
        code.abas[1].conteudo.insert("x");
        let _ = code.update(Msg::FocarGrupo(0));
        assert_eq!((code.foco, code.ativa, code.outra_ativa), (0, 1, 1));
        let _ = code.update(Msg::FecharAba(1));
        assert!(code.confirmar.is_none());
        assert_eq!(code.abas[1].grupos, 2);
        assert_eq!(code.ativa, 0);
        let _ = code.update(Msg::FocarGrupo(1));
        let _ = code.update(Msg::FecharAba(1));
        assert_eq!(code.confirmar, Some(Confirmar::FecharAba(1)));
        code.confirmar = None;
        let _ = code.update(Msg::Juntar);
        assert!(code.abas.iter().all(|a| a.grupos == 1));
        assert_eq!(code.ativa, 1);
    }

    #[test]
    fn caminhos_da_resposta_viram_links_e_saem_ao_copiar() {
        use super::*;
        let raiz = std::env::temp_dir().join(format!("feathercode-links-{}", std::process::id()));
        std::fs::create_dir_all(raiz.join("src")).unwrap();
        std::fs::write(raiz.join("src/a.rs"), "").unwrap();
        let texto = "Mudei `src/a.rs:42`, veja (src/a.rs).\n```\nsrc/a.rs:1\n```\nnao.rs:3 e src/a.rs:x";
        let ligado = ligar_caminhos(texto, &raiz);
        assert_eq!(
            ligado,
            "Mudei [src/a.rs:42](arquivo:src/a.rs:42), veja ([src/a.rs](arquivo:src/a.rs)).\n```\nsrc/a.rs:1\n```\nnao.rs:3 e src/a.rs:x"
        );
        assert_eq!(
            sem_links(&ligado),
            "Mudei src/a.rs:42, veja (src/a.rs).\n```\nsrc/a.rs:1\n```\nnao.rs:3 e src/a.rs:x"
        );
        std::fs::remove_dir_all(&raiz).unwrap();
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
