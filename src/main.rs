mod move_generation;
mod search;

use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use crate::search::Search;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdin_lines = stdin.lock().lines();

    let (cmd_tx, cmd_rx) = channel::<EngineCommand>();
    let stop_signal = Arc::new(AtomicBool::new(false));

    let stop_signal_clone = stop_signal.clone();
    let search_thread_handle = tokio::spawn(async move {
        search_loop(cmd_rx, stop_signal_clone)
    });

    // UCI main loop: read commands from stdin line by line.
    while let Some(Ok(line)) = stdin_lines.next() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        let command = parts.first().copied().unwrap_or("");

        match command {
            "uci" => {
                let mut out = stdout.lock();
                writeln!(out, "id name HydraChessRandom").unwrap();
                writeln!(out, "id author Hydra").unwrap();
                writeln!(out, "uciok").unwrap();
                out.flush().unwrap();
            }
            "isready" => {
                let mut out = stdout.lock();
                writeln!(out, "readyok").unwrap();
                out.flush().unwrap();
            }
            "ucinewgame" => {
                cmd_tx.send(EngineCommand::NewGame).unwrap();
            }
            "position" => {
                let position = parts[1..].join(" ");
                cmd_tx.send(EngineCommand::Position { position }).unwrap();
            }
            "go" => {
                // TODO: Parse go args
                cmd_tx
                    .send(EngineCommand::Go {
                        white_time_left_ms: 0,
                        white_increment: 0,
                        black_time_left_ms: 0,
                        black_increment: 0,
                    })
                    .unwrap();
            }
            "debug" => {
                cmd_tx.send(EngineCommand::Debug).unwrap();
            }
            "stop" => {
                stop_signal.store(true, Ordering::Relaxed);
            }
            "quit" => {
                stop_signal.store(true, Ordering::Relaxed);
                cmd_tx.send(EngineCommand::Quit).unwrap();
                search_thread_handle.await.unwrap();
                break;
            }
            _ => {
                // Ignore unknown commands silently (common in UCI).
            }
        }
    }
}

enum EngineCommand {
    NewGame,
    Position {
        position: String,
    },
    Go {
        white_time_left_ms: u64,
        white_increment: u64,
        black_time_left_ms: u64,
        black_increment: u64,
    },
    Debug,
    Quit,
}

fn search_loop(cmd_rx: Receiver<EngineCommand>, stop_signal: Arc<AtomicBool>) {
    let mut search = Search::new();
    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            EngineCommand::NewGame => {
                search.new_game();
            }
            EngineCommand::Position { position } => {
                search.position(&position);
            }
            EngineCommand::Go {
                white_time_left_ms,
                white_increment,
                black_time_left_ms,
                black_increment,
            } => {
                stop_signal.store(false, Ordering::Relaxed);
                search.search(&stop_signal);
            }
            EngineCommand::Debug => {
                search.debug();
            }
            EngineCommand::Quit => break,
        }
    }
}
