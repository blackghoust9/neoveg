//! The child process of the OpenFX host: `neoveg_ofx_host describe <library>` loads the plug-in
//! library, describes its plug-ins and prints the answer on a line that starts with
//! `NEOVEG_OFX_RESULT ` (see `neoveg_ofx::report`). Anything else the plug-ins print is ignored by
//! the program that reads it. The exit code is 0 when there is an answer with plug-ins, 1 when
//! there is an error and 2 for a command line this does not understand.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use neoveg_ofx::describe::describe_library;
use neoveg_ofx::Answer;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (answer, code) = match args.as_slice() {
        [command, path] if command == "describe" => match describe_library(&PathBuf::from(path)) {
            Ok(plugins) => (Answer::Plugins(plugins), ExitCode::SUCCESS),
            Err(message) => (Answer::Error(message), ExitCode::from(1)),
        },
        _ => (
            Answer::Error("uso: neoveg_ofx_host describe <biblioteca>".into()),
            ExitCode::from(2),
        ),
    };
    // Whatever the plug-ins printed has to be out first, so that the answer is the last line
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout);
    let _ = writeln!(stdout, "{}", answer.to_line());
    let _ = stdout.flush();
    code
}
