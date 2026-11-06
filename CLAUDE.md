# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

FeatherCode is a code editor for the FeatherWorks suite, written in Rust with `pelicano`, the suite's own UI library (from crates.io, 0.1.31; `feather-ui` v0.3.12, built on it, still pins it by git tag, so a `[patch]` at the end of `Cargo.toml` points that git source to the same crates.io version: one copy only). Claude Code runs in a side panel. Code, comments, UI text and commit messages are all in Portuguese (pt-BR). Keep it that way.

## Commands

```bash
cargo run                     # opens the start page
cargo run -- <pasta>          # opens a folder directly (arg read in Code::novo, src/main.rs)
cargo test                    # unit tests live in `mod testes` at the bottom of each module
cargo test <nome_do_teste>    # one test, e.g. cargo test edicao::
cargo clippy
```

Screenshot mode, used to check the UI without clicking through it:
`FEATHERCODE_CAPTURAR=foto.png cargo run -- <pasta>` captures the window to a PNG and closes it. This mode never writes the user config. Optional flags: `FEATHERCODE_CAPTURAR_ARQUIVO=<relative path>` opens a file, `FEATHERCODE_CAPTURAR_EXEMPLO=1` fills in a sample conversation and terminal, `FEATHERCODE_CAPTURAR_MENU=<n>` leaves top menu `n` open, `FEATHERCODE_CAPTURAR_BARRA=<termo>` opens find and replace with that term, `FEATHERCODE_CAPTURAR_DIALOGO=1` shows the close-tab question for the open file, `FEATHERCODE_CAPTURAR_PREVIA=1` shows the open Markdown file formatted, `FEATHERCODE_CAPTURAR_PROBLEMAS=1` fills the Problems tab with two samples in the open file, `FEATHERCODE_CAPTURAR_MENU_COMMIT=1` (with `FEATHERCODE_CAPTURAR_GIT=1`) leaves the Commit arrow menu open, and `FEATHERCODE_CAPTURAR_DIVIDIR=<path>` splits the editor and opens that file on the right.

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
- **`problemas.rs`**: Problems tab (next to Terminal, Rust projects only): runs `cargo check --message-format=json`, keeps the primary span of errors and warnings in the folder's own files, and turns columns into byte ranges for the editor's wavy underline (`pelicano::Warning`). It runs on the ↻ button or when the tab opens empty, not on F5: two cargos would wait on each other's target lock.
- **`config.rs`**: theme, recents and preferences (`salvar_sozinho` included), stored as JSON in the system config folder (`dirs`).

Git panel: `ler_status` splits `git status --porcelain` into Preparadas (X) and Mudanças (Y; untracked shows "U"). With something staged, Commit takes only the staged files; with nothing staged it runs `add -A` first. The arrow opens Commit e Push / Commit de tudo. Unstaging falls back to `git rm --cached` in a repository with no commit yet.

Diff: side by side by default (`paineis::lado_a_lado` pairs removed and added lines; the missing side is hatched by `hachura`), inline by choice (`config.diff_em_linha`) or when the editor card is under 500 px.

Split editor: `self.abas` holds every open file; `Aba::grupos` says which side shows it (1 left, 2 right, 3 both, same `Content`). `self.ativa` is always the active tab of the side with the keyboard (`foco`), `outra_ativa` the other side's, so code that uses `self.aba()` needs no change; `FocarGrupo` swaps them. Each side is wrapped in `ao_clicar`, a widget that reports the click before the editor takes it. Closing a tab leaves only its side (`fechar_aba`); `remover_aba` takes it from both; a side left empty joins (`juntar`). Editor ids are `("editor", side)`; focus it with `focar_editor()`.

Editor typing: Enter goes through `on_key` to `Msg::NovaLinha`, which keeps the line's indentation and adds a level after `{ ( [ :` (`edicao::quebra_com_recuo`). `edicao::detectar_recuo` reads the first 400 lines to tell tabs from 2/4/8 spaces; Tab inserts that unit and the status bar shows it (click toggles `config.mostrar_invisiveis`). Find/replace and go-to-line are a floating box at the editor's top right (`barra_view`, stacked over the editor). Saving (Ctrl+S, Save all) runs `cargo check` for the Problems tab. With the terminal closed, the bottom edge stays draggable and opens it after 40 px.

Autocomplete (`completar.rs`): after each edit (`on_change` → `Msg::Editou`) the name being typed is matched against the file's own words (most used first) and the language keywords, case-sensitive, so accepting only appends the rest. pelicano draws the list at the caret (`TextEditor::completions`); Tab or Enter accepts, arrows move, Esc closes, and it hides once the cursor moves away from where it was asked.

Messages that arrive on their own (git, diff, check, autosave tick…) are in the list at the top of `update` that does not close the open menu.

Editor safety, all in `main.rs`:
- `Aba::disco` is a hash of the text on disk at the last read or save. `recarregar_abas` (after each agent tool result and turn) reloads clean tabs that changed on disk, and marks dirty ones `conflito`: a yellow strip above the editor offers "Ver diferença" (`paineis::diff_textos`, `git diff --no-index`), "Ficar com a minha" and "Usar a do disco". Autosave skips tabs in conflict.
- Closing a dirty tab or the window asks first (`Confirmar`, the suite `feather_ui::dialogo::Dialogo`). The window runs with `close_on_request: false`, so the X arrives as `Event::CloseRequested` → `Msg::Fechar`.
- The active tab's underline is the `Sublinhado` widget, not a `stack`: in a `stack`, a `Length::FILL` layer takes the whole bar, and the first tab hid the other tabs and the Run button.
- Agent replies go through `ligar_caminhos` when they arrive: `src/x.rs:42` (file must exist) becomes a link `arquivo:src/x.rs:42` that `Msg::Link` opens at the line. "Copiar resposta" strips them (`sem_links`).

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
- [x] Diferença **lado a lado** (05/11), com faixa hachurada; a em linha fica como opção. O desenho
      aprovado dizia voltar à em linha abaixo de 900 px, mas com Git e agente abertos o editor tem
      ~560 px: o limite ficou em 500.
- [x] No Gráfico, clicar num commit abre a lista de arquivos dele; clicar no arquivo mostra a
      diferença daquele commit (`git show <hash> -- arquivo`).
- [x] Enter no campo da mensagem faz o commit; a seta ao lado abre Commit e Push e Commit de tudo (05/11).
- [x] Preparar/despreparar arquivo por arquivo (+ / −), Preparadas e Mudanças separadas (05/11).

**Editor (feito em 19/10):** Ctrl+Z/Ctrl+Y (hoje o do próprio `text_editor` do pelicano); linha do
cursor destacada; trilho no estilo do VS Code; só a pena no topo.

**Explorador**
- [x] Ícone por tipo de arquivo (05/11): o catálogo da suíte não tem ícone por linguagem, então é
      um arquivo desenhado com a cor do tipo (`paineis::cor_do_tipo`), e pasta aberta/fechada.
- [x] Botões no topo: atualizar e recolher tudo (limpa também o filtro).
- [x] Botões no topo: novo arquivo e nova pasta (05/11), com o diálogo de nome da suíte.

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
- [x] Dois arquivos lado a lado (05/11): ◫ na barra de abas, Ctrl+\ ou Exibir ▸ Dividir o editor;
      dois lados no máximo, divisa arrastável, × à direita junta.
- [x] Prévia de Markdown (botão Prévia/Editar na barra de abas); HTML abre no navegador (05/11).

**Menus — triagem dos menus do VS Code (19/10). Regra 3: só entra o que funcionar.**
- Vale fazer:
  - [x] Arquivo: Abrir arquivo (Ctrl+Shift+O), Salvar tudo (Ctrl+Shift+S).
  - [x] Atalhos: Explorador (Ctrl+Shift+E), Buscar (Ctrl+Shift+F), Git (Ctrl+Shift+G),
    focar/abrir Terminal (Ctrl+`), Comentar linha (Ctrl+/ e Ctrl+; no editor).
  - [x] (05/11) Localizar (Ctrl+F), Substituir (Ctrl+H), Ir para linha (Ctrl+G): barra acima do
    editor sobre `Content::find_next/replace_next/replace_all/go_to_line` do pelicano (sem
    "anterior": o pelicano só busca para a frente). Salvar sozinho (Arquivo e Configurações ▸ Editor).
  - Arquivo: Novo arquivo (Ctrl+N), Abrir recente ▸, Salvar como,
    Reverter arquivo, Fechar pasta, Sair (Ctrl+Q).
  - Editar: Desfazer/Refazer, Recortar/Copiar/Colar,
    Buscar/Substituir nos arquivos, Comentar linha (também Ctrl+/).
  - Seleção: Selecionar tudo, Mover linha (Alt+↑/↓), Copiar linha (Shift+Alt+↑/↓).
  - Exibir: Paleta de comandos (Ctrl+Shift+P), Explorador/Buscar/Git (Ctrl+Shift+E/F/G),
    Terminal (Ctrl+`), Quebra de linha (Alt+Z), Layout do editor (dividir).
  - Ir: Ir para arquivo (Ctrl+P, já existe), Ir para linha (Ctrl+G), Voltar/Avançar.
  - Terminal: Dividir terminal, Rodar arquivo ativo, Rodar texto selecionado.
  - [x] Problemas (05/11): erros do `cargo check` na lista, clicar leva à linha, sublinhado no
    editor, "Pedir ao agente para corrigir" põe a lista no campo do chat.
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
- [x] Explorador (05/11): Abrir, Ver alterações (se o painel Git já leu as mudanças), Copiar
      caminho / caminho relativo, Renomear, Excluir (confirma; apaga de vez, não há lixeira); na
      pasta também Novo arquivo, Nova pasta, Abrir no terminal.
- [x] Aba (05/11): Fechar, Fechar as outras, Fechar todas (as com mudança não salva ficam abertas),
      Copiar caminho.
- [ ] Mudança do Git: Abrir arquivo, Ver alterações, Descartar alterações (com confirmação).

**Lançador e cor**
- [ ] Lançador: página de primeira instalação (rodar em segundo plano?) e redesenho, no artifact.
- [ ] Cor do FeatherCode: roxo hoje; Petróleo `#2b7a78` proposto, sem decisão.
