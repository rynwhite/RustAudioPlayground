// src/audio_app_manager.rs

use eframe::{egui, App, Frame, CreationContext};
use std::sync::{Arc, Mutex};
use crate::dsp_module::DSPModule;
use crate::audio_app::AudioApp;



pub struct AudioAppManager {
    modules: Vec<Arc<dyn DSPModule>>,
    current_module_index: usize,
    current_audio_app: Option<AudioApp>,
    cpu_usage: Arc<Mutex<f32>>, // Shared CPU usage field
}

impl AudioAppManager {
    pub fn new(modules: Vec<Arc<dyn DSPModule>>) -> Self {
        Self {
            modules,
            current_module_index: 0,
            current_audio_app: None,
            cpu_usage: Arc::new(Mutex::new(0.0)), // Initialize shared CPU usage
        }
    }

    pub fn switch_module(&mut self, index: usize) {
        if index >= self.modules.len() {
            return;
        }
        self.current_module_index = index;
        self.current_audio_app = None; // Reset to load the new module
    }

    fn initialize_current_app(&mut self, ctx: &egui::Context) {
        if self.current_audio_app.is_some() {
            return;
        }

        if let Some(module) = self.modules.get(self.current_module_index) {
            let builder = module.initialize();
            match builder.build(self.cpu_usage.clone()) { // Pass shared CPU usage
                Ok(app) => {
                    self.current_audio_app = Some(app);
                }
                Err(e) => {
                    eprintln!("Failed to build AudioApp: {}", e);
                }
            }
        }
    }
}

impl App for AudioAppManager {
    fn update(&mut self, ctx: &egui::Context, frame: &mut Frame) {
        // Initialize the current app if not already done
        self.initialize_current_app(ctx);
        let cpu_usage = *self.cpu_usage.lock().unwrap(); // Access the shared CPU usage
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {

            ui.ctx().set_pixels_per_point(2.4);
            // Optional: Adjust padding or spacing for the group
            ui.add_space(9.0);

            // Create a bordered group
            ui.group(|ui| {
                // Optional: Add some padding inside the group
                ui.horizontal(|ui| {
                    // Label for the ComboBox
                    ui.label("Module");

                    // Module Dropdown
                    egui::ComboBox::from_id_source("module_dropdown")
                        .width(200.0) // Optional: set a fixed width
                        .selected_text(
                            self.modules
                                .get(self.current_module_index)
                                .map(|m| m.name().to_string())
                                .unwrap_or_else(|| "None".to_string()),
                        )
                        .show_ui(ui, |cb| {
                            for (index, module) in self.modules.iter().enumerate() {
                                cb.selectable_value(
                                    &mut self.current_module_index,
                                    index,
                                    module.name(),
                                );
                            }
                        });

                    // Switch Button
                    if ui
                        .add(egui::Button::new("Switch"))
                        .clicked()
                    {
                        self.switch_module(self.current_module_index);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("CPU Usage: {:.2}%", cpu_usage));
                    });
                });

                // Optional: Add separators or additional UI elements inside the group
                // ui.separator();
            });

            // Optional: Add more space after the group
            ui.add_space(5.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(ref mut app) = self.current_audio_app {
                app.update(ctx, frame);
            } else {
                ui.label("No DSP Module Selected.");
            }
        });
    }
}
