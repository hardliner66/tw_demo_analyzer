use std::{collections::HashMap, process::exit};

use eframe::egui::{self, ComboBox, Key};
use egui_dropdown::DropDownBox;
use egui_plot::{Bar, BarChart, GridMark, Line, Plot, PlotPoints};
use stringlit::s;

use crate::data::{self, Inputs};

#[derive(Default)]
pub struct MyApp {
    pub names: Vec<String>,
    pub inputs: HashMap<String, Vec<Inputs>>,
    pub filter: String,
    pub selected: SelectedFilter,
}

#[derive(PartialEq, Eq, Default)]
pub enum SelectedFilter {
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
            ui.vertical(|ui| {
                ui.label("Player name:");
                ui.add_enabled(
                    self.names.len() > 1,
                    DropDownBox::from_iter(
                        &self.names,
                        "test_dropbox",
                        &mut self.filter,
                        |ui, text| ui.selectable_label(false, text),
                    ),
                );
            });
            let mut reset = false;
            ui.vertical(|ui| {
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
