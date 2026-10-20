//! FeatherCode: editor de código da suíte FeatherWorks, com o Claude Code no painel lateral.
mod agente;
mod arvore;

use agente::{Item, Resposta, Sinal};
use feather_ui::cromo;
use feather_ui::icones::icone;
use feather_ui::tema::{self, COR_AZUL_ACENTO, COR_AZUL_FUNDO, COR_BORDA, COR_BORDA_SUAVE, COR_CROMO, COR_CROMO_2, COR_ERRO, COR_FUNDO_JANELA, COR_HOVER, COR_PAINEL, COR_PAINEL_FAIXA, COR_TEXTO, COR_TEXTO_MUTED};
use iced::widget::{button, column, container, horizontal_space, pick_list, progress_bar, row, text, text_editor, text_input, Column, Space};
use iced::{font, highlighter, keyboard, window, Alignment, Background, Border, Color, Element, Font, Length, Size, Subscription, Task};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::UnboundedSender;

const NEGRITO: Font = Font { weight: font::Weight::Semibold, ..Font::DEFAULT };
const MONO: Font = Font::MONOSPACE;
/// Índice de "Roxo" em `tema::ACENTOS`: a cor do FeatherCode.
const ROXO: usize = 5;
const VERDE: Color = Color::from_rgb(0.19, 0.6, 0.33);

fn main() -> iced::Result {
    tema::definir_tema(tema::escuro_resolvido(true, false), ROXO);
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
        .run_with(Code::novo)
}

struct Aba {
    caminho: PathBuf,
    conteudo: text_editor::Content,
    sujo: bool,
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
    Comando(String),
    Rodar,
    Rodou(String),
    Janela(Option<window::Id>),
    Arrastar,
    Minimizar,
    Maximizar,
    Fechar,
}

struct Code {
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
    modelo: String,
    proximo_controle: u64,
    comando: String,
    saida: String,
    rodando: bool,
    aviso: String,
    janela: Option<window::Id>,
    ultimo_clique_barra: Option<std::time::Instant>,
}

impl Code {
    fn novo() -> (Self, Task<Msg>) {
        let pasta = std::env::args_os().nth(1).map(PathBuf::from).or_else(|| std::env::current_dir().ok()).unwrap_or_else(|| ".".into());
        let pasta = pasta.canonicalize().unwrap_or(pasta);
        let code = Code {
            arvore: arvore::Arvore::nova(pasta),
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
            modelo: String::new(),
            proximo_controle: 0,
            comando: String::new(),
            saida: String::new(),
            rodando: false,
            aviso: String::new(),
            janela: None,
            ultimo_clique_barra: None,
        };
        (code, window::get_latest().map(Msg::Janela))
    }

    fn subscription(&self) -> Subscription<Msg> {
        let teclas = keyboard::on_key_press(|tecla, m| match tecla.as_ref() {
            keyboard::Key::Character("s") if m.command() => Some(Msg::Salvar),
            keyboard::Key::Character("o") if m.command() => Some(Msg::AbrirPasta),
            _ => None,
        });
        let mut subs = vec![teclas];
        if self.ligado {
            let conversa = agente::conversa(self.arvore.raiz.clone(), self.modo.id());
            subs.push(Subscription::run_with_id(("agente", self.conversa), conversa).map(Msg::Agente));
        }
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

    fn aba(&self) -> Option<&Aba> {
        self.abas.get(self.ativa)
    }

    fn relativo<'a>(&self, p: &'a Path) -> &'a Path {
        p.strip_prefix(&self.arvore.raiz).unwrap_or(p)
    }

    /// O que o claude precisa saber do editor junto com o pedido: arquivo, linha e seleção.
    fn contexto(&self) -> String {
        let Some(aba) = self.aba() else { return String::new() };
        let (linha, _) = aba.conteudo.cursor_position();
        let mut s = format!("\n\n<contexto_do_editor>\nArquivo aberto no FeatherCode: {} (cursor na linha {})", self.relativo(&aba.caminho).display(), linha + 1);
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
            Sinal::Modelo(m) => self.modelo = m,
            Sinal::FimDoTurno(erro) => {
                self.ocupado = false;
                self.itens.extend(erro.map(Item::Aviso));
                self.arvore.recarregar();
                self.recarregar_abas();
            }
        }
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::AbrirPasta => {
                return Task::perform(rfd::AsyncFileDialog::new().set_title("Abrir pasta").pick_folder(), |h| Msg::PastaEscolhida(h.map(|h| h.path().to_path_buf())));
            }
            Msg::PastaEscolhida(Some(p)) => {
                if self.abas.iter().any(|a| a.sujo) {
                    self.aviso = "Salve ou feche as abas com mudanças antes de trocar de pasta.".into();
                    return Task::none();
                }
                self.arvore = arvore::Arvore::nova(p);
                self.abas.clear();
                self.ativa = 0;
                return self.update(Msg::NovaConversa);
            }
            Msg::PastaEscolhida(None) => {}
            Msg::Alternar(p) => self.arvore.alternar(&p),
            Msg::AbrirArquivo(p) => {
                if let Some(i) = self.abas.iter().position(|a| a.caminho == p) {
                    self.ativa = i;
                } else {
                    match std::fs::read_to_string(&p) {
                        Ok(t) => {
                            self.abas.push(Aba { caminho: p, conteudo: text_editor::Content::with_text(&t), sujo: false });
                            self.ativa = self.abas.len() - 1;
                            self.aviso.clear();
                        }
                        Err(e) => self.aviso = format!("Não deu para abrir {} como texto: {e}", self.relativo(&p).display()),
                    }
                }
            }
            Msg::Editar(acao) => {
                if let Some(aba) = self.abas.get_mut(self.ativa) {
                    aba.sujo |= acao.is_edit();
                    aba.conteudo.perform(acao);
                }
            }
            Msg::TrocarAba(i) => self.ativa = i,
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
            }
            Msg::Modo(m) => {
                self.modo = m;
                if self.canal.is_some() {
                    self.controle(serde_json::json!({"subtype": "set_permission_mode", "mode": m.id()}));
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
                let Some(Item::Pedido { request_id, input, sugestoes, resposta, .. }) = self.itens.get_mut(i) else { return Task::none() };
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
            Msg::Comando(c) => self.comando = c,
            Msg::Rodar => {
                let cmd = std::mem::take(&mut self.comando);
                if cmd.trim().is_empty() || self.rodando {
                    self.comando = cmd;
                    return Task::none();
                }
                self.rodando = true;
                self.saida.push_str(&format!("$ {cmd}\n"));
                return Task::perform(rodar(cmd, self.arvore.raiz.clone()), Msg::Rodou);
            }
            Msg::Rodou(s) => {
                self.rodando = false;
                self.saida.push_str(&s);
                // shortcut: guarda só o fim da saída; um terminal de verdade (PTY) substitui isto.
                if self.saida.len() > 200_000 {
                    let mut corte = self.saida.len() - 150_000;
                    while !self.saida.is_char_boundary(corte) {
                        corte += 1;
                    }
                    self.saida.drain(..corte);
                }
                self.arvore.recarregar();
            }
            Msg::Janela(id) => self.janela = id,
            Msg::Arrastar => {
                let Some(id) = self.janela else { return Task::none() };
                return feather_ui::janelas::arrastar_ou_maximizar(id, &mut self.ultimo_clique_barra);
            }
            Msg::Minimizar => return self.janela.map_or_else(Task::none, |id| window::minimize(id, true)),
            Msg::Maximizar => return self.janela.map_or_else(Task::none, window::toggle_maximize),
            Msg::Fechar => return self.janela.map_or_else(Task::none, window::close),
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Msg> {
        let pasta = self.arvore.raiz.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let nome = container(
            row![
                cromo::emblema(),
                text("FeatherCode").size(13).font(NEGRITO),
                text(pasta).size(13).color(COR_TEXTO_MUTED()),
                horizontal_space(),
                button(row![icone("folder-open", 15.0), text("Abrir pasta").size(12)].spacing(6).align_y(Alignment::Center))
                    .on_press(Msg::AbrirPasta)
                    .padding([4, 10])
                    .style(tema::estilo_botao_secundario),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .padding([0, 12])
        .width(Length::Fill)
        .height(cromo::BARRA_ALTURA)
        .align_y(Alignment::Center)
        .style(tema::estilo_container_topo);
        let barra = cromo::barra_integrada(nome.into(), cromo::AcoesJanela { arrastar: Msg::Arrastar, minimizar: Msg::Minimizar, maximizar: Msg::Maximizar, fechar: Msg::Fechar });

        let meio = row![
            self.arvore_view(),
            linha_vertical(),
            column![self.abas_view(), self.editor_view(), self.terminal_view()].width(Length::Fill),
            linha_vertical(),
            self.agente_view(),
        ]
        .height(Length::Fill);
        cromo::moldura_janela(column![barra, meio, self.status_view()].into())
    }

    fn arvore_view(&self) -> Element<'_, Msg> {
        let aberto = self.aba().map(|a| a.caminho.as_path());
        let linhas = self.arvore.linhas().into_iter().map(|l| {
            let nome = l.caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let marca: Element<Msg> = if l.pasta { icone(if l.aberta { "chevron-down" } else { "chevron-right" }, 12.0).into() } else { Space::with_width(12).into() };
            let simbolo = icone(if l.pasta { "folder" } else { "file-code" }, 14.0);
            let ativo = aberto == Some(l.caminho.as_path());
            let msg = if l.pasta { Msg::Alternar(l.caminho) } else { Msg::AbrirArquivo(l.caminho) };
            button(row![Space::with_width(l.nivel as f32 * 14.0), marca, simbolo, text(nome).size(13)].spacing(5).align_y(Alignment::Center))
                .on_press(msg)
                .width(Length::Fill)
                .padding([2, 8])
                .style(move |_, s| item_lista(ativo, s))
                .into()
        });
        let titulo = text(self.arvore.raiz.file_name().map(|n| n.to_string_lossy().to_uppercase()).unwrap_or_default()).size(11).font(NEGRITO).color(COR_TEXTO_MUTED());
        container(column![container(titulo).padding([8, 12]), tema::rolagem(Column::with_children(linhas)).height(Length::Fill)])
            .width(240)
            .height(Length::Fill)
            .style(|_| fundo(COR_PAINEL_FAIXA()))
            .into()
    }

    fn abas_view(&self) -> Element<'_, Msg> {
        let abas = self.abas.iter().enumerate().map(|(i, a)| {
            let ativa = i == self.ativa;
            let nome = a.caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let fechar = button(text(if a.sujo { "●" } else { "×" }).size(13)).on_press(Msg::FecharAba(i)).padding([0, 4]).style(|_, s| item_lista(false, s));
            button(row![text(nome).size(12), fechar].spacing(6).align_y(Alignment::Center))
                .on_press(Msg::TrocarAba(i))
                .padding([6, 12])
                .style(move |_, s| aba_estilo(ativa, s))
                .into()
        });
        container(row(abas)).width(Length::Fill).height(34).style(|_| fundo(COR_CROMO_2())).into()
    }

    fn editor_view(&self) -> Element<'_, Msg> {
        let Some(aba) = self.aba() else {
            let vazio = column![
                text("Nenhum arquivo aberto").size(16).font(NEGRITO),
                text("Escolha um arquivo na árvore à esquerda, ou peça ao Claude Code no painel da direita.").size(13).color(COR_TEXTO_MUTED()),
                text("Ctrl+O abre outra pasta · Ctrl+S salva").size(12).color(COR_TEXTO_MUTED()),
            ]
            .spacing(8)
            .align_x(Alignment::Center);
            return container(vazio).center(Length::Fill).style(|_| fundo(COR_PAINEL())).into();
        };
        let extensao = aba.caminho.extension().and_then(|e| e.to_str()).unwrap_or("txt");
        let tema_cor = if tema::escuro() { highlighter::Theme::Base16Ocean } else { highlighter::Theme::InspiredGitHub };
        text_editor(&aba.conteudo)
            .on_action(Msg::Editar)
            .font(MONO)
            .size(13)
            .padding([8, 12])
            .height(Length::Fill)
            .highlight(extensao, tema_cor)
            .style(|_, _| text_editor::Style {
                background: Background::Color(COR_PAINEL()),
                border: Border::default(),
                icon: COR_TEXTO_MUTED(),
                placeholder: COR_TEXTO_MUTED(),
                value: COR_TEXTO(),
                selection: COR_AZUL_FUNDO(),
            })
            .into()
    }

    fn terminal_view(&self) -> Element<'_, Msg> {
        let entrada = text_input(if self.rodando { "Rodando…" } else { "Comando na pasta (ex.: cargo build)" }, &self.comando)
            .on_input(Msg::Comando)
            .on_submit(Msg::Rodar)
            .font(MONO)
            .size(12)
            .padding(0)
            .style(campo_sem_borda);
        let saida = tema::rolagem(text(&self.saida).font(MONO).size(12).color(COR_TEXTO_MUTED())).anchor_bottom().height(Length::Fill).width(Length::Fill);
        container(column![
            text("TERMINAL").size(11).font(NEGRITO).color(COR_TEXTO_MUTED()),
            saida,
            row![text("$").font(MONO).size(12).color(COR_AZUL_ACENTO()), entrada].spacing(8).align_y(Alignment::Center),
        ]
        .spacing(6))
        .padding([8, 12])
        .width(Length::Fill)
        .height(190)
        .style(|_| container::Style { border: Border { color: COR_BORDA(), width: 1.0, radius: 0.0.into() }, ..fundo(COR_PAINEL_FAIXA()) })
        .into()
    }

    fn agente_view(&self) -> Element<'_, Msg> {
        let titulo = if self.modelo.is_empty() { "Claude Code".to_string() } else { format!("Claude Code · {}", self.modelo.trim_start_matches("claude-")) };
        let topo = container(
            row![
                text(titulo).size(13).font(NEGRITO),
                horizontal_space(),
                button(icone("plus", 15.0)).on_press(Msg::NovaConversa).padding(4).style(|_, s| item_lista(false, s)),
            ]
            .align_y(Alignment::Center),
        )
        .padding([0, 12])
        .height(34)
        .align_y(Alignment::Center)
        .style(|_| fundo(COR_CROMO_2()));

        let mut corpo = Column::new().spacing(12).padding([12, 14]);
        if let (Some(_), _) | (_, Some(_)) = self.uso {
            corpo = corpo.push(column![barra_uso("Sessão (5 h)", self.uso.0), barra_uso("Semana (7 dias)", self.uso.1)].spacing(6));
        }
        if self.itens.is_empty() {
            corpo = corpo.push(
                column![
                    text("Peça uma mudança ou pergunte sobre o código.").size(13),
                    text("O Claude Code roda na pasta aberta, com a sua conta. Antes de gravar um arquivo ou rodar um comando, ele pede permissão aqui.").size(12).color(COR_TEXTO_MUTED()),
                ]
                .spacing(6),
            );
        }
        for (i, item) in self.itens.iter().enumerate() {
            corpo = corpo.push(item_view(i, item));
        }
        if self.ocupado {
            corpo = corpo.push(text("Trabalhando…").size(12).color(COR_TEXTO_MUTED()));
        }

        let chip: Element<Msg> = match self.aba() {
            Some(a) => {
                let (l, _) = a.conteudo.cursor_position();
                container(text(format!("{}:{}", self.relativo(&a.caminho).display(), l + 1)).font(MONO).size(11).color(COR_TEXTO_MUTED()))
                    .padding([1, 6])
                    .style(|_| container::Style { border: Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: 4.0.into() }, ..Default::default() })
                    .into()
            }
            None => Space::with_height(0).into(),
        };
        let acao = if self.ocupado {
            button(icone("player-stop", 14.0)).on_press(Msg::Parar).padding(4).style(tema::estilo_botao_secundario)
        } else {
            button(icone("arrow-up", 14.0)).on_press(Msg::Enviar).padding(4).style(tema::estilo_botao_primario)
        };
        let entrada = container(
            column![
                chip,
                text_input("Peça uma mudança ou pergunte…", &self.pedido).on_input(Msg::Pedido).on_submit(Msg::Enviar).size(13).padding([4, 2]).style(campo_sem_borda),
                row![
                    pick_list(Modo::TODOS, Some(self.modo), Msg::Modo).text_size(12).padding([2, 6]).style(tema::estilo_menu_da_faixa).menu_style(tema::estilo_menu_popup),
                    horizontal_space(),
                    acao,
                ]
                .align_y(Alignment::Center),
            ]
            .spacing(6),
        )
        .padding(8)
        .style(|_| container::Style { border: Border { color: COR_BORDA(), width: 1.0, radius: 6.0.into() }, ..fundo(COR_PAINEL()) });

        container(column![topo, tema::rolagem(corpo).anchor_bottom().height(Length::Fill), container(entrada).padding(iced::Padding { top: 0.0, right: 10.0, bottom: 10.0, left: 10.0 })])
            .width(380)
            .height(Length::Fill)
            .style(|_| fundo(COR_PAINEL_FAIXA()))
            .into()
    }

    fn status_view(&self) -> Element<'_, Msg> {
        let (pos, lingua) = match self.aba() {
            Some(a) => {
                let (l, c) = a.conteudo.cursor_position();
                (format!("Ln {}, Col {}", l + 1, c + 1), a.caminho.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default())
            }
            None => (String::new(), String::new()),
        };
        let aviso = text(&self.aviso).size(11).color(COR_ERRO());
        container(row![aviso, horizontal_space(), text(pos).size(11), text("UTF-8").size(11), text(lingua).size(11)].spacing(18).align_y(Alignment::Center))
            .padding([0, 12])
            .height(24)
            .align_y(Alignment::Center)
            .style(|_| container::Style { text_color: Some(COR_TEXTO_MUTED()), ..fundo(COR_CROMO()) })
            .into()
    }
}

fn item_view(i: usize, item: &Item) -> Element<'_, Msg> {
    match item {
        Item::Eu(t) => container(text(t).size(13))
            .padding([7, 10])
            .width(Length::Fill)
            .style(|_| container::Style { border: Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: 6.0.into() }, ..fundo(COR_PAINEL()) })
            .into(),
        // shortcut: resposta em texto puro; o widget markdown do Iced entra quando o bloco de código fizer falta.
        Item::Texto(t) => text(t).size(13).into(),
        Item::Ferramenta { frase, erro, .. } => row![text("●").size(9).color(if *erro { COR_ERRO() } else { VERDE }), text(frase).size(12).color(COR_TEXTO_MUTED())]
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
        Item::Aviso(t) => text(t).size(12).color(COR_ERRO()).into(),
        Item::Pedido { frase, previa, resposta, .. } => {
            let mut cartao = Column::new().push(container(text(frase).size(12).font(NEGRITO)).padding([6, 10]));
            if !previa.is_empty() {
                let linhas = previa.iter().map(|(s, l)| {
                    let cor = match s {
                        '-' => COR_ERRO(),
                        '+' => VERDE,
                        _ => COR_TEXTO_MUTED(),
                    };
                    text(format!("{s} {l}")).font(MONO).size(11).color(cor).into()
                });
                cartao = cartao.push(container(Column::with_children(linhas)).padding([4, 10]).width(Length::Fill).style(|_| fundo(COR_FUNDO_JANELA())));
            }
            let fim: Element<Msg> = match resposta {
                Resposta::Pendente => column![
                    opcao("1", "Permitir", Msg::Responder(i, Resposta::Gravou), true),
                    opcao("2", "Permitir e não perguntar nesta sessão", Msg::Responder(i, Resposta::Sempre), false),
                    opcao("3", "Não, explicar o que mudar", Msg::Responder(i, Resposta::Recusou), false),
                ]
                .padding(6)
                .into(),
                Resposta::Recusou => container(text("Recusado").size(12).color(COR_ERRO())).padding([6, 10]).into(),
                _ => container(text("Permitido").size(12).color(VERDE)).padding([6, 10]).into(),
            };
            let pendente = *resposta == Resposta::Pendente;
            container(cartao.push(fim))
                .width(Length::Fill)
                .style(move |_| container::Style { border: Border { color: if pendente { COR_AZUL_ACENTO() } else { COR_BORDA() }, width: 1.0, radius: 6.0.into() }, ..fundo(COR_PAINEL()) })
                .into()
        }
    }
}

fn opcao<'a>(tecla: &'a str, rotulo: &'a str, msg: Msg, foco: bool) -> Element<'a, Msg> {
    button(row![text(tecla).font(MONO).size(11).color(COR_TEXTO_MUTED()), text(rotulo).size(12)].spacing(8))
        .on_press(msg)
        .width(Length::Fill)
        .padding([4, 6])
        .style(move |_, s| item_lista(foco, s))
        .into()
}

fn barra_uso(rotulo: &str, uso: Option<f32>) -> Element<'_, Msg> {
    let u = uso.unwrap_or(0.0);
    column![
        row![text(rotulo).size(11).color(COR_TEXTO_MUTED()), horizontal_space(), text(format!("{:.0}%", u * 100.0)).size(11).color(COR_TEXTO_MUTED())],
        progress_bar(0.0..=1.0, u).height(4).style(|_| progress_bar::Style {
            background: COR_BORDA_SUAVE().into(),
            bar: COR_AZUL_ACENTO().into(),
            border: Border { radius: 2.0.into(), ..Default::default() },
        }),
    ]
    .spacing(3)
    .into()
}

async fn rodar(cmd: String, pasta: PathBuf) -> String {
    // shortcut: roda até o fim e mostra a saída; sem PTY, programa interativo não funciona.
    #[cfg(windows)]
    let mut c = tokio::process::Command::new("cmd");
    #[cfg(windows)]
    c.arg("/C");
    #[cfg(not(windows))]
    let mut c = tokio::process::Command::new("sh");
    #[cfg(not(windows))]
    c.arg("-c");
    match c.arg(&cmd).current_dir(pasta).output().await {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
            s.push_str(&String::from_utf8_lossy(&o.stderr));
            if !o.status.success() {
                s.push_str(&format!("[{}]\n", o.status));
            }
            s
        }
        Err(e) => format!("Não deu para rodar: {e}\n"),
    }
}

fn fundo(cor: Color) -> container::Style {
    container::Style { background: Some(cor.into()), text_color: Some(COR_TEXTO()), ..Default::default() }
}

fn linha_vertical<'a>() -> Element<'a, Msg> {
    container(Space::with_width(1)).height(Length::Fill).style(|_| fundo(COR_BORDA_SUAVE())).into()
}

fn item_lista(ativo: bool, s: button::Status) -> button::Style {
    let fundo = match (ativo, s) {
        (true, _) => Some(COR_AZUL_FUNDO().into()),
        (_, button::Status::Hovered | button::Status::Pressed) => Some(COR_HOVER().into()),
        _ => None,
    };
    button::Style { background: fundo, text_color: if ativo { COR_AZUL_ACENTO() } else { COR_TEXTO() }, border: Border { radius: 4.0.into(), ..Default::default() }, ..Default::default() }
}

fn aba_estilo(ativa: bool, s: button::Status) -> button::Style {
    let fundo = if ativa { Some(COR_PAINEL().into()) } else if matches!(s, button::Status::Hovered) { Some(COR_HOVER().into()) } else { None };
    button::Style {
        background: fundo,
        text_color: if ativa { COR_TEXTO() } else { COR_TEXTO_MUTED() },
        ..Default::default()
    }
}

fn campo_sem_borda(t: &iced::Theme, s: text_input::Status) -> text_input::Style {
    text_input::Style { background: Color::TRANSPARENT.into(), border: Border::default(), value: COR_TEXTO(), ..text_input::default(t, s) }
}
