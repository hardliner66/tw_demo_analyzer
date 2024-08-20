use std::{
    cmp::Ordering, collections::HashMap, fs::File, io::BufReader, path::PathBuf, process::exit,
};

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use stringlit::s;
use twsnap::{compat::ddnet::DemoReader, enums::HookState, time::Instant, Snap};

mod data;

use data::Inputs;

#[derive(ValueEnum, Clone)]
enum AnalysisOutputFormat {
    Plain,
    Json,
    Yaml,
    Toml,
    Rsn,
}

#[derive(ValueEnum, Clone)]
enum ExtractionOutputFormat {
    Json,
    Yaml,
    Toml,
    Rsn,
}

#[derive(Parser, Clone)]
struct FilterOptions {
    #[arg(long, default_value = "")]
    filter: String,

    #[arg(short, long)]
    /// Pretty print if the format supports it
    pretty: bool,
}

#[derive(Parser)]
struct Args {
    #[arg(global = true, short, long)]
    /// Where to output the file to. If not specified, stdout is used.
    out: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(visible_aliases=["analyse", "a"])]
    Analyze {
        #[command(flatten)]
        filter_options: FilterOptions,
        #[arg(short, long, default_value = "plain")]
        format: AnalysisOutputFormat,
        path: PathBuf,
    },
    #[command(visible_alias = "e")]
    Extract {
        #[command(flatten)]
        filter_options: FilterOptions,
        #[arg(short, long, default_value = "json")]
        format: ExtractionOutputFormat,
        path: PathBuf,
    },
    #[command(visible_aliases = ["m", "em"])]
    ExtractMap { path: PathBuf },
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

#[derive(Serialize)]
struct CombinedStats {
    direction_change_rate_average: f32,
    direction_change_rate_median: f32,
    direction_change_rate_max: f32,
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
        let duration_since_last_change = c.0.duration_since(last_tick).unwrap_or_default();
        let ticks_per_action = duration_since_last_change.ticks();
        if ticks_per_action > 0 {
            times.push((1.0 / ticks_per_action as f32) * 50.0);
            last_tick = c.0;
        }
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

fn hook_pressed(hs: HookState) -> bool {
    match hs {
        HookState::Retracted => false,
        HookState::Idle => false,
        HookState::RetractStart => false,
        HookState::Retracting => false,
        HookState::RetractEnd => false,
        HookState::Flying => true,
        HookState::Grabbed => true,
    }
}
fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Analyze {
            path,
            format,
            filter_options,
        } => {
            let file = BufReader::new(File::open(path).unwrap());
            let mut reader = DemoReader::new(file).expect("Couldn't open demo reader");
            let mut direction_stats = HashMap::new();
            let mut hook_stats = HashMap::new();
            let mut inputs = HashMap::<String, Vec<Inputs>>::new();
            let mut snap = Snap::default();
            let mut last_input_direction = HashMap::new();
            let mut last_input_hook = HashMap::new();
            while let Ok(Some(_chunk)) = reader.next_chunk(&mut snap) {
                for (_id, p) in snap.players.iter() {
                    let name = p.name.to_string();
                    if !name
                        .to_lowercase()
                        .contains(&filter_options.filter.to_lowercase())
                    {
                        continue;
                    }
                    if let Some(tee) = &p.tee {
                        inputs
                            .entry(name.clone())
                            .or_insert_with(|| Vec::new())
                            .push(tee.into());
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
                            .or_insert(hook_pressed(tee.hook_state))
                            != hook_pressed(tee.hook_state);
                        if input_changed_hook {
                            hook_stats
                                .entry(name.clone())
                                .or_insert(Vec::new())
                                .push(DirectionChange(tee.tick));
                        }
                        last_input_hook.insert(name.clone(), hook_pressed(tee.hook_state));
                    }
                }
            }

            let direction_stats = direction_stats
                .into_iter()
                .map(|(n, s)| (n, calculate_direction_change_stats(s)));

            let mut hook_stats = hook_stats
                .into_iter()
                .map(|(n, s)| (n, calculate_direction_change_stats(s)))
                .collect::<HashMap<_, _>>();

            let stats = direction_stats
                .map(move |(n, ds)| {
                    let hs = hook_stats.remove(&n).unwrap_or_default();
                    let c = CombinedStats {
                        direction_change_rate_average: ds.average,
                        direction_change_rate_median: ds.median,
                        direction_change_rate_max: ds.max,
                        hook_state_change_rate_average: hs.average,
                        hook_state_change_rate_median: hs.median,
                        hook_state_change_rate_max: hs.max,
                        direction_changes: ds.overall_changes,
                        hook_changes: hs.overall_changes,
                        overall_changes: ds.overall_changes + hs.overall_changes,
                    };
                    (n, c)
                })
                .collect::<HashMap<_, _>>();

            let output = match format {
                AnalysisOutputFormat::Json => {
                    if filter_options.pretty {
                        serde_json::to_string_pretty(&stats).unwrap()
                    } else {
                        serde_json::to_string(&stats).unwrap()
                    }
                }
                AnalysisOutputFormat::Yaml => serde_yaml::to_string(&stats).unwrap(),
                AnalysisOutputFormat::Toml => {
                    if filter_options.pretty {
                        toml::to_string_pretty(&stats).unwrap()
                    } else {
                        toml::to_string(&stats).unwrap()
                    }
                }
                AnalysisOutputFormat::Rsn => {
                    if filter_options.pretty {
                        rsn::to_string_pretty(&stats)
                    } else {
                        rsn::to_string(&stats)
                    }
                }
                AnalysisOutputFormat::Plain => {
                    let strings: Vec<String> = stats
                        .into_iter()
                        .map(
                            |(
                                name,
                                CombinedStats {
                                    direction_change_rate_average,
                                    direction_change_rate_median,
                                    direction_change_rate_max,
                                    hook_state_change_rate_average,
                                    hook_state_change_rate_median,
                                    hook_state_change_rate_max,
                                    direction_changes,
                                    hook_changes,
                                    overall_changes,
                                    ..
                                },
                            )| {
                                let mut vec = Vec::with_capacity(11);
                                vec.push(format!("{:=^44}", format!(" {name} ")));
                                vec.push(s!(""));
                                vec.push(format!("Overal Input State Changes : {overall_changes}"));
                                vec.push(format!(
                                    "Direction Changes ........ : {direction_changes}"
                                ));
                                vec.push(format!("Hook Changes ............. : {hook_changes}"));
                                vec.push(s!(""));
                                vec.push(format!("{:-^44}", format!(" Direction Change Rate ")));
                                vec.push(s!(""));
                                vec.push(format!(
                                    "Average : {direction_change_rate_average:0>5.2} per second"
                                ));
                                vec.push(format!(
                                    "Median  : {direction_change_rate_median:0>5.2} per second"
                                ));
                                vec.push(format!(
                                    "Max ... : {direction_change_rate_max:0>5.2} per second"
                                ));
                                vec.push(s!(""));
                                vec.push(format!("{:-^44}", format!(" Hook State Change Rate ")));
                                vec.push(s!(""));
                                vec.push(format!(
                                    "Average : {hook_state_change_rate_average:0>5.2} per second"
                                ));
                                vec.push(format!(
                                    "Median  : {hook_state_change_rate_median:0>5.2} per second"
                                ));
                                vec.push(format!(
                                    "Max ... : {hook_state_change_rate_max:0>5.2} per second"
                                ));
                                vec.push(s!(""));
                                vec.push(s!("============================================"));
                                vec.push(format!("{:=^44}", s!(" END ")));
                                vec.push(s!("============================================"));
                                vec.push(s!(""));
                                vec.push(s!(""));

                                vec.join("\n")
                            },
                        )
                        .collect();
                    strings.join("\n")
                }
            };
            if let Some(out) = args.out {
                std::fs::write(out, output)?;
            } else {
                println!("{output}");
            }
        }
        Command::Extract {
            path,
            format,
            filter_options,
        } => {
            let file = BufReader::new(File::open(path).unwrap());
            let mut reader = DemoReader::new(file).expect("Couldn't open demo reader");
            let mut inputs = HashMap::new();
            let mut snap = Snap::default();
            while let Ok(Some(_chunk)) = reader.next_chunk(&mut snap) {
                for (_id, p) in snap.players.iter() {
                    let name = p.name.to_string();
                    if !name
                        .to_lowercase()
                        .contains(&filter_options.filter.to_lowercase())
                    {
                        continue;
                    }
                    if let Some(tee) = &p.tee {
                        inputs
                            .entry(name.clone())
                            .or_insert_with(|| Vec::<Inputs>::new())
                            .push(tee.into());
                    }
                }
            }

            let output = match format {
                ExtractionOutputFormat::Json => {
                    if filter_options.pretty {
                        serde_json::to_string_pretty(&inputs).unwrap()
                    } else {
                        serde_json::to_string(&inputs).unwrap()
                    }
                }
                ExtractionOutputFormat::Yaml => serde_yaml::to_string(&inputs).unwrap(),
                ExtractionOutputFormat::Toml => {
                    if filter_options.pretty {
                        toml::to_string_pretty(&inputs).unwrap()
                    } else {
                        toml::to_string(&inputs).unwrap()
                    }
                }
                ExtractionOutputFormat::Rsn => {
                    if filter_options.pretty {
                        rsn::to_string_pretty(&inputs)
                    } else {
                        rsn::to_string(&inputs)
                    }
                }
            };
            if let Some(out) = args.out {
                std::fs::write(out, output)?;
            } else {
                println!("{output}");
            }
        }
        Command::ExtractMap { path } => {
            let file = BufReader::new(File::open(path).unwrap());
            let reader = DemoReader::new(file).expect("Couldn't open demo reader");
            let map_name = format!("{}.map", reader.map_name());
            if let Some(map_data) = reader.map_data() {
                let p: PathBuf = if let Some(out) = args.out {
                    if out.is_dir() {
                        out.join(map_name).into()
                    } else {
                        out.into()
                    }
                } else {
                    map_name.into()
                };
                std::fs::write(p, map_data).unwrap();
            } else {
                eprintln!("Map not found in demo!");
                exit(1);
            }
        }
    }

    Ok(())
}
