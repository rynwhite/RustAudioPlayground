// src/dsp_module.rs

use crate::audio_app::{AudioAppBuilder, ParamValue};

pub trait DSPModule {
    fn name(&self) -> &str;

    /// Initializes the AudioAppBuilder with module-specific parameters and processing functions.
    fn initialize(&self) -> AudioAppBuilder;
}
