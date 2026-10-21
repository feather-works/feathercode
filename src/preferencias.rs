//! Configurações com os mesmos componentes visuais da suíte.
use crate::config::Tema;
use crate::*;
use feather_ui::configuracoes::{linha, menu, tela};
use iced::widget::{checkbox, column, row};

const SECOES: [&str; 7] = [
    "Aparência",
    "Editor",
    "Terminal",
    "Agentes de IA",
    "Projetos",
    "Atalhos",
    "Sobre",
];

impl Code {
    pub(crate) fn preferencias_view(&self, secao: usize) -> Element<'_, Msg> {
        let secao = secao.min(SECOES.len() - 1);
        let navegacao = menu(
            "Configurações",
            SECOES
                .iter()
                .enumerate()
                .map(|(i, nome)| (nome.to_string(), i == secao, Msg::Configuracoes(Some(i)))),
        );
        let corpo: Element<Msg> = match secao {
            0 => {
                let temas = row([
                    ("Escuro", Tema::Escuro),
                    ("Claro", Tema::Claro),
                    ("Sistema", Tema::Sistema),
                ]
                .into_iter()
                .map(|(nome, tema)| {
                    button(text(nome).size(12))
                        .on_press(Msg::Ini(inicio::Ini::Tema(tema)))
                        .padding([6, 10])
                        .style(move |_, s| item_lista(self.config.tema == tema, s, 5.0))
                        .into()
                }))
                .spacing(6);
                column![
                    linha("Tema da janela", "Escolha as cores da interface.", temas),
                    text("As preferências de aparência são salvas automaticamente.")
                        .size(12)
                        .color(COR_TEXTO_MUTED()),
                ]
                .spacing(12)
                .into()
            }
            1 => {
                let mut cores = Column::new().spacing(4);
                for (i, tema) in highlighter::Theme::ALL.iter().enumerate() {
                    cores = cores.push(
                        button(text(paineis::nome_tema(*tema)).size(12))
                            .on_press(Msg::TemaCodigo(i))
                            .padding([5, 10])
                            .style(move |_, s| item_lista(self.tema_codigo() == *tema, s, 5.0)),
                    );
                }
                column![
                    linha(
                        "Linhas alternadas",
                        "Alterne tons suaves no fundo das linhas do código.",
                        checkbox("Ativar", self.config.linhas_alternadas)
                            .on_toggle(|_| Msg::AlternarLinhas)
                    ),
                    linha(
                        "Cores do código",
                        "Escolha o tema de destaque da sintaxe.",
                        cores
                    ),
                ]
                .into()
            }
            2 => column![
                linha(
                    "Painel do terminal",
                    "Mostrar o terminal na área de trabalho.",
                    checkbox("Mostrar", self.terminal_aberto).on_toggle(|_| Msg::AlternarTerminal)
                ),
                text("Ctrl+` abre ou foca o terminal. Ctrl+J mostra ou oculta o painel.")
                    .size(13)
                    .color(COR_TEXTO_MUTED()),
            ]
            .spacing(12)
            .into(),
            3 => column![
                linha(
                    "Painel do agente",
                    "Mostrar o assistente ao lado do código.",
                    checkbox("Mostrar", self.agente_aberto).on_toggle(|_| Msg::AlternarAgente)
                ),
                linha(
                    "Agente da conversa",
                    "Trocar de agente inicia uma conversa nova.",
                    pick_list(
                        self.agentes_instalados(),
                        Some(self.chat.provedor),
                        Msg::Provedor
                    )
                    .text_size(12)
                ),
                linha(
                    "Permissões",
                    "Escolha como o agente pode atuar na conversa atual.",
                    pick_list(Modo::TODOS, Some(self.chat.modo), Msg::Modo).text_size(12)
                ),
                text(self.explicacao_agente())
                    .size(12)
                    .color(COR_TEXTO_MUTED()),
            ]
            .spacing(8)
            .into(),
            4 => column![
                linha(
                    "Git em novos projetos",
                    "Inicializar um repositório ao criar um projeto.",
                    checkbox("Ativar", self.config.iniciar_git)
                        .on_toggle(|v| Msg::Ini(inicio::Ini::Git(v)))
                ),
                linha(
                    "Pasta dos projetos",
                    &self.onde_criar().display().to_string(),
                    button(text("Escolher pasta…").size(12))
                        .on_press(Msg::Ini(inicio::Ini::EscolherOnde))
                ),
            ]
            .into(),
            5 => {
                let mut lista = Column::new();
                for (acao, tecla) in [
                    ("Configurações", "Ctrl+,"),
                    ("Abrir pasta", "Ctrl+O"),
                    ("Abrir arquivo", "Ctrl+Shift+O"),
                    ("Salvar", "Ctrl+S"),
                    ("Salvar tudo", "Ctrl+Shift+S"),
                    ("Procurar arquivo", "Ctrl+P"),
                    ("Explorador", "Ctrl+Shift+E"),
                    ("Buscar nos arquivos", "Ctrl+Shift+F"),
                    ("Git", "Ctrl+Shift+G"),
                    ("Focar terminal", "Ctrl+`"),
                    ("Mostrar/ocultar terminal", "Ctrl+J"),
                    ("Rodar projeto", "F5"),
                    ("Comentar linha", "Ctrl+/ ou Ctrl+;"),
                    ("Desfazer", "Ctrl+Z"),
                    ("Refazer", "Ctrl+Y ou Ctrl+Shift+Z"),
                    ("Fechar configurações", "Esc"),
                ] {
                    lista = lista.push(linha(acao, "", text(tecla).size(12).font(MONO)));
                }
                lista.into()
            }
            _ => column![
                text("FeatherCode").size(26).font(NEGRITO),
                text(format!("Versão {}", env!("CARGO_PKG_VERSION"))).size(14),
                text("Editor de código da suíte FeatherWorks.").size(13),
                text("Licença GPL-3.0-only")
                    .size(12)
                    .color(COR_TEXTO_MUTED()),
            ]
            .spacing(12)
            .into(),
        };
        tela(
            navegacao,
            SECOES[secao],
            corpo,
            Some(("Fechar", Msg::Configuracoes(None))),
        )
    }
}
