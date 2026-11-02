//! Programas de terminal (git, claude) e links, sem console no Windows.
//!
//! O FeatherCode é um programa de janela (`windows_subsystem = "windows"`): sem console próprio,
//! cada git ou claude aberto por baixo abriria uma janela preta por um instante. Eles rodam
//! com a saída lida pelo app, então sem janela (o que todo editor de código faz com o git).
//! Link abre pela API do Windows, não por um `cmd` escondido (o Defender acusa esse padrão).

use std::ffi::OsStr;

/// CREATE_NO_WINDOW: programa de console sem janela.
#[cfg(windows)]
const SEM_JANELA: u32 = 0x0800_0000;

pub fn comando(programa: impl AsRef<OsStr>) -> tokio::process::Command {
    #[allow(unused_mut)]
    let mut c = tokio::process::Command::new(programa);
    #[cfg(windows)]
    c.creation_flags(SEM_JANELA);
    c
}

pub fn comando_std(programa: impl AsRef<OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut c = std::process::Command::new(programa);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(SEM_JANELA);
    }
    c
}

/// Abre `url` no programa padrão do sistema.
pub fn abrir_link(url: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let largo = |s: &str| -> Vec<u16> { s.encode_utf16().chain([0]).collect() };
        let (verbo, alvo) = (largo("open"), largo(url));
        // SAFETY: textos terminados em zero que vivem até o fim da chamada.
        let r = unsafe { ShellExecuteW(std::ptr::null_mut(), verbo.as_ptr(), alvo.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) };
        // Acima de 32 é sucesso (a documentação do ShellExecute).
        return if r as isize > 32 { Ok(()) } else { Err(std::io::Error::other(format!("ShellExecute devolveu {}", r as isize))) };
    }
    #[cfg(target_os = "macos")]
    return std::process::Command::new("open").arg(url).spawn().map(drop);
    #[cfg(all(unix, not(target_os = "macos")))]
    return std::process::Command::new("xdg-open").arg(url).spawn().map(drop);
}
