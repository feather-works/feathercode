//! Painel de terminais: shell de verdade num PTY (iced_term, motor do Alacritty), em abas.
use crate::*;
use iced::widget::{column, row};

pub struct Aba {
    pub termo: iced_term::Terminal,
    pub nome: String,
}

#[derive(Default)]
pub struct Terminais {
    pub abas: Vec<Aba>,
    pub ativa: usize,
    proximo: u64,
    /// Id do terminal aberto pelo botão Rodar; rodar de novo fecha o anterior.
    rodar: Option<u64>,
    /// Id do terminal do último pull/push/fetch, trocado a cada comando.
    git: Option<u64>,
}

fn shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into())
}

fn hex(c: Color) -> String {
    let canal = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", canal(c.r), canal(c.g), canal(c.b))
}

/// Paleta no fundo do terminal da janela; as cores ANSI ficam as do iced_term.
fn configuracao(programa: String, args: Vec<String>) -> iced_term::settings::Settings {
    let paleta = iced_term::ColorPalette {
        background: hex(cor_terminal()),
        foreground: hex(COR_TEXTO()),
        ..Default::default()
    };
    iced_term::settings::Settings {
        font: iced_term::settings::FontSettings {
            size: 13.0,
            scale_factor: 1.3,
            font_type: MONO,
        },
        theme: iced_term::settings::ThemeSettings::new(Box::new(paleta)),
        backend: iced_term::settings::BackendSettings {
            program: programa,
            args,
        },
    }
}

impl Terminais {
    /// Abre um shell na pasta. O iced_term não deixa escolher a pasta, então um `sh` entra nela e vira o shell.
    pub fn novo(&mut self, pasta: &Path, comando: Option<&str>) -> Result<u64, String> {
        let mut script = String::from("cd \"$1\" || exit 1; ");
        if comando.is_some() {
            script.push_str("eval \"$3\"; printf '\\n[fim: código %s]\\n' \"$?\"; ");
        }
        script.push_str("exec \"$2\"");
        let args = vec![
            "-c".into(),
            script,
            "sh".into(),
            pasta.display().to_string(),
            shell(),
            comando.unwrap_or_default().to_string(),
        ];
        let id = self.proximo;
        self.proximo += 1;
        let termo = iced_term::Terminal::new(id, configuracao("/bin/sh".into(), args))
            .map_err(|e| format!("Não deu para abrir o terminal: {e}"))?;
        let nome = comando.map_or_else(
            || {
                std::path::Path::new(&shell())
                    .file_name()
                    .map_or("shell".into(), |n| n.to_string_lossy().into_owned())
            },
            |c| c.split_whitespace().take(2).collect::<Vec<_>>().join(" "),
        );
        self.abas.push(Aba { termo, nome });
        self.ativa = self.abas.len() - 1;
        Ok(id)
    }

    /// Roda o comando do projeto numa aba própria, fechando a da execução anterior.
    pub fn rodar(&mut self, pasta: &Path, comando: &str) -> Result<u64, String> {
        if let Some(id) = self.rodar.take() {
            self.fechar(id);
        }
        let id = self.novo(pasta, Some(comando))?;
        self.rodar = Some(id);
        Ok(id)
    }

    pub fn rodar_git(&mut self, pasta: &Path, comando: &str) -> Result<u64, String> {
        if let Some(id) = self.git.take() {
            self.fechar(id);
        }
        let id = self.novo(pasta, Some(comando))?;
        self.git = Some(id);
        Ok(id)
    }

    /// Fechar a aba derruba o PTY e o processo dela.
    pub fn fechar(&mut self, id: u64) {
        self.abas.retain(|a| a.termo.id != id);
        self.ativa = self.ativa.min(self.abas.len().saturating_sub(1));
    }

    pub fn ativo(&self) -> Option<&iced_term::Terminal> {
        self.abas.get(self.ativa).map(|a| &a.termo)
    }

    /// Evento do PTY: atualiza a tela do terminal e fecha a aba quando o shell sai.
    pub fn tratar(&mut self, ev: iced_term::Event) {
        let iced_term::Event::BackendCall(id, cmd) = ev;
        let Some(aba) = self.abas.iter_mut().find(|a| a.termo.id == id) else {
            return;
        };
        if let iced_term::actions::Action::Shutdown =
            aba.termo.handle(iced_term::Command::ProxyToBackend(cmd))
        {
            self.fechar(id);
        }
    }

    pub fn subscription(&self) -> Subscription<Msg> {
        Subscription::batch(self.abas.iter().map(|a| {
            Subscription::run_with_id(("terminal", a.termo.id), a.termo.subscription())
                .map(Msg::Term)
        }))
    }
}

impl Code {
    pub(crate) fn terminal_view(&self) -> Element<'_, Msg> {
        let t = &self.terminais;
        let abas = t.abas.iter().enumerate().map(|(i, a)| {
            let ativa = i == t.ativa;
            let fechar = button(text("×").size(12).color(COR_TEXTO_MUTED()))
                .on_press(Msg::FecharTerminal(a.termo.id))
                .padding([0, 4])
                .style(|_, s| item_lista(false, s, 3.0));
            button(
                row![text(&a.nome).size(11).font(MONO), fechar]
                    .spacing(6)
                    .align_y(Alignment::Center),
            )
            .on_press(Msg::TrocarTerminal(i))
            .padding([2, 8])
            .style(move |_, s| item_lista(ativa, s, 5.0))
            .into()
        });
        let cabecalho = container(
            row![
                text("TERMINAL")
                    .size(11)
                    .font(NEGRITO)
                    .color(COR_TEXTO_MUTED()),
                iced::widget::Row::with_children(abas)
                    .spacing(4)
                    .align_y(Alignment::Center),
                botao_icone("plus", Msg::NovoTerminal),
                horizontal_space(),
                botao_icone("x", Msg::AlternarTerminal),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .padding([0, 10])
        .height(30)
        .align_y(Alignment::Center);
        let corpo: Element<Msg> = match t.ativo() {
            Some(termo) => {
                foco_por_clique(iced_term::TerminalView::show(termo).map(Msg::Term)).into()
            }
            None => container(
                button(text("Abrir um terminal").size(12).color(COR_AZUL_ACENTO()))
                    .on_press(Msg::NovoTerminal)
                    .style(|_, s| item_lista(false, s, 5.0)),
            )
            .center(Length::Fill)
            .style(|_| fundo(cor_terminal()))
            .into(),
        };
        column![
            cabecalho,
            linha_horizontal(),
            container(corpo)
                .padding([4, 8])
                .style(|_| fundo(cor_terminal()))
        ]
        .height(ALTURA_TERMINAL)
        .into()
    }
}

/// O `TerminalView` nasce com o foco e só o perde por uma operação de foco; clicar no chat não
/// tira, e o que se digitava lá ia parar no shell também. Este invólucro decide a cada clique:
/// clique no terminal liga o foco dele, clique em qualquer outro lugar desliga.
pub struct FocoPorClique<'a, M> {
    conteudo: Element<'a, M>,
}

pub fn foco_por_clique<'a, M>(conteudo: impl Into<Element<'a, M>>) -> FocoPorClique<'a, M> {
    FocoPorClique {
        conteudo: conteudo.into(),
    }
}

/// Liga ou desliga o foco de tudo o que for focável dentro do terminal.
struct Foco(bool);

impl<T> iced::advanced::widget::Operation<T> for Foco {
    fn container(
        &mut self,
        _id: Option<&iced::advanced::widget::Id>,
        _bounds: iced::Rectangle,
        operate_on_children: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation<T>),
    ) {
        operate_on_children(self);
    }

    fn focusable(
        &mut self,
        state: &mut dyn iced::advanced::widget::operation::Focusable,
        _id: Option<&iced::advanced::widget::Id>,
    ) {
        if self.0 {
            state.focus();
        } else {
            state.unfocus();
        }
    }
}

impl<M> iced::advanced::Widget<M, iced::Theme, iced::Renderer> for FocoPorClique<'_, M> {
    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.conteudo)]
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.diff_children(std::slice::from_ref(&self.conteudo));
    }

    fn size(&self) -> iced::Size<Length> {
        self.conteudo.as_widget().size()
    }

    fn layout(
        &self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limites: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        self.conteudo
            .as_widget()
            .layout(&mut tree.children[0], renderer, limites)
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        tema: &iced::Theme,
        estilo: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        self.conteudo.as_widget().draw(
            &tree.children[0],
            renderer,
            tema,
            estilo,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate(
        &self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.conteudo
            .as_widget()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn on_event(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, M>,
        viewport: &iced::Rectangle,
    ) -> iced::event::Status {
        if let iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_)) = event {
            let dentro = cursor.is_over(layout.bounds());
            self.conteudo.as_widget().operate(
                &mut tree.children[0],
                layout,
                renderer,
                &mut Foco(dentro),
            );
        }
        self.conteudo.as_widget_mut().on_event(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        )
    }

    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &iced::Renderer,
    ) -> iced::mouse::Interaction {
        self.conteudo.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &iced::Renderer,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        self.conteudo
            .as_widget_mut()
            .overlay(&mut tree.children[0], layout, renderer, translation)
    }
}

impl<'a, M: 'a> From<FocoPorClique<'a, M>> for Element<'a, M> {
    fn from(w: FocoPorClique<'a, M>) -> Self {
        Element::new(w)
    }
}
