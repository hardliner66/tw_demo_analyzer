#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{collections::HashMap, fs::File, io::BufReader, path::PathBuf, process::exit};

use clap::{Parser, Subcommand, ValueEnum};
use eframe::egui::{self, ComboBox, Key};
use egui_dropdown::DropDownBox;
use egui_plot::{Bar, BarChart, GridMark, Line, Plot, PlotPoints};
use serde::Serialize;
use stringlit::s;
use twsnap::{compat::ddnet::DemoReader, enums::HookState, Snap};
use winit::platform::x11::EventLoopBuilderExtX11;

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
    #[arg(short, long, default_value = "")]
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
        #[arg(long, default_value = "plain")]
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

    #[command(visible_alias = "v")]
    Visualize { path: PathBuf },
}

#[derive(Debug, Clone, Default)]
struct Stats {
    average: f32,
    median: f32,
    max: usize,
    overall_changes: usize,
}

#[derive(Serialize)]
struct CombinedStats {
    direction_change_rate_average: f32,
    direction_change_rate_median: f32,
    direction_change_rate_max: usize,
    hook_state_change_rate_average: f32,
    hook_state_change_rate_median: f32,
    hook_state_change_rate_max: usize,
    direction_changes: usize,
    hook_changes: usize,
    overall_changes: usize,
}

fn calculate_direction_change_stats(mut changes: Vec<i32>) -> Stats {
    if changes.is_empty() {
        return Stats::default();
    }

    changes.sort();

    let mut times = Vec::new();
    let changes_count = changes.len();
    for i in 0..changes_count {
        let last_tick = changes[i] + 50;
        let mut actions = 1;
        for n in 1..50 {
            if i + n >= changes_count || changes[i + n] > last_tick {
                break;
            }
            actions += 1;
        }
        times.push(actions);
    }

    assert!(
        times.len() > 0,
        "If we are here, we must have at least one action per second"
    );

    if times.is_empty() {
        return Stats::default();
    }

    times.sort();

    let max = *times.last().unwrap();
    let average = times.iter().sum::<usize>() as f32 / times.len() as f32;

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

fn extract(path: PathBuf, filter: &str) -> anyhow::Result<HashMap<String, Vec<Inputs>>> {
    let file = BufReader::new(File::open(path).unwrap());
    let mut reader = DemoReader::new(file).expect("Couldn't open demo reader");
    let mut inputs = HashMap::new();
    let mut snap = Snap::default();
    while let Ok(Some(_chunk)) = reader.next_chunk(&mut snap) {
        for (_id, p) in snap.players.iter() {
            let name = p.name.to_string();
            if !name.to_lowercase().contains(&filter.to_lowercase()) {
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
    Ok(inputs)
}

#[derive(Default)]
struct MyApp {
    names: Vec<String>,
    inputs: HashMap<String, Vec<Inputs>>,
    filter: String,
    selected: SelectedFilter,
}

#[derive(PartialEq, Eq, Default)]
enum SelectedFilter {
    #[default]
    ShowBoth,
    ShowHooks,
    ShowDirections,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_down(Key::Escape)) {
            exit(0);
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("My egui Application");
            let mut reset = false;
            ui.vertical(|ui| {
                ui.add(DropDownBox::from_iter(
                    &self.names,
                    "test_dropbox",
                    &mut self.filter,
                    |ui, text| ui.selectable_label(false, text),
                ));
                ComboBox::from_label("filter")
                    .selected_text(format!(
                        "{}",
                        match self.selected {
                            SelectedFilter::ShowBoth => "Both",
                            SelectedFilter::ShowHooks => "Hooks",
                            SelectedFilter::ShowDirections => "Directions",
                        }
                    ))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.selected, SelectedFilter::ShowHooks, "Hooks");
                        ui.selectable_value(
                            &mut self.selected,
                            SelectedFilter::ShowDirections,
                            "Directions",
                        );
                        ui.selectable_value(&mut self.selected, SelectedFilter::ShowBoth, "Both");
                    });
                reset = ui.button("Reset").clicked();
            });

            if let Some(data) = self.inputs.get(&self.filter) {
                let direction_data: PlotPoints = data
                    .iter()
                    .map(|t| {
                        [
                            t.tick as f64,
                            match t.direction {
                                data::Direction::Left => -1,
                                data::Direction::None => 0,
                                data::Direction::Right => 1,
                            } as f64,
                        ]
                    })
                    .collect();

                let hook_data: Vec<Bar> = data
                    .iter()
                    .map(|t| {
                        let hook = match t.hook_state {
                            data::HookState::Retracted => 0.0,
                            data::HookState::Idle => 0.0,
                            data::HookState::RetractStart => 0.0,
                            data::HookState::Retracting => 0.0,
                            data::HookState::RetractEnd => 0.0,
                            data::HookState::Flying => 0.5,
                            data::HookState::Grabbed => 0.5,
                        };
                        Bar::new(t.tick as f64, hook)
                    })
                    .collect();

                let directions = Line::new(direction_data);
                let hooks = BarChart::new(hook_data);
                let plot = Plot::new("direction_plot")
                    .allow_scroll(false)
                    .y_axis_formatter(|gm, _rng| {
                        if gm.value < 0.0 {
                            s!("Left")
                        } else if gm.value > 0.0 {
                            if gm.value > 0.4 && gm.value < 0.6 {
                                s!("Hook")
                            } else {
                                s!("Right")
                            }
                        } else {
                            s!("Idle")
                        }
                    })
                    .y_grid_spacer(|_| {
                        vec![
                            GridMark {
                                value: -1.0,
                                step_size: 1.0,
                            },
                            GridMark {
                                value: 0.0,
                                step_size: 1.0,
                            },
                            GridMark {
                                value: 0.5,
                                step_size: 0.5,
                            },
                            GridMark {
                                value: 1.0,
                                step_size: 1.0,
                            },
                        ]
                    })
                    .x_axis_formatter(|gm, _rng| format!("{}s", (gm.value / 50.0) as usize));
                let plot = if reset { plot.reset() } else { plot };
                plot.show(ui, |plot_ui| match self.selected {
                    SelectedFilter::ShowBoth => {
                        plot_ui.line(directions);
                        plot_ui.bar_chart(hooks)
                    }
                    SelectedFilter::ShowHooks => {
                        plot_ui.line(directions);
                    }
                    SelectedFilter::ShowDirections => plot_ui.bar_chart(hooks),
                });
            }
        });
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
                        let tick = (tee.tick.seconds() * 50.0) as i32;
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
                                .push(tick);
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
                                .push(tick);
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
                                    "Max ... : {:0>5.2} per second",
                                    direction_change_rate_max as f32
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
                                    "Max ... : {:0>5.2} per second",
                                    hook_state_change_rate_max as f32
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
            let inputs = extract(path, &filter_options.filter)?;
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
                std::fs::write(&p, map_data).unwrap();
                println!("Exported map to {p:?}");
            } else {
                eprintln!("Map not found in demo!");
                exit(1);
            }
        }
        Command::Visualize { path } => {
            let inputs = extract(path, "")?;

            let options = eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default(),
                event_loop_builder: Some(Box::new(|b| {
                    b.with_x11();
                })),
                ..Default::default()
            };
            let max_name = inputs
                .iter()
                .max_by_key(|i| i.1.len())
                .unwrap()
                .0
                .to_owned();
            let mut names: Vec<_> = inputs.keys().cloned().collect();
            names.sort();
            eframe::run_native(
                "My egui App",
                options,
                Box::new(|_| {
                    Ok(Box::<MyApp>::new(MyApp {
                        names,
                        inputs,
                        filter: max_name,
                        ..Default::default()
                    }))
                }),
            )
            .unwrap();
        }
    }

    Ok(())
}
