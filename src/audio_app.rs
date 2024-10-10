// src/audio_app.rs

use eframe::{egui, App, NativeOptions};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::dsp::DspProcessor;
use std::fs;

#[derive(Clone)]
pub enum ParamValue {
    Number(f32),
    Boolean(bool),
}

#[derive(Clone)]
pub struct AudioParam {
    pub name: String,
    pub value: Arc<Mutex<ParamValue>>,
    pub min: f32,
    pub max: f32,
}

pub struct AudioAppBuilder {
    params: Vec<AudioParam>,
    process_fn: Option<Arc<dyn Fn(&mut [i16], &[ParamValue]) + Send + Sync + 'static>>,
    window_title: String,
    native_options: NativeOptions,
}

impl AudioAppBuilder {
    pub fn new() -> Self {
        Self {
            params: Vec::new(),
            process_fn: None,
            window_title: "Audio Controller".to_string(),
            native_options: NativeOptions::default(),
        }
    }

    pub fn add_param(mut self, name: &str, value: ParamValue, min: f32, max: f32) -> Self {
        self.params.push(AudioParam {
            name: name.to_string(),
            value: Arc::new(Mutex::new(value)),
            min,
            max,
        });
        self
    }

    pub fn set_process_fn<F>(mut self, process_fn: F) -> Self
    where
        F: Fn(&mut [i16], &[ParamValue]) + Send + Sync + 'static,
    {
        self.process_fn = Some(Arc::new(process_fn));
        self
    }

    pub fn set_window_title(mut self, title: &str) -> Self {
        self.window_title = title.to_string();
        self
    }

    pub fn set_native_options(mut self, options: NativeOptions) -> Self {
        self.native_options = options;
        self
    }

    pub fn build(self, cpu_usage: Arc<Mutex<f32>>) -> Result<AudioApp, eframe::Error> {
        let process_fn = self.process_fn.expect("Process function must be set");
        let mut audio_app = AudioApp::new(self.params, process_fn, cpu_usage);

        // Automatically load and play the first audio file
        if let Some(first_file) = audio_app.available_files.first().cloned() {
            audio_app.selected_file = Some(first_file.clone());
            audio_app.load_audio(&first_file);
            println!("Automatically playing the first audio file: {}", first_file);
        } else {
            println!("No audio files found in src/assets.");
        }

        Ok(audio_app)
    }
}

pub struct AudioApp {
    params: Vec<AudioParam>,
    dsp_processor: Option<DspProcessor>,
    is_playing: Arc<AtomicBool>,
    bypass: Arc<AtomicBool>, // Bypass flag
    available_files: Vec<String>,
    selected_file: Option<String>,
    process_fn: Arc<dyn Fn(&mut [i16], &[ParamValue]) + Send + Sync + 'static>,
    available_block_sizes: Vec<usize>,
    selected_block_size: usize,
    cpu_usage: Arc<Mutex<f32>>,
}

impl AudioApp {
    pub fn new(
        params: Vec<AudioParam>,
        process_fn: Arc<dyn Fn(&mut [i16], &[ParamValue]) + Send + Sync + 'static>,
        cpu_usage: Arc<Mutex<f32>>,
    ) -> Self {
        let is_playing = Arc::new(AtomicBool::new(false));
        let bypass = Arc::new(AtomicBool::new(false));

        // Scan the src/assets directory for audio files
        let assets_path = "src/assets";
        let mut available_files = Vec::new();

        if let Ok(entries) = fs::read_dir(assets_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "wav" || ext == "mp3" || ext == "ogg" {
                            if let Some(file_name) = path.file_name() {
                                if let Some(name_str) = file_name.to_str() {
                                    available_files.push(name_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        available_files.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

        // Define available block sizes
        let available_block_sizes = vec![1024, 2048, 4096, 8192, 16384];
        let selected_block_size = 4096; // Default block size

        AudioApp {
            params,
            dsp_processor: None,
            is_playing,
            bypass,
            available_files,
            selected_file: None,
            process_fn,
            available_block_sizes,
            selected_block_size,
            cpu_usage,
        }
    }

    pub fn load_audio(&mut self, file_name: &str) {
        if let Some(ref dsp) = self.dsp_processor {
            dsp.stop();
        }

        let process_fn = Arc::clone(&self.process_fn);
        let bypass = Arc::clone(&self.bypass);
        let block_size = self.selected_block_size;
        let cpu_usage = self.cpu_usage.clone(); // Use the shared CPU usage

        let file_path = format!("src/assets/{}", file_name);
        let dsp_processor = DspProcessor::new(
            &file_path,
            Arc::clone(&self.is_playing),
            bypass,
            self.params.iter().map(|p| Arc::clone(&p.value)).collect(),
            process_fn,
            block_size,
            cpu_usage, 
        );

        self.is_playing.store(true, Ordering::SeqCst);
        dsp_processor.process();

        self.dsp_processor = Some(dsp_processor);
    }
}

impl App for AudioApp {
    /// The `update` method is called on each frame to update the UI.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Header Row
            ui.horizontal(|ui| {
                // Left Side: Dropdowns and Play/Stop Buttons
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0; // Adjust spacing between buttons

                        // Audio File Dropdown
                        egui::ComboBox::from_label("Audio File")
                            .selected_text(
                                self.selected_file
                                    .clone()
                                    .unwrap_or_else(|| "None".to_string()),
                            )
                            .show_ui(ui, |cb| {
                                for file in &self.available_files {
                                    cb.selectable_value(
                                        &mut self.selected_file,
                                        Some(file.clone()),
                                        file,
                                    );
                                }
                            });

                        // Play Button
                        if ui.button("Play").clicked() {
                            if let Some(file) = self.selected_file.clone() {
                                self.load_audio(&file);
                            }
                        }

                        // Stop Button
                        if ui.button("Stop").clicked() {
                            self.is_playing.store(false, Ordering::SeqCst);
                            if let Some(ref dsp) = self.dsp_processor {
                                dsp.stop();
                            }
                        }
                    });
                });

                 // Calculate a spacer width as a percentage of the available width (e.g., 20%)
                let available_width = ui.available_width();
                let spacer_width = available_width - 260.0;

                // Allocate the calculated spacer width
                ui.allocate_space(egui::vec2(spacer_width, 0.0));

                // Spacer to push Bypass and Block Size to the right
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    // Bypass Checkbox
                    let mut bypass = self.bypass.load(Ordering::SeqCst);
                    if ui.checkbox(&mut bypass, "Bypass").changed() {
                        self.bypass.store(bypass, Ordering::SeqCst);
                    }

                    // Block Size Dropdown
                    ui.separator(); // Add some spacing
                    egui::ComboBox::from_label("Block Size")
                        .selected_text(self.selected_block_size.to_string())
                        .show_ui(ui, |cb| {
                            for &size in &self.available_block_sizes {
                                cb.selectable_value(
                                    &mut self.selected_block_size,
                                    size,
                                    size.to_string(),
                                );
                            }
                        });
                });
            });

            ui.separator();

            ui.add_space(20.0);
            // Plugin Parameters
            egui::ScrollArea::vertical().show(ui, |ui| {
                let available_width = ui.available_width();
                
                
                for param in &self.params {
                    let mut value = param.value.lock().unwrap();
                    ui.add_space(5.0);
                    // Use a horizontal layout to contain the label and the slider
                    ui.horizontal(|ui| {

                         let label_width = 140.0;

                        // Set a fixed width for the label so it doesn't affect slider width
                        let label = egui::Label::new(
                            egui::RichText::new(&param.name).text_style(egui::TextStyle::Body)
                        )
                        .wrap(false);
            
                        // Use add_sized to set the label size and ensure it takes up a fixed width space
                        ui.add_sized([label_width, 10.0], label);
            
            
                        // Use a spacer to manage spacing between label and slider
                 

                        // Apply the modified style back to the context
                        let mut style = (*ctx.style()).clone();
                        style.spacing.slider_width = 300.0; // Adjust the slider width as needed
                        ctx.set_style(style);
            
                        match &mut *value {
                            ParamValue::Number(ref mut v) => {
                                // Add a slider that fills the remaining width of the horizontal layout
                                ui.add(
                                    egui::Slider::new(v, param.min..=param.max)
                                        .text("") // Use an empty string for the slider text
                                        .show_value(true)
                                        
                                       // Adjust width based on label width
                                );
                            }
                            ParamValue::Boolean(ref mut v) => {
                                ui.checkbox(v, "");
                            }
                        }
                    });
                }
            });


        });
    }
}
