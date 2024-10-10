use eframe::{egui, NativeOptions};
use std::sync::Arc;
use crate::dsp_module::DSPModule;
use crate::audio_app_manager::AudioAppManager;

// Import DSP modules
mod dsp;
mod dsp_module;
mod dsp_modules;
mod audio_app;
mod audio_app_manager;

// Bring DSP modules into scope
use dsp_modules::gain_control::GainControlModule;

fn main() -> Result<(), eframe::Error> {
    // Initialize DSP modules
    let gain_module = Arc::new(GainControlModule::new());
    // Add more modules as needed

    // Create a vector of DSP modules
    let modules: Vec<Arc<dyn DSPModule>> = vec![
        gain_module,
        // Add more modules here
    ];

    // Initialize the AudioAppManager with the modules
    let manager = AudioAppManager::new(modules);

    // Configure the viewport (window) settings
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([700.0, 1000.0]).with_max_inner_size([700.0, 2000.0]),
        ..Default::default()
    };

    // Run the manager with custom window options
    eframe::run_native(
        "DSP Library Manager",
        native_options,
        Box::new(|cc| Box::new(manager)),
    )
}
