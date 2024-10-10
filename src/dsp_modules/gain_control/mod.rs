
use crate::dsp_module::DSPModule;
use crate::audio_app::{AudioAppBuilder, ParamValue};
use std::sync::Arc;

pub struct GainControlProcessor;


//  This is a library agnostic function
impl GainControlProcessor {
    pub fn new() -> Self {
        Self
    }

    pub fn process(&self, buffer: &mut [i16], gain: f32) {
        for sample in buffer.iter_mut() {
            let new_sample = (*sample as f32 * gain).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            *sample = new_sample;
        }
    }
}



//  This is an interface for the main audio app test app.
// it's creating an ap using AudioAppBuilder.
pub struct GainControlModule {
    processor: Arc<GainControlProcessor>,
}

impl GainControlModule {
    pub fn new() -> Self {
        Self {
            processor: Arc::new(GainControlProcessor::new()),
        }
    }
}

impl DSPModule for GainControlModule {
    fn name(&self) -> &str {
        "Gain Control"
    }


    fn initialize(&self) -> AudioAppBuilder {
        // Clone the processor Arc to move into the closure
        let processor = Arc::clone(&self.processor);

        let process_fn = move |buffer: &mut [i16], state: &[ParamValue]| {
            let gain = if let ParamValue::Number(v) = state.get(0).unwrap_or(&ParamValue::Number(1.0)) { *v } else { 1.0 };
            processor.process(buffer, gain);
        };

        AudioAppBuilder::new()
            .add_param("Gain", ParamValue::Number(1.0), 0.0, 2.0)
            .set_process_fn(Box::new(process_fn)) 
            .set_window_title("Gain Control")
    }
}
