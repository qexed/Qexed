use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_default();
    if cmd != "reproduce" {
        eprintln!("usage: qexed-doc-verify reproduce --page <mdx> [--json] [--preview]");
        return ExitCode::from(2);
    }

    let mut page = None;
    let mut json = false;
    let mut preview = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--preview" => preview = true,
            "--page" => page = args.next(),
            other if other.starts_with("--page=") => page = Some(other[7..].to_string()),
            other => {
                eprintln!("unknown argument: {other}");
                return ExitCode::from(2);
            }
        }
    }

    let Some(page) = page else {
        eprintln!("missing --page");
        return ExitCode::from(2);
    };

    let report = qexed_doc_verify::reproduce_page_opts(&PathBuf::from(page), preview);
    if json {
        match serde_json::to_string(&report) {
            Ok(text) => println!("{text}"),
            Err(err) => {
                eprintln!("{err}");
                return ExitCode::from(1);
            }
        }
    } else if report.ok {
        println!("{} {}", report.outcome, report.page);
    } else {
        eprintln!(
            "{} {}: {}",
            report.outcome,
            report.page,
            report.error.as_deref().unwrap_or("复现失败")
        );
    }

    if report.ok { ExitCode::SUCCESS } else { ExitCode::from(1) }
}
