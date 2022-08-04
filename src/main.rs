mod log_item;
mod utils;

use std::{
    collections::HashMap,
    io::{self, stdin},
    thread::sleep,
    time::Duration,
};

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle, WeakProgressBar};
use log_item::Verbosity;

use crate::{log_item::ActivityType, utils::store_path_base};

fn main() -> io::Result<()> {
    let mut bars = MultiProgress::new();
    let mut activities: HashMap<i64, ProgressBar> = HashMap::new();
    let mut activity_names: HashMap<i64, String> = HashMap::new();

    let mut builds_bar: WeakProgressBar = WeakProgressBar::new();
    let mut copy_paths_bar: WeakProgressBar = WeakProgressBar::new();

    for line in stdin().lines() {
        let line = line?;
        let item = log_item::parse_line(&line)?;
        match item {
            log_item::LogItem::Msg { level, msg } => {
                if level <= Verbosity::Error && !msg.starts_with("linking ") {
                    bars.println(msg)?
                }
            }
            log_item::LogItem::Start {
                id,
                level,
                text,
                activity,
            } => {
                if activities.contains_key(&id) {
                    bars.println(format!("Duplicate id {}", id))?
                } else {
                    use log_item::Activity::*;

                    let bytes_style = ProgressStyle::with_template(
                        "{spinner} [{bytes:>10}/{total_bytes:>10}] {wide_msg}",
                    )
                    .unwrap();

                    let bar_style = ProgressStyle::with_template(
                        "{spinner} {prefix:.bold} ({pos}/{len}) {wide_bar}",
                    )
                    .unwrap();
                    let msg_style =
                        ProgressStyle::with_template("{spinner} [{elapsed_precise}] {wide_msg}")
                            .unwrap();
                    let log_style = ProgressStyle::with_template(
                        "{spinner} [{elapsed_precise}] {prefix:.bold}: {wide_msg}",
                    )
                    .unwrap();

                    let activity_type = activity.to_type();

                    let bar = ProgressBar::new_spinner().with_message(text);

                    let bar = match activity {
                        CopyPaths => {
                            copy_paths_bar = bar.downgrade();
                            bar.with_style(bar_style).with_prefix("Downloading")
                        }
                        Builds => {
                            builds_bar = bar.downgrade();
                            bar.with_style(bar_style).with_prefix("Building")
                        }
                        Unknown => bar.with_style(msg_style),
                        CopyPath { path, from, to } => bar
                            .with_style(msg_style)
                            .with_prefix(store_path_base(&path)),
                        FileTransfer { uri } => bar
                            .with_style(bytes_style)
                            .with_message(uri),
                        Realise => bar.with_style(msg_style).with_message("Realising paths"),
                        Build {
                            path,
                            machine,
                            round,
                            total_rounds,
                        } => {
                            let name = store_path_base(&path);
                            activity_names.insert(id, name.clone());
                            bar.with_style(log_style).with_prefix(name)
                        }
                        OptimiseStore => bar.with_style(msg_style),
                        VerifyPaths => bar.with_style(msg_style),
                        Substitute { path, uri } => bar.with_style(msg_style),
                        QueryPathInfo { path, uri } => bar.with_style(msg_style),
                        PostBuildHook { path } => bar.with_style(msg_style),
                        BuildWaiting { path, resolved } => bar.with_style(msg_style),
                    };

                    let bar = match activity_type {
                        ActivityType::FileTransfer => {
                            bar.set_draw_target(ProgressDrawTarget::hidden());
                            bars.add(bar)
                        }

                        ActivityType::CopyPath => {
                            if let Some(parent) = copy_paths_bar.upgrade() {
                                bars.insert_after(&parent, bar)
                            } else {
                                bars.add(bar)
                            }
                        }

                        ActivityType::Build => {
                            if let Some(parent) = builds_bar.upgrade() {
                                bars.insert_after(&parent, bar)
                            } else {
                                bars.add(bar)
                            }
                        }

                        _ => bars.add(bar),
                    };

                    activities.insert(id, bar);
                }
            }
            log_item::LogItem::Result { id, result } => {
                if let Some(bar) = activities.get(&id) {
                    use log_item::LogResult::*;
                    bar.tick();
                    match result {
                        FileLinked { size, blocks } => {}
                        BuildLogLine { line } => {
                            let name = activity_names.get(&id).map(|x| x.as_str()).unwrap_or("");
                            // bars.println(format!("{}> {}", name, line))?;
                            bar.set_message(
                                String::from_utf8(strip_ansi_escapes::strip(line)?).unwrap(),
                            );
                        }
                        UntrustedPath { path } => {}
                        CorruptedPath { path } => {}
                        SetPhase { phase } => {
                            let name = activity_names.get(&id).map(|x| x.as_str()).unwrap_or("");
                            bar.set_prefix(format!("{} ({})", name, phase));
                        }
                        Progress {
                            done,
                            expected,
                            running,
                            failed,
                        } => {
                            bar.set_length(expected as u64);
                            bar.set_position(done as u64);
                            if bar.is_hidden() && done > 0 {
                                let bar = activities.remove(&id).unwrap();
                                let bar = if let Some(parent) = copy_paths_bar.upgrade() {
                                    bars.insert_after(&parent, bar)
                                } else {
                                    bars.add(bar)
                                };
                                activities.insert(id, bar);
                            }
                        }
                        SetExpected {
                            activity_type,
                            expected,
                        } => {}
                        PostBuildLogLine { line } => {}
                    }
                } else {
                    bars.println(format!("Missing id {}", id))?
                }
            }
            log_item::LogItem::Stop { id } => {
                if let Some(bar) = activities.remove(&id) {
                    bar.finish_and_clear();
                    bars.remove(&bar);
                }
            }
            log_item::LogItem::OutputLine(msg) => bars.println(msg)?,
            log_item::LogItem::UnknownItem(_) => {
                bars.println(format!("Unknown message: {}", line))?
            }
        }
    }

    Ok(())
}
