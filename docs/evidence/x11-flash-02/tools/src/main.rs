//! xflash: measurement tools for TEST_LOG 6.38 to 6.42 (docs/evidence/x11-flash-02/).
//! Subcommands: `observe` (X-side observer), `run` (run driver, one app per batch), `analyze`
//! (offline analysis of one or more batch directories).

mod analyze;
mod cell;
mod obs;
mod run;
mod util;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, rest) = match args.split_first() {
        Some((c, r)) => (c.as_str(), r),
        None => {
            eprintln!("usage: xflash observe|run|analyze ...");
            std::process::exit(2);
        }
    };
    let res = match cmd {
        "observe" => obs::run(rest),
        "run" => run::run(rest),
        "analyze" => analyze::run(rest),
        "cell" => cell::run(rest),
        other => Err(format!("unknown subcommand {other}")),
    };
    if let Err(e) = res {
        eprintln!("xflash {cmd}: {e}");
        std::process::exit(1);
    }
}
