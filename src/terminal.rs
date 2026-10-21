//! Painel de terminais: shell de verdade num PTY (o terminal do pelicano, motor do Alacritty), em abas.
use crate::*;
use pelicano::futures::StreamExt;
use pelicano::{column, row, TerminalEvent, TerminalSettings};

pub struct Aba {
    pub termo: pelicano::Terminal,
    pub nome: String,
}

#[derive(Default)]
pub struct Terminais {
    pub abas: Vec<Aba>,
    pub ativa: usize,
    /// Id do terminal aberto pelo botão Rodar; rodar de novo fecha o anterior.
    rodar: Option<u64>,
    /// Id do terminal do último pull/push/fetch, trocado a cada comando.
    git: Option<u64>,
}

fn shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into())
}

/// Nome do `.id()` do terminal, para o `Task::focus`.
fn id_widget(id: u64) -> (&'static str, u64) {
    ("terminal", id)
}

impl Terminais {
    /// Abre um shell na pasta. Com `comando`, um `sh` roda ele antes e depois vira o shell.
    pub fn novo(&mut self, pasta: &Path, comando: Option<&str>) -> Result<u64, String> {
        let (program, args) = match comando {
            Some(c) => (
                Some("/bin/sh".to_string()),
                vec![
                    "-c".into(),
                    "eval \"$1\"; printf '\\n[fim: código %s]\\n' \"$?\"; exec \"$2\"".into(),
                    "sh".into(),
                    c.to_string(),
                    shell(),
                ],
            ),
            None => (None, Vec::new()),
        };
        let termo = pelicano::Terminal::new(TerminalSettings {
            program,
            args,
            cwd: Some(pasta.to_path_buf()),
            ..Default::default()
        })
        .map_err(|e| format!("Não deu para abrir o terminal: {e}"))?;
        let nome = comando.map_or_else(
            || {
                std::path::Path::new(&shell())
                    .file_name()
                    .map_or("shell".into(), |n| n.to_string_lossy().into_owned())
            },
            |c| c.split_whitespace().take(2).collect::<Vec<_>>().join(" "),
        );
        let id = termo.id();
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
        self.abas.retain(|a| a.termo.id() != id);
        self.ativa = self.ativa.min(self.abas.len().saturating_sub(1));
    }

    pub fn ativo(&self) -> Option<&pelicano::Terminal> {
        self.abas.get(self.ativa).map(|a| &a.termo)
    }

    /// Leva o teclado ao terminal ativo.
    pub fn focar(&self) -> Task<Msg> {
        self.ativo()
            .map_or_else(Task::none, |t| Task::focus(id_widget(t.id())))
    }

    /// Evento do PTY: a saída nova só pede um quadro; o shell que sai fecha a aba.
    pub fn tratar(&mut self, id: u64, ev: TerminalEvent) {
        if let TerminalEvent::Exit(_) = ev {
            self.fechar(id);
        }
    }

    pub fn subscription(&self) -> Subscription<Msg> {
        Subscription::batch(self.abas.iter().map(|a| {
            let id = a.termo.id();
            Subscription::run_with_id(
                ("terminal", id),
                a.termo.events().map(move |e| Msg::Term(id, e)),
            )
        }))
    }
}

impl Code {
    pub(crate) fn terminal_view(&self) -> Element<'_, Msg> {
        let t = &self.terminais;
        let abas = t.abas.iter().enumerate().map(|(i, a)| {
            let ativa = i == t.ativa;
            let fechar = button(text("×").size(12).color(COR_TEXTO_MUTED()))
                .on_press(Msg::FecharTerminal(a.termo.id()))
                .padding([0, 4])
                .style(|_, s| item_lista(false, s, 3.0));
            button(
                row![text(a.nome.as_str()).size(11).font(MONO), fechar]
                    .spacing(6)
                    .align(Align::Center),
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
                row(abas).spacing(4).align(Align::Center),
                botao_icone("plus", Msg::NovoTerminal),
                horizontal_space(),
                botao_icone("x", Msg::AlternarTerminal),
            ]
            .spacing(10)
            .align(Align::Center),
        )
        .padding([0, 10])
        .width(Length::FILL)
        .height(30.0)
        .align_y(Align::Center);
        let corpo: Element<Msg> = match t.ativo() {
            Some(termo) => pelicano::terminal(termo)
                .id(id_widget(termo.id()))
                .size(13.0)
                .font(MONO)
                .text_color(COR_TEXTO())
                .background(cor_terminal())
                .padding(4.0)
                .into(),
            None => container(
                button(text("Abrir um terminal").size(12).color(COR_AZUL_ACENTO()))
                    .on_press(Msg::NovoTerminal)
                    .style(|_, s| item_lista(false, s, 5.0)),
            )
            .center()
            .width(Length::FILL)
            .height(Length::FILL)
            .style(|_| fundo(cor_terminal()))
            .into(),
        };
        column![
            cabecalho,
            linha_horizontal(),
            container(corpo)
                .padding([4, 8])
                .width(Length::FILL)
                .height(Length::FILL)
                .style(|_| fundo(cor_terminal()))
        ]
        .height(self.altura_terminal)
        .into()
    }
}
