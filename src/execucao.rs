//! Comandos do terminal e do botão Rodar: a saída chega linha a linha e dá para parar no meio.
use futures::StreamExt;
use iced::futures;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::oneshot;

#[derive(Debug, Clone)]
pub enum Saida {
    Linha(String),
    /// Texto final: código de saída ou motivo.
    Fim(String),
}

/// Roda `cmd` no shell, na pasta; `parar` mata o processo.
pub fn executar(
    cmd: String,
    pasta: PathBuf,
    parar: oneshot::Receiver<()>,
) -> impl futures::Stream<Item = Saida> {
    iced::stream::channel(
        64,
        move |mut tx: futures::channel::mpsc::Sender<Saida>| async move {
            use futures::SinkExt;
            #[cfg(windows)]
            let mut c = tokio::process::Command::new("cmd");
            #[cfg(windows)]
            c.arg("/C");
            #[cfg(not(windows))]
            let mut c = tokio::process::Command::new("sh");
            #[cfg(not(windows))]
            c.arg("-c");
            c.arg(&cmd)
                .current_dir(pasta)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            let mut filho = match c.spawn() {
                Ok(f) => f,
                Err(e) => {
                    let _ = tx
                        .send(Saida::Fim(format!("Não deu para rodar: {e}")))
                        .await;
                    return;
                }
            };
            let saida = BufReader::new(filho.stdout.take().expect("stdout pedido")).lines();
            let erro = BufReader::new(filho.stderr.take().expect("stderr pedido")).lines();
            let linhas =
                futures::stream::select(tokio_stream_linhas(saida), tokio_stream_linhas(erro));
            futures::pin_mut!(linhas);
            let mut parar = parar;
            loop {
                tokio::select! {
                    l = linhas.next() => match l {
                        Some(l) => { let _ = tx.send(Saida::Linha(l)).await; }
                        None => break,
                    },
                    _ = &mut parar => {
                        let _ = filho.kill().await;
                        let _ = tx.send(Saida::Fim("[parado]".into())).await;
                        return;
                    }
                }
            }
            let fim = match filho.wait().await {
                Ok(s) if s.success() => String::new(),
                Ok(s) => format!("[{s}]"),
                Err(e) => format!("[{e}]"),
            };
            let _ = tx.send(Saida::Fim(fim)).await;
        },
    )
}

fn tokio_stream_linhas<R: tokio::io::AsyncBufRead + Unpin + Send + 'static>(
    linhas: tokio::io::Lines<R>,
) -> impl futures::Stream<Item = String> {
    futures::stream::unfold(linhas, |mut l| async move {
        l.next_line().await.ok().flatten().map(|t| (t, l))
    })
}

/// Comando que roda o projeto da pasta, pelo que existe nela; o arquivo aberto decide quando a pasta não diz.
pub fn comando_do_projeto(raiz: &Path, arquivo: Option<&Path>) -> Option<String> {
    if let Ok(cargo) = std::fs::read_to_string(raiz.join("Cargo.toml")) {
        if cargo.contains("[workspace]") {
            // Workspace da suíte: roda o crate da janela.
            let gui = std::fs::read_dir(raiz.join("crates"))
                .ok()?
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .find(|n| n.ends_with("-gui"));
            return Some(gui.map_or("cargo run".into(), |g| format!("cargo run -p {g}")));
        }
        return Some("cargo run".into());
    }
    if raiz.join("package.json").is_file() {
        return Some("npm start".into());
    }
    for py in ["main.py", "app.py", "src/main.py"] {
        if raiz.join(py).is_file() {
            return Some(format!("python3 {py}"));
        }
    }
    let arquivo = arquivo?;
    let relativo = arquivo
        .strip_prefix(raiz)
        .unwrap_or(arquivo)
        .display()
        .to_string();
    match arquivo.extension()?.to_str()? {
        "py" => Some(format!("python3 \"{relativo}\"")),
        "sh" => Some(format!("sh \"{relativo}\"")),
        "js" => Some(format!("node \"{relativo}\"")),
        "html" => Some(format!("xdg-open \"{relativo}\"")),
        _ => None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn descobre_o_comando_pela_pasta() {
        let raiz = std::env::temp_dir().join(format!("feathercode-rodar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(raiz.join("crates/app-gui")).unwrap();
        assert_eq!(
            comando_do_projeto(&raiz, Some(&raiz.join("x.py"))).as_deref(),
            Some("python3 \"x.py\"")
        );
        assert_eq!(comando_do_projeto(&raiz, None), None);
        std::fs::write(
            raiz.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        assert_eq!(
            comando_do_projeto(&raiz, None).as_deref(),
            Some("cargo run -p app-gui")
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[tokio::test]
    async fn saida_chega_e_codigo_de_erro_aparece() {
        let (_tx, rx) = oneshot::channel();
        let v: Vec<Saida> = executar(
            "echo oi; echo erro >&2; exit 3".into(),
            std::env::temp_dir(),
            rx,
        )
        .collect()
        .await;
        let linhas: Vec<_> = v
            .iter()
            .filter_map(|s| {
                if let Saida::Linha(l) = s {
                    Some(l.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(linhas.contains(&"oi") && linhas.contains(&"erro"), "{v:?}");
        assert!(
            matches!(v.last(), Some(Saida::Fim(f)) if f.contains('3')),
            "{v:?}"
        );
    }

    #[tokio::test]
    async fn parar_mata_o_processo() {
        let (tx, rx) = oneshot::channel();
        let fluxo = executar("sleep 30".into(), std::env::temp_dir(), rx);
        tx.send(()).unwrap();
        let v: Vec<Saida> = fluxo.collect().await;
        assert!(
            matches!(v.last(), Some(Saida::Fim(f)) if f == "[parado]"),
            "{v:?}"
        );
    }
}
