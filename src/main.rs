mod move_generation;
mod search_threads;

use crate::search_threads::{SearchThreads, Signals};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::{process, thread};
use std::sync::atomic::Ordering::Relaxed;

fn main() {
    if cfg!(not(debug_assertions)) {
        std::panic::set_hook(Box::new(|panic_info| {
            eprintln!("Critical engine panic: {panic_info}");

            // Immediate termination of the entire engine process
            // We want a fast exit instead of unwinding through the engine.
            process::exit(1);
        }));
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdin_lines = stdin.lock().lines();

    let (cmd_tx, cmd_rx) = channel::<EngineCommand>();
    let stop_signal = Arc::new(AtomicBool::new(false));
    let quit_signal = Arc::new(AtomicBool::new(false));

    let stop_signal_clone = stop_signal.clone();
    let quit_signal_clone = quit_signal.clone();
    let search_thread_handle = thread::spawn(move || {
        search_loop(
            cmd_rx,
            Signals {
                stop_signal: stop_signal_clone,
                quit_signal: quit_signal_clone,
            },
        )
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
                writeln!(out, "id name HydraChessV7").unwrap();
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
                stop_signal.store(false, Relaxed);
                let (white_time_left_ms, white_increment, black_time_left_ms, black_increment) =
                    if parts.len() < 2 || parts[1] == "infinite" {
                        (u64::MAX, 0, u64::MAX, 0)
                    } else {
                        let mut temp_white_time_left_ms = u64::MAX;
                        let mut temp_white_increment = 0;
                        let mut temp_black_time_left_ms = u64::MAX;
                        let mut temp_black_increment = 0;

                        let mut i = 1;
                        loop {
                            if i >= parts.len() {
                                break;
                            }
                            if parts[i] == "wtime" {
                                temp_white_time_left_ms = parts[i + 1].parse().unwrap_or(u64::MAX);
                                i += 2;
                                continue;
                            } else if parts[i] == "winc" {
                                temp_white_increment = parts[i + 1].parse().unwrap_or(0);
                                i += 2;
                                continue;
                            } else if parts[i] == "btime" {
                                temp_black_time_left_ms = parts[i + 1].parse().unwrap_or(u64::MAX);
                                i += 2;
                                continue;
                            } else if parts[i] == "binc" {
                                temp_black_increment = parts[i + 1].parse().unwrap_or(0);
                                i += 2;
                                continue;
                            } else {
                                i += 1;
                            }
                        }
                        (
                            temp_white_time_left_ms,
                            temp_white_increment,
                            temp_black_time_left_ms,
                            temp_black_increment,
                        )
                    };

                cmd_tx
                    .send(EngineCommand::Go {
                        white_time_left_ms,
                        white_increment,
                        black_time_left_ms,
                        black_increment,
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
                quit_signal.store(true, Ordering::Relaxed);
                cmd_tx.send(EngineCommand::Quit).unwrap();
                search_thread_handle.join().unwrap();
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

fn search_loop(cmd_rx: Receiver<EngineCommand>, signals: Signals) {
    let mut search = SearchThreads::new(signals);
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
                search.go(
                    white_time_left_ms,
                    white_increment,
                    black_time_left_ms,
                    black_increment,
                );
            }
            EngineCommand::Debug => {
                search.debug();
            }
            EngineCommand::Quit => {
                break;
            }
        }
    }
}
