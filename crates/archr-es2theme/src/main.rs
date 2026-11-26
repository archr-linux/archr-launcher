//! archr-es2theme <tema do ES> <pasta de saída> [--lang pt] [--name "Nome"]
//!
//! Converts an EmulationStation theme into an archr-launcher theme folder,
//! ready to copy to /storage/.config/archr-launcher/themes/ or publish.
use std::path::PathBuf;

const USAGE: &str = "uso: archr-es2theme <pasta do tema do ES> <pasta de saída> [--lang en|pt|es...] [--name \"Nome\"]";

fn main() {
    let mut args = std::env::args().skip(1);
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut opts = archr_es2theme::Options::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--lang" => opts.lang = args.next().unwrap_or_else(|| fail(USAGE)),
            "--name" => opts.name = Some(args.next().unwrap_or_else(|| fail(USAGE))),
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            _ => paths.push(arg.into()),
        }
    }
    let [src, out] = <[PathBuf; 2]>::try_from(paths).unwrap_or_else(|_| fail(USAGE));
    if out.join("theme.toml").exists() && out.read_dir().is_ok_and(|mut d| d.next().is_some()) {
        eprintln!(
            "aviso: {} já tem um theme.toml; os arquivos são substituídos",
            out.display()
        );
    }
    match archr_es2theme::convert(&src, &out, &opts) {
        Ok(c) => {
            println!(
                "{} visualizações, {} opções, {} arquivos ({:.1} MB) em {}",
                c.views.len(),
                c.options.len(),
                c.files,
                c.bytes as f64 / 1_048_576.,
                out.display()
            );
            println!(
                "{} itens ficaram de fora ou mudaram: veja CONVERSAO.md",
                c.notes.len()
            );
            if c.warnings.is_empty() {
                println!("o launcher lê o tema sem avisos");
            } else {
                println!("o launcher deu {} avisos ao ler o tema:", c.warnings.len());
                for w in &c.warnings {
                    println!("  {w}");
                }
                std::process::exit(2);
            }
        }
        Err(e) => fail(&e),
    }
}

fn fail(text: &str) -> ! {
    eprintln!("{text}");
    std::process::exit(1)
}
