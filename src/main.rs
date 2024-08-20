use std::{cmp::Ordering, collections::HashMap, fs::File, io::BufReader, path::PathBuf};

use clap::{Parser, ValueEnum};
use rayon::prelude::*;
use serde::Serialize;
use twsnap::{compat::ddnet::DemoReader, time::Instant, Snap};

#[derive(ValueEnum, Clone)]
enum OutputFormat {
    Json,
    Yaml,
    Toml,
    Rsn,
}

#[derive(Parser)]
struct Args {
    #[arg(short, long, default_value = "json")]
    format: OutputFormat,
    #[arg(short, long)]
    /// Pretty print if the format supports it
    pretty: bool,
    #[arg(short, long)]
    /// Where to output the file to. If not specified, stdout is used.
    out: Option<PathBuf>,
    paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
struct DirectionChange(Instant);

#[derive(Debug, Clone, Default)]
struct Stats {
    average: f32,
    median: f32,
    max: f32,
    overall_changes: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
struct CombinedStats {
    demo: PathBuf,
    direction_change_list: Vec<DirectionChange>,
    direction_change_rate_average: f32,
    direction_change_rate_median: f32,
    direction_change_rate_max: f32,
    hook_state_change_list: Vec<DirectionChange>,
    hook_state_change_rate_average: f32,
    hook_state_change_rate_median: f32,
    hook_state_change_rate_max: f32,
    direction_changes: usize,
    hook_changes: usize,
    overall_changes: usize,
}

fn calculate_direction_change_stats(mut changes: Vec<DirectionChange>) -> Stats {
    if changes.is_empty() || changes.len() == 1 {
        return Stats::default();
    }

    changes.sort_by_key(|a| a.0);

    let mut last_tick = changes.first().unwrap().0;

    let mut times = Vec::new();
    for c in changes.iter().skip(1) {
        let duration_since_last_change = c.0.duration_since(last_tick).unwrap();
        let ticks_per_action = duration_since_last_change.ticks();
        times.push((1.0 / ticks_per_action as f32) * 50.0);
        last_tick = c.0;
    }

    times.sort_by(|a, b| {
        if a < b {
            Ordering::Less
        } else if a > b {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    });

    let max = *times.last().unwrap();
    let average = times.iter().sum::<f32>() / times.len() as f32;

    let median = if times.len() % 2 == 0 {
        let mid = times.len() / 2;
        (times[mid - 1] + times[mid]) as f32 / 2.0
    } else {
        times[times.len() / 2] as f32
    };

    Stats {
        average,
        median,
        max,
        overall_changes: changes.len(),
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let stats = args.paths.par_iter().map(|path| {
        let file = BufReader::new(File::open(path).unwrap());

        let mut direction_stats = HashMap::new();
        let mut hook_stats = HashMap::new();
        if let Ok(mut reader) = DemoReader::new(file) {
            let mut snap = Snap::default();
            let mut last_input_direction = HashMap::new();
            let mut last_input_hook = HashMap::new();
            while let Ok(Some(_chunk)) = reader.next_chunk(&mut snap) {
                for (_id, p) in snap.players.iter() {
                    let name = p.name.to_string();
                    if let Some(tee) = &p.tee {
                        let input_changed_direction = *last_input_direction
                            .entry(name.clone())
                            .or_insert(tee.direction)
                            != tee.direction;
                        if input_changed_direction {
                            direction_stats
                                .entry(name.clone())
                                .or_insert(Vec::new())
                                .push(DirectionChange(tee.tick));
                        }
                        last_input_direction.insert(name.clone(), tee.direction);

                        let input_changed_hook = *last_input_hook
                            .entry(name.clone())
                            .or_insert(tee.hook_state)
                            != tee.hook_state;
                        if input_changed_hook {
                            hook_stats
                                .entry(name.clone())
                                .or_insert(Vec::new())
                                .push(DirectionChange(tee.tick));
                        }
                        last_input_hook.insert(name.clone(), tee.hook_state);
                    }
                }
            }
        }

        (path, direction_stats, hook_stats)
    });

    let stats = stats.map(|(demo, d, h)| {
        let direction_stats = d
            .into_iter()
            .map(|(n, s)| (n, (s.clone(), calculate_direction_change_stats(s))));

        let mut hook_stats = h
            .into_iter()
            .map(|(n, s)| (n, (s.clone(), calculate_direction_change_stats(s))))
            .collect::<HashMap<_, _>>();

        let combined = direction_stats
            .map(move |(n, (dc, ds))| {
                let (hc, hs) = hook_stats.remove(&n).unwrap_or_default();
                let c = CombinedStats {
                    demo: demo.clone(),
                    direction_change_list: dc,
                    direction_change_rate_average: ds.average,
                    direction_change_rate_median: ds.median,
                    direction_change_rate_max: ds.max,
                    hook_state_change_list: hc,
                    hook_state_change_rate_average: hs.average,
                    hook_state_change_rate_median: hs.median,
                    hook_state_change_rate_max: hs.max,
                    direction_changes: ds.overall_changes,
                    hook_changes: hs.overall_changes,
                    overall_changes: ds.overall_changes + hs.overall_changes,
                };
                (n, c)
            })
            .collect::<Vec<_>>();

        combined
    });

    let stats = stats.flatten().collect::<HashMap<_, _>>();

    let output = match args.format {
        OutputFormat::Json => {
            if args.pretty {
                serde_json::to_string_pretty(&stats).unwrap()
            } else {
                serde_json::to_string(&stats).unwrap()
            }
        }
        OutputFormat::Yaml => serde_yaml::to_string(&stats).unwrap(),
        OutputFormat::Toml => {
            if args.pretty {
                toml::to_string_pretty(&stats).unwrap()
            } else {
                toml::to_string(&stats).unwrap()
            }
        }
        OutputFormat::Rsn => {
            if args.pretty {
                rsn::to_string_pretty(&stats)
            } else {
                rsn::to_string(&stats)
            }
        }
    };

    if let Some(out) = args.out {
        std::fs::write(out, output)?;
    } else {
        println!("{output}");
    }

    Ok(())
}
