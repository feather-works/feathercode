# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

FeatherCode is a code editor for the FeatherWorks suite, written in Rust with `iced` 0.13. Claude Code runs in a side panel. Code, comments, UI text and commit messages are all in Portuguese (pt-BR). Keep it that way.

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

The app is a single iced `application`. `Code` in `src/main.rs` is the whole state, and `Msg` is the only message enum. `update`/`view`/`subscription` live there. The other modules hold pure logic or view pieces that `main.rs` calls. They reach back into it with `use crate::*`.

- **`agente.rs`**: talks to the installed `claude` CLI over the Agent SDK stream-json protocol, the same one the VS Code extension uses. It runs `--input-format/--output-format stream-json --permission-prompt-tool stdio --permission-mode <modo>` and exchanges one JSON per line. The stream is an iced `Subscription::run_with_id(("agente", self.conversa), …)`. To restart the process, bump `Code::conversa` or set `ligado` to false. Dropping the stream kills the child (`kill_on_drop`). Permission requests come in as `control_request`, and the editor answers with `permitir`/`negar`. `interrupt`/`set_permission_mode` go out through `controle`. Each user message gets the `<contexto_do_editor>` block appended (open file + cursor line). It is built in `main.rs`.
- **`execucao.rs`**: runs shell commands for the terminal and the Run button (F5). Output streams line by line and stops through a `oneshot`. `comando_do_projeto` picks the command from the project type: Cargo (in a workspace, the `crates/*-gui` crate), `package.json`, `main.py`…
- **`inicio.rs`**: start page (recents, theme, detects `claude`/`codex`/`agy`, new project and clone). Its own message enum is `Ini`, wrapped in `Msg::Ini`.
- **`modelos.rs`**: project templates as file lists. `{nome}` becomes the package name and `{modulo}` the snake_case name.
- **`paineis.rs`**: left rail (Explorer, Search, Git), the search and Git panels, and the top menu.
- **`arvore.rs`**: file tree. Folders are read lazily when expanded. The ignored folders are a fixed list (`IGNORADAS`), not `.gitignore`.
- **`edicao.rs`**: Tab/Shift+Tab on the selection and Ctrl+; to comment, by file extension.
- **`config.rs`**: theme, recents and preferences, stored as JSON in the system config folder (`dirs`).

Colors, theme, icons, window chrome and the suite brand come from `feather-ui`, a git dependency pinned by tag in `Cargo.toml`. Reuse its `COR_*` and widgets before you write your own style.

## Conventions

- Known limits are marked `// shortcut: <limit>, <when to upgrade>`.
- File dialogs use `rfd` with `xdg-portal` (no GTK).
- Commits are prefixed with `claude:` plus a short description in Portuguese.
