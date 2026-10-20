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
        .align_y(Alignment::Center)
        .style(|_| fundo(COR_CROMO_2()));
        let corpo: Element<Msg> = match t.ativo() {
            Some(termo) => iced_term::TerminalView::show(termo).map(Msg::Term),
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
