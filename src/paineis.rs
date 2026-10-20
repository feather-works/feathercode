//! Trilho da esquerda (Explorador, Buscar, Git, Agente), os painéis de busca e de Git e o menu do topo.
use crate::*;
use iced::widget::{column, row};

pub const LARGURA_TRILHO: f32 = 44.0;
const MAX_ACHADOS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lado {
    Explorador,
    Buscar,
    Git,
}

#[derive(Debug, Clone)]
pub struct Achado {
    pub caminho: PathBuf,
    /// Começa em 0.
    pub linha: usize,
    pub texto: String,
}

/// Procura `termo` (sem diferenciar maiúsculas) nos arquivos de texto da pasta.
pub fn buscar(raiz: &Path, termo: &str) -> Vec<Achado> {
    let termo = termo.to_lowercase();
    let mut achados = Vec::new();
    let mut pilha = vec![raiz.to_path_buf()];
    while let Some(pasta) = pilha.pop() {
        let Ok(itens) = std::fs::read_dir(&pasta) else {
            continue;
        };
        let mut itens: Vec<_> = itens.flatten().map(|e| e.path()).collect();
        itens.sort();
        for p in itens.into_iter().rev() {
            let nome = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if arvore::ignorada(&nome) {
                continue;
            }
            if p.is_dir() {
                pilha.push(p);
                continue;
            }
            // Arquivo grande ou binário não entra.
            if p.metadata().map_or(true, |m| m.len() > 2_000_000) {
                continue;
            }
            let Ok(texto) = std::fs::read_to_string(&p) else {
                continue;
            };
            for (i, l) in texto.lines().enumerate() {
                if l.to_lowercase().contains(&termo) {
                    achados.push(Achado {
                        caminho: p.clone(),
                        linha: i,
                        texto: l.trim().chars().take(160).collect(),
                    });
                    if achados.len() >= MAX_ACHADOS {
                        return achados;
                    }
                }
            }
        }
    }
    achados
}

/// `git status --porcelain`: (sigla, caminho).
pub async fn status_git(raiz: PathBuf) -> Result<Vec<(String, PathBuf)>, String> {
    let saida = tokio::process::Command::new("git")
        .args(["status", "--porcelain=v1", "-uall"])
        .current_dir(&raiz)
        .output()
        .await
        .map_err(|e| format!("Não deu para rodar o git: {e}"))?;
    if !saida.status.success() {
        return Err(String::from_utf8_lossy(&saida.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&saida.stdout)
        .lines()
        .filter(|l| l.len() > 3)
        .map(|l| {
            let caminho = l[3..]
                .rsplit(" -> ")
                .next()
                .unwrap_or(&l[3..])
                .trim_matches('"');
            (l[..2].trim().to_string(), raiz.join(caminho))
        })
        .collect())
}

/// Ícone de ramo do Git (Tabler, MIT); o catálogo da feather-ui não tem.
fn icone_git(tamanho: f32) -> iced::widget::Svg<'static> {
    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#808080" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="7" cy="18" r="2"/><circle cx="7" cy="6" r="2"/><circle cx="17" cy="6" r="2"/><path d="M7 8v8"/><path d="M9 18h6a2 2 0 0 0 2-2v-5"/><path d="M14 14l3-3 3 3"/></svg>"##;
    iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG.as_bytes()))
        .width(tamanho)
        .height(tamanho)
        .style(|_, _| iced::widget::svg::Style {
            color: Some(COR_TEXTO()),
        })
}

/// Itens de cada menu do topo: (rótulo, atalho, mensagem, marcado).
type Item = (&'static str, &'static str, Msg, bool);

pub const MENUS: [(&str, f32); 4] = [
    ("Arquivo", 62.0),
    ("Exibir", 54.0),
    ("Terminal", 66.0),
    ("Agente", 58.0),
];
/// Largura do bloco pena + "FeatherCode" antes dos menus.
pub const LARGURA_MARCA: f32 = 122.0;

impl Code {
    pub(crate) fn trilho_view(&self) -> Element<'_, Msg> {
        let botao = |icone: Element<'static, Msg>,
                     dica: &'static str,
                     ativo: bool,
                     msg: Msg|
         -> Element<'static, Msg> {
            let b = button(container(icone).center(28))
                .on_press(msg)
                .padding(4)
                .style(move |_, s| button::Style {
                    background: Some(
                        (if ativo {
                            COR_AZUL_FUNDO()
                        } else if matches!(s, button::Status::Hovered) {
                            COR_HOVER()
                        } else {
                            Color::TRANSPARENT
                        })
                        .into(),
                    ),
                    border: Border {
                        radius: 7.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                });
            tooltip(
                b,
                container(text(dica).size(12))
                    .padding([4, 8])
                    .style(|_| container::Style {
                        border: Border {
                            color: COR_BORDA(),
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..fundo(COR_PAINEL())
                    }),
                tooltip::Position::Right,
            )
            .into()
        };
        let lado = self.lado;
        column![
            botao(
                icone_traco("file-code", 18.0).into(),
                "Explorador  Ctrl+B",
                lado == Some(Lado::Explorador),
                Msg::Lado(Lado::Explorador)
            ),
            botao(
                icone_traco("search", 18.0).into(),
                "Buscar nos arquivos  Ctrl+Shift+F",
                lado == Some(Lado::Buscar),
                Msg::Lado(Lado::Buscar)
            ),
            botao(
                icone_git(18.0).into(),
                "Git",
                lado == Some(Lado::Git),
                Msg::Lado(Lado::Git)
            ),
            botao(
                icone_traco("sparkles", 18.0).into(),
                "Painel do agente",
                self.agente_aberto,
                Msg::AlternarAgente
            ),
        ]
        .spacing(6)
        .padding([10, 6])
        .width(LARGURA_TRILHO)
        .height(Length::Fill)
        .into()
    }

    /// O painel ao lado do trilho, conforme o botão escolhido.
    pub(crate) fn lado_view(&self) -> Option<Element<'_, Msg>> {
        let corpo = match self.lado? {
            Lado::Explorador => return Some(self.arvore_view()),
            Lado::Buscar => self.busca_view(),
            Lado::Git => self.git_view(),
        };
        Some(
            container(corpo)
                .width(LARGURA_ARVORE)
                .height(Length::Fill)
                .style(|_| fundo(COR_PAINEL_FAIXA()))
                .into(),
        )
    }

    fn busca_view(&self) -> Element<'_, Msg> {
        let campo = text_input("Buscar nos arquivos", &self.busca)
            .id(text_input::Id::new("busca"))
            .on_input(Msg::Busca)
            .on_submit(Msg::BuscarAgora)
            .size(13)
            .padding([6, 8])
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
        let resumo = if self.buscando {
            "Procurando…".to_string()
        } else if let Some(a) = &self.achados {
            match a.len() {
                0 => "Nada encontrado.".into(),
                n if n >= MAX_ACHADOS => {
                    format!("Mais de {MAX_ACHADOS} resultados; mostrando os primeiros.")
                }
                n => format!("{n} resultado(s)"),
            }
        } else {
            "Enter para buscar.".into()
        };
        let mut lista = Column::new();
        let mut ultimo: Option<&Path> = None;
        for a in self.achados.iter().flatten() {
            if ultimo != Some(a.caminho.as_path()) {
                ultimo = Some(a.caminho.as_path());
                lista = lista.push(
                    container(
                        text(self.relativo(&a.caminho).display().to_string())
                            .size(12)
                            .font(NEGRITO),
                    )
                    .padding(iced::Padding {
                        top: 8.0,
                        right: 12.0,
                        bottom: 2.0,
                        left: 12.0,
                    }),
                );
            }
            let linha = row![
                text(format!("{}", a.linha + 1))
                    .font(MONO)
                    .size(11)
                    .color(COR_TEXTO_MUTED())
                    .width(34),
                text(&a.texto).font(MONO).size(11).wrapping(Wrapping::None)
            ]
            .spacing(4);
            lista = lista.push(
                button(linha)
                    .on_press(Msg::IrPara(a.caminho.clone(), a.linha))
                    .width(Length::Fill)
                    .padding([2, 12])
                    .style(|_, s| item_lista(false, s, 0.0)),
            );
        }
        column![
            container(
                column![
                    titulo_lado("BUSCAR"),
                    campo,
                    text(resumo).size(11).color(COR_TEXTO_MUTED())
                ]
                .spacing(8)
            )
            .padding([10, 12]),
            tema::rolagem(lista)
                .height(Length::Fill)
                .style(rolagem_discreta)
        ]
        .into()
    }

    fn git_view(&self) -> Element<'_, Msg> {
        let topo = row![
            titulo_lado("GIT"),
            horizontal_space(),
            botao_icone("refresh", Msg::GitAtualizar)
        ]
        .align_y(Alignment::Center);
        let ramo = text(
            self.ramo
                .as_deref()
                .map_or("Sem repositório Git".into(), |r| format!("Ramo {r}")),
        )
        .size(12)
        .color(COR_TEXTO_MUTED());
        let corpo: Element<Msg> = match &self.git {
            None => text("Lendo…").size(12).color(COR_TEXTO_MUTED()).into(),
            Some(Err(e)) => text(e).size(12).color(COR_ERRO()).into(),
            Some(Ok(v)) if v.is_empty() => text("Nada mudou desde o último commit.")
                .size(12)
                .color(COR_TEXTO_MUTED())
                .into(),
            Some(Ok(v)) => {
                let linhas = v.iter().map(|(sigla, p)| {
                    let cor = match sigla.as_str() {
                        "M" | "MM" | "AM" => AMARELO,
                        "D" => COR_ERRO(),
                        _ => VERDE,
                    };
                    let nome = self.relativo(p).display().to_string();
                    button(
                        row![
                            text(nome).size(12).width(Length::Fill),
                            text(sigla.clone()).font(MONO).size(11).color(cor)
                        ]
                        .spacing(6),
                    )
                    .on_press(Msg::AbrirArquivo(p.clone()))
                    .width(Length::Fill)
                    .padding([3, 12])
                    .style(|_, s| item_lista(false, s, 0.0))
                    .into()
                });
                column![
                    container(
                        text(format!("Mudanças ({})", v.len()))
                            .size(11)
                            .font(NEGRITO)
                            .color(COR_TEXTO_MUTED())
                    )
                    .padding([4, 12]),
                    tema::rolagem(Column::with_children(linhas))
                        .height(Length::Fill)
                        .style(rolagem_discreta)
                ]
                .into()
            }
        };
        column![
            container(column![topo, ramo].spacing(6)).padding([10, 12]),
            corpo
        ]
        .into()
    }

    fn itens_menu(&self, i: usize) -> Vec<Item> {
        match i {
            0 => vec![
                ("Abrir pasta…", "Ctrl+O", Msg::AbrirPasta, false),
                ("Salvar", "Ctrl+S", Msg::Salvar, false),
                ("Fechar aba", "", Msg::FecharAba(self.ativa), false),
                ("Início", "", Msg::IrInicio, false),
            ],
            1 => {
                let mut v = vec![
                    (
                        "Explorador",
                        "Ctrl+B",
                        Msg::Lado(Lado::Explorador),
                        self.lado == Some(Lado::Explorador),
                    ),
                    (
                        "Buscar nos arquivos",
                        "Ctrl+Shift+F",
                        Msg::Lado(Lado::Buscar),
                        self.lado == Some(Lado::Buscar),
                    ),
                    (
                        "Git",
                        "",
                        Msg::Lado(Lado::Git),
                        self.lado == Some(Lado::Git),
                    ),
                    (
                        "Terminal",
                        "Ctrl+J",
                        Msg::AlternarTerminal,
                        self.terminal_aberto,
                    ),
                    (
                        "Painel do agente",
                        "",
                        Msg::AlternarAgente,
                        self.agente_aberto,
                    ),
                ];
                for (n, t) in highlighter::Theme::ALL.iter().enumerate() {
                    v.push((
                        nome_tema(*t),
                        if n == 0 { "Cores do código" } else { "" },
                        Msg::TemaCodigo(n),
                        self.tema_codigo() == *t,
                    ));
                }
                v
            }
            2 => vec![
                if self.rodando {
                    ("Parar", "", Msg::PararComando, false)
                } else {
                    ("Rodar projeto", "F5", Msg::RodarProjeto, false)
                },
                ("Ir para o terminal", "", Msg::FocarTerminal, false),
                ("Limpar saída", "", Msg::LimparTerminal, false),
            ],
            _ => {
                let mut v = vec![("Nova conversa", "", Msg::NovaConversa, false)];
                if self.ocupado {
                    v.push(("Parar resposta", "Esc", Msg::Parar, false));
                }
                v
            }
        }
    }

    pub(crate) fn menus_view(&self) -> Element<'_, Msg> {
        let botoes = MENUS.iter().enumerate().map(|(i, (nome, largura))| {
            let aberto = self.menu == Some(i);
            button(
                text(*nome)
                    .size(12)
                    .color(if aberto {
                        COR_TEXTO()
                    } else {
                        COR_TEXTO_MUTED()
                    })
                    .center(),
            )
            .on_press(Msg::Menu(if aberto { None } else { Some(i) }))
            .width(*largura)
            .padding([4, 0])
            .style(move |_, s| button::Style {
                background: (aberto || matches!(s, button::Status::Hovered))
                    .then(|| COR_HOVER().into()),
                border: Border {
                    radius: 5.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
        });
        iced::widget::Row::with_children(botoes).into()
    }

    /// Camada por cima da janela com o menu aberto; clicar fora fecha.
    pub(crate) fn menu_aberto_view(&self) -> Option<Element<'_, Msg>> {
        let i = self.menu?;
        let x = 12.0 + LARGURA_MARCA + MENUS[..i].iter().map(|m| m.1).sum::<f32>();
        let mut lista = Column::new().spacing(1);
        for (rotulo, atalho, msg, marcado) in self.itens_menu(i) {
            // "Cores do código" abre a seção dos temas.
            if atalho == "Cores do código" {
                lista = lista.push(linha_horizontal()).push(
                    container(text("Cores do código").size(11).color(COR_TEXTO_MUTED()))
                        .padding([4, 10]),
                );
            }
            let marca: Element<Msg> = if marcado {
                icone_traco("check", 13.0).into()
            } else {
                Space::with_width(13).into()
            };
            let atalho_txt = if atalho == "Cores do código" {
                ""
            } else {
                atalho
            };
            lista = lista.push(
                button(
                    row![
                        marca,
                        text(rotulo).size(12).width(Length::Fill),
                        text(atalho_txt).size(11).color(COR_TEXTO_MUTED())
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .on_press(msg)
                .width(Length::Fill)
                .padding([5, 10])
                .style(|_, s| item_lista(false, s, 5.0)),
            );
        }
        let caixa = container(lista)
            .width(250)
            .padding(4)
            .style(|_| container::Style {
                border: Border {
                    color: COR_BORDA(),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                    offset: iced::Vector::new(0.0, 8.0),
                    blur_radius: 24.0,
                },
                ..fundo(COR_PAINEL())
            });
        let fundo_clique =
            mouse_area(container(Space::new(Length::Fill, Length::Fill))).on_press(Msg::Menu(None));
        Some(
            stack![
                fundo_clique,
                container(caixa).padding(iced::Padding {
                    top: cromo::BARRA_ALTURA,
                    left: x,
                    right: 0.0,
                    bottom: 0.0
                })
            ]
            .into(),
        )
    }
}

fn titulo_lado(t: &str) -> Element<'_, Msg> {
    text(t)
        .size(11)
        .font(NEGRITO)
        .color(COR_TEXTO_MUTED())
        .into()
}

pub fn nome_tema(t: highlighter::Theme) -> &'static str {
    match t {
        highlighter::Theme::SolarizedDark => "Solarized escuro",
        highlighter::Theme::Base16Mocha => "Mocha",
        highlighter::Theme::Base16Ocean => "Oceano",
        highlighter::Theme::Base16Eighties => "Anos 80",
        highlighter::Theme::InspiredGitHub => "GitHub claro",
    }
}

#[cfg(test)]
mod testes {
    #[test]
    fn busca_acha_linha_e_pula_pasta_ignorada() {
        let raiz = std::env::temp_dir().join(format!("feathercode-busca-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(raiz.join("target")).unwrap();
        std::fs::write(raiz.join("a.rs"), "fn main() {}\nlet Margens = 1;\n").unwrap();
        std::fs::write(raiz.join("target/b.rs"), "margens").unwrap();
        let v = super::buscar(&raiz, "margens");
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].linha, v[0].texto.as_str()), (1, "let Margens = 1;"));
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
