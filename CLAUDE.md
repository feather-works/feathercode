# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

FeatherCode is a code editor for the FeatherWorks suite, written in Rust with `pelicano`, the suite's own UI library (`../pelicano`, path dependency for now). Claude Code runs in a side panel. Code, comments, UI text and commit messages are all in Portuguese (pt-BR). Keep it that way.

## Commands

```bash
cargo run                     # opens the start page
cargo run -- <pasta>          # opens a folder directly (arg read in Code::novo, src/main.rs)
cargo test                    # unit tests live in `mod testes` at the bottom of each module
cargo test <nome_do_teste>    # one test, e.g. cargo test edicao::
cargo clippy
```

Screenshot mode, used to check the UI without clicking through it:
`FEATHERCODE_CAPTURAR=foto.png cargo run -- <pasta>` captures the window to a PNG and closes it. This mode never writes the user config. Optional flags: `FEATHERCODE_CAPTURAR_ARQUIVO=<relative path>` opens a file, `FEATHERCODE_CAPTURAR_EXEMPLO=1` fills in a sample conversation and terminal, and `FEATHERCODE_CAPTURAR_MENU=<n>` leaves top menu `n` open.

## Architecture

The app is a single `pelicano::App` (`pelicano::run`). `Code` in `src/main.rs` is the whole state, and `Msg` is the only message enum. `update`/`view`/`subscription` live there. The other modules hold pure logic or view pieces that `main.rs` calls. They reach back into it with `use crate::*`.

- **`agente.rs`**: talks to the installed `claude` CLI over the Agent SDK stream-json protocol, the same one the VS Code extension uses. It runs `--input-format/--output-format stream-json --permission-prompt-tool stdio --permission-mode <modo>` and exchanges one JSON per line. The stream is a pelicano `Subscription::run_with_id(("agente", self.conversa), …)`. To restart the process, bump `Code::conversa` or set `ligado` to false. Dropping the stream kills the child (`kill_on_drop`). Permission requests come in as `control_request`, and the editor answers with `permitir`/`negar`. `interrupt`/`set_permission_mode` go out through `controle`. Each user message gets the `<contexto_do_editor>` block appended (open file + cursor line). It is built in `main.rs`.
- **`terminal.rs`**: terminal tabs over `pelicano::Terminal` (PTY). `Task::focus(("terminal", id))` moves the keyboard to one.
- **`edicao.rs`**: Tab/Shift+Tab and comment on the `pelicano::Content` via `edit_lines`. Undo/redo, scroll, line numbers and highlighting belong to pelicano's `text_editor`.
- **`execucao.rs`**: runs shell commands for the terminal and the Run button (F5). Output streams line by line and stops through a `oneshot`. `comando_do_projeto` picks the command from the project type: Cargo (in a workspace, the `crates/*-gui` crate), `package.json`, `main.py`…
- **`inicio.rs`**: start page (recents, theme, detects `claude`/`codex`/`agy`, new project and clone). Its own message enum is `Ini`, wrapped in `Msg::Ini`.
- **`modelos.rs`**: project templates as file lists. `{nome}` becomes the package name and `{modulo}` the snake_case name.
- **`paineis.rs`**: left rail (Explorer, Search, Git), the search and Git panels, and the top menu.
- **`arvore.rs`**: file tree. Folders are read lazily when expanded. The ignored folders are a fixed list (`IGNORADAS`), not `.gitignore`.
- **`config.rs`**: theme, recents and preferences, stored as JSON in the system config folder (`dirs`).

Colors, theme, icons, window chrome and the suite brand come from `feather-ui`, a git dependency pinned by tag in `Cargo.toml`. Reuse its `COR_*` and widgets before you write your own style.

## Conventions

- Known limits are marked `// shortcut: <limit>, <when to upgrade>`.
- File dialogs use `rfd` with `xdg-portal` (no GTK).
- Commits are prefixed with `claude:` plus a short description in Portuguese.

## Fila de trabalho (anotada em 19/10/2026, pedidos do usuário)

Regra que vale para tudo: tela nova é desenhada no artifact primeiro
(https://claude.ai/artifact/Vg5ygNrD6ZpHo9NGmk629q) e só vira código depois de aprovada.

**Git (referência: painel Source Control do VS Code, prints de 19/10)**
- [x] Botão "Título com IA" ao lado de Commit: usa o agente selecionado para sugerir o título a partir do diff e dos nomes de arquivos novos, com revisão antes do commit.
- [x] Pull, Push, Fetch (rodam numa aba do terminal) e Commit com mensagem (`add -A` + commit).
- [x] Clicar numa mudança mostra a diferença contra o último commit (em linha, com +/−).
- [ ] Diferença **lado a lado** (antes | depois, faixa hachurada onde não há linha), como o VS Code;
      a em linha fica como opção.
- [x] No Gráfico, clicar num commit abre a lista de arquivos dele; clicar no arquivo mostra a
      diferença daquele commit (`git show <hash> -- arquivo`).
- [ ] Ctrl+Enter faz o commit; botão Commit com seta: "Commit e Push", "Commit (só o preparado)".
- [ ] Preparar/despreparar arquivo por arquivo (+ / −), como "Staged Changes".

**Editor (feito em 19/10):** Ctrl+Z/Ctrl+Y (hoje o do próprio `text_editor` do pelicano); linha do
cursor destacada; trilho no estilo do VS Code; só a pena no topo.

**Explorador**
- [ ] Ícone por tipo de arquivo (`.py`, `.rs`, `.env`, `.gitignore`, `.md`, `.txt`…).
- [x] Botões no topo: atualizar e recolher tudo (limpa também o filtro).
- [ ] Botões no topo: novo arquivo e nova pasta.

**Agentes**
- [x] Trocar o agente no rodapé do painel (seletor com os instalados). `adaptadores.rs` traduz o
      Codex (`codex app-server`, JSON-RPC; pede permissão) e o Antigravity (`agy` stream-json) para
      o protocolo do Claude Code, que é o que o painel entende. Teste com a conta de verdade:
      `FEATHERCODE_AGENTE=codex|agy cargo test --release agente_de_verdade -- --ignored --nocapture`.
- Limites do Antigravity: sem janela ele não pede permissão (recusa na hora; o painel avisa e
  sugere "Aceitar edições"); trocar de modo só vale em conversa nova; o snap
  (`antigravity-cli`, de terceiro) não enxerga `/run/media`, pasta oculta da casa nem o `/tmp`.
- [ ] Histórico de conversas do Codex (`thread/list` + `thread/resume`) e do Antigravity
      (`--conversation <id>`); hoje o botão de histórico só aparece com o Claude Code.

**Configurações**
- [x] Engrenagem no fim do trilho + Ctrl+, e menu Exibir; padrão da suíte (`feather_ui::configuracoes`). Seções: Aparência, Editor, Terminal, Agentes de IA, Projetos, Atalhos e Sobre. Implementado por pedido direto do usuário em 20/10/2026.

**Editor**
- [ ] Dois arquivos lado a lado (dividir o editor).
- [ ] Prévia de Markdown; prévia de HTML (abrir no navegador).

**Menus — triagem dos menus do VS Code (19/10). Regra 3: só entra o que funcionar.**
- Vale fazer:
  - [x] Arquivo: Abrir arquivo (Ctrl+Shift+O), Salvar tudo (Ctrl+Shift+S).
  - [x] Atalhos: Explorador (Ctrl+Shift+E), Buscar (Ctrl+Shift+F), Git (Ctrl+Shift+G),
    focar/abrir Terminal (Ctrl+`), Comentar linha (Ctrl+/ e Ctrl+; no editor).
  - Arquivo: Novo arquivo (Ctrl+N), Abrir recente ▸, Salvar como,
    Salvamento automático, Reverter arquivo, Fechar pasta, Sair (Ctrl+Q).
  - Editar: Desfazer/Refazer, Recortar/Copiar/Colar, Localizar (Ctrl+F), Substituir (Ctrl+H),
    Buscar/Substituir nos arquivos, Comentar linha (também Ctrl+/).
  - Seleção: Selecionar tudo, Mover linha (Alt+↑/↓), Copiar linha (Shift+Alt+↑/↓).
  - Exibir: Paleta de comandos (Ctrl+Shift+P), Explorador/Buscar/Git (Ctrl+Shift+E/F/G),
    Terminal (Ctrl+`), Quebra de linha (Alt+Z), Layout do editor (dividir).
  - Ir: Ir para arquivo (Ctrl+P, já existe), Ir para linha (Ctrl+G), Voltar/Avançar.
  - Terminal: Dividir terminal, Rodar arquivo ativo, Rodar texto selecionado.
  - Depois: Problemas (erros do `cargo check`/linters na lista, clicar leva à linha).
- Não vale (por ora):
  - Workspaces (adicionar pasta ao workspace, salvar/duplicar workspace), perfis, Compartilhar,
    Emmet, Extensões.
  - Multicursor (Ctrl+D, cursor acima/abaixo, seleção em coluna): o `text_editor` do pelicano
    não tem; só com editor próprio.
  - Expandir/encolher seleção: precisa da árvore de sintaxe.
  - Depurador (breakpoints, passo a passo, Debug Console): exige DAP, é projeto à parte.
  - Tarefas (`tasks.json`, Run Task, Configure Tasks): o botão Rodar já cobre.
- Confiança na pasta (Workspace Trust): a tela inteira do VS Code não faz sentido aqui (não há
  extensões nem configurações da pasta). Talvez só uma confirmação antes do Rodar numa pasta
  clonada que nunca foi rodada.

**Menu do botão direito (pedido em 19/10; hoje não existe nenhum)** — mesmo visual do menu
do topo, só com o que funciona:
- [x] Editor (21/10): Recortar, Copiar, Colar · Comentar linha · Rodar seleção no terminal ·
      Mandar seleção (ou a linha) ao agente, que vira o chip `arquivo 300:500` no chat.
      Falta Selecionar tudo: o `Content` do pelicano não tem como selecionar pelo app (Ctrl+A funciona).
- [ ] Explorador: Abrir, Ver alterações (se mudou), Copiar caminho / caminho relativo,
      Renomear, Excluir (com confirmação); na pasta também Novo arquivo, Nova pasta, Abrir no terminal.
- [ ] Aba: Fechar, Fechar as outras, Fechar todas, Copiar caminho.
- [ ] Mudança do Git: Abrir arquivo, Ver alterações, Descartar alterações (com confirmação).

**Lançador e cor**
- [ ] Lançador: página de primeira instalação (rodar em segundo plano?) e redesenho, no artifact.
- [ ] Cor do FeatherCode: roxo hoje; Petróleo `#2b7a78` proposto, sem decisão.
