//! Página inicial: começar (abrir pasta), recentes, aparência e projeto novo a partir de modelo.
use crate::modelos::MODELOS;
use crate::*;
// Explícito: o glob traria também o `column!` da std e ficaria ambíguo.
use iced::widget::{column, row};

impl Code {
    pub(crate) fn inicio_view(&self) -> Element<'_, Msg> {
        let marca = row![
            marca::icone_atual(52.0),
            column![
                text("FeatherCode").size(26).font(NEGRITO),
                text("Editor de código da suíte FeatherWorks, com o Claude Code junto")
                    .size(13)
                    .color(COR_TEXTO_MUTED())
            ]
            .spacing(2),
        ]
        .spacing(14)
        .align_y(Alignment::Center);

        let comecar = secao(
            "Começar",
            column![link(
                icone_traco("folder-open", 15.0),
                "Abrir pasta…",
                Some("Ctrl+O"),
                Msg::AbrirPasta
            )]
            .into(),
        );
        let pastas = secao(
            "Pastas recentes",
            recentes(
                &self.config.pastas,
                Msg::AbrirRecente,
                "Nenhuma pasta aberta ainda.",
            ),
        );
        let arquivos = secao(
            "Arquivos recentes",
            recentes(
                &self.config.arquivos,
                Msg::AbrirArquivoRecente,
                "Nenhum arquivo aberto ainda.",
            ),
        );
        let escuro = tema::escuro();
        let aparencia = secao(
            "Aparência",
            row![
                button(text("Escuro").size(13))
                    .on_press(Msg::Tema(true))
                    .padding([6, 14])
                    .style(tema::estilo_aba_faixa(escuro)),
                button(text("Claro").size(13))
                    .on_press(Msg::Tema(false))
                    .padding([6, 14])
                    .style(tema::estilo_aba_faixa(!escuro)),
            ]
            .spacing(6)
            .into(),
        );
        let esquerda = column![marca, comecar, pastas, arquivos, aparencia]
            .spacing(28)
            .width(Length::FillPortion(2));

        let cartoes: Vec<Element<Msg>> = MODELOS
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let escolhido = self.modelo == Some(i);
                let corpo = column![
                    row![
                        text(m.nome).size(14).font(NEGRITO),
                        horizontal_space(),
                        chip(m.linguagem)
                    ]
                    .align_y(Alignment::Center),
                    text(m.descricao).size(12).color(COR_TEXTO_MUTED()),
                ]
                .spacing(6);
                button(corpo)
                    .on_press(Msg::EscolherModelo(i))
                    .padding(12)
                    .width(Length::Fill)
                    .height(92)
                    .style(move |_, s| cartao(escolhido, s))
                    .into()
            })
            .collect();
        let mut grade = Column::new().spacing(10);
        let mut cartoes = cartoes.into_iter();
        while let Some(a) = cartoes.next() {
            let b: Element<Msg> = cartoes
                .next()
                .unwrap_or_else(|| Space::with_width(Length::Fill).into());
            grade = grade.push(row![a, b].spacing(10));
        }
        let mut direita = column![
            text("Novo projeto").size(16).font(NEGRITO),
            text("Escolha um modelo: os arquivos já vêm prontos para rodar, e a pasta abre no editor.").size(12).color(COR_TEXTO_MUTED()),
            grade,
        ]
        .spacing(10)
        .width(Length::FillPortion(3));
        if let Some(m) = self.modelo.and_then(|i| MODELOS.get(i)) {
            direita = direita.push(self.formulario(m));
        }

        let pagina = container(row![esquerda, direita].spacing(48)).max_width(1120);
        container(
            tema::rolagem(container(pagina).padding([40, 48]).center_x(Length::Fill))
                .style(rolagem_discreta)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| fundo(COR_PAINEL()))
        .into()
    }

    fn formulario<'a>(&'a self, m: &'a modelos::Modelo) -> Element<'a, Msg> {
        let onde = self.onde_criar();
        let previa =
            modelos::nome_de_pacote(&self.nome_projeto).map(|n| onde.join(n).display().to_string());
        let destino = match &previa {
            Ok(p) => text(format!("Vai criar {p}"))
                .size(12)
                .color(COR_TEXTO_MUTED()),
            Err(e) if !self.nome_projeto.is_empty() => text(*e).size(12).color(COR_ERRO()),
            Err(_) => text("").size(12),
        };
        let mut corpo = column![
            text(format!("{} · {}", m.nome, m.linguagem))
                .size(13)
                .font(NEGRITO),
            text_input("Nome do projeto, ex.: meu-programa", &self.nome_projeto)
                .on_input(Msg::NomeProjeto)
                .on_submit(Msg::CriarProjeto)
                .size(13)
                .padding([7, 10])
                .style(campo_com_borda),
            row![
                text(format!("Em: {}", onde.display()))
                    .size(12)
                    .color(COR_TEXTO_MUTED()),
                horizontal_space(),
                button(text("Escolher pasta…").size(12))
                    .on_press(Msg::EscolherOnde)
                    .padding([4, 10])
                    .style(tema::estilo_botao_secundario),
            ]
            .align_y(Alignment::Center),
            destino,
            text(format!("Para rodar depois: {}", m.rodar))
                .size(12)
                .font(MONO)
                .color(COR_TEXTO_MUTED()),
        ]
        .spacing(10);
        if !self.erro_projeto.is_empty() {
            corpo = corpo.push(text(&self.erro_projeto).size(12).color(COR_ERRO()));
        }
        let criar = button(text("Criar projeto").size(13))
            .padding([7, 16])
            .style(tema::estilo_botao_primario);
        corpo = corpo.push(row![
            horizontal_space(),
            if previa.is_ok() {
                criar.on_press(Msg::CriarProjeto)
            } else {
                criar
            }
        ]);
        container(corpo)
            .padding(16)
            .width(Length::Fill)
            .style(|_| container::Style {
                border: Border {
                    color: COR_BORDA(),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..fundo(COR_PAINEL_FAIXA())
            })
            .into()
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
                    .and_then(|p| p.parent())
                    .map(Path::to_path_buf)
            })
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| ".".into())
    }
}

fn secao<'a>(titulo: &'a str, corpo: Element<'a, Msg>) -> Element<'a, Msg> {
    column![
        text(titulo.to_uppercase())
            .size(11)
            .font(NEGRITO)
            .color(COR_TEXTO_MUTED()),
        corpo
    ]
    .spacing(8)
    .into()
}

fn link<'a>(
    icone: impl Into<Element<'a, Msg>>,
    rotulo: &'a str,
    atalho: Option<&'a str>,
    msg: Msg,
) -> Element<'a, Msg> {
    let mut linha = row![icone.into(), text(rotulo).size(13).color(COR_AZUL_ACENTO())]
        .spacing(8)
        .align_y(Alignment::Center);
    if let Some(a) = atalho {
        linha = linha
            .push(horizontal_space())
            .push(text(a).size(11).color(COR_TEXTO_MUTED()));
    }
    button(linha)
        .on_press(msg)
        .padding([5, 8])
        .width(Length::Fill)
        .style(|_, s| item_lista(false, s, 4.0))
        .into()
}

fn recentes<'a>(lista: &'a [PathBuf], msg: fn(PathBuf) -> Msg, vazio: &'a str) -> Element<'a, Msg> {
    if lista.is_empty() {
        return text(vazio).size(12).color(COR_TEXTO_MUTED()).into();
    }
    let itens = lista.iter().map(|p| {
        let nome = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.display().to_string());
        let pai = p
            .parent()
            .map(|x| x.display().to_string())
            .unwrap_or_default();
        button(
            row![
                text(nome).size(13).color(COR_AZUL_ACENTO()),
                text(pai).size(12).color(COR_TEXTO_MUTED())
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .on_press(msg(p.clone()))
        .padding([4, 8])
        .width(Length::Fill)
        .style(|_, s| item_lista(false, s, 4.0))
        .into()
    });
    Column::with_children(itens).spacing(2).into()
}

fn chip(t: &str) -> Element<'_, Msg> {
    container(text(t).size(11).color(COR_TEXTO_MUTED()))
        .padding([1, 7])
        .style(|_| container::Style {
            border: Border {
                color: COR_BORDA(),
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn cartao(escolhido: bool, s: button::Status) -> button::Style {
    let fundo = if matches!(s, button::Status::Hovered) {
        COR_HOVER()
    } else {
        COR_PAINEL_FAIXA()
    };
    button::Style {
        background: Some(fundo.into()),
        text_color: COR_TEXTO(),
        border: Border {
            color: if escolhido {
                COR_AZUL_ACENTO()
            } else {
                COR_BORDA_SUAVE()
            },
            width: if escolhido { 2.0 } else { 1.0 },
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

fn campo_com_borda(t: &iced::Theme, s: text_input::Status) -> text_input::Style {
    let foco = matches!(s, text_input::Status::Focused);
    text_input::Style {
        background: COR_PAINEL().into(),
        border: Border {
            color: if foco { COR_AZUL_ACENTO() } else { COR_BORDA() },
            width: 1.0,
            radius: 6.0.into(),
        },
        ..campo_sem_borda(t, s)
    }
}
