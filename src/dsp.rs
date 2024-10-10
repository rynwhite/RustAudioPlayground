// src/dsp.rs

use rodio::{OutputStream, Sink, Decoder, Source};
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::audio_app::ParamValue;

use std::time::Instant;


pub struct DspProcessor {
    sink: Arc<Mutex<Sink>>,
    _stream: OutputStream,
    is_playing: Arc<AtomicBool>,
    bypass: Arc<AtomicBool>, // Bypass flag
    params: Vec<Arc<Mutex<ParamValue>>>,
    process_fn: Arc<dyn Fn(&mut [i16], &[ParamValue]) + Send + Sync + 'static>,
    block_size: usize, // Added block_size field
    cpu_usage: Arc<Mutex<f32>>, // New field for storing CPU usage
}

impl DspProcessor {
    pub fn new(
        file_path: &str,
        is_playing: Arc<AtomicBool>,
        bypass: Arc<AtomicBool>, // Bypass flag
        params: Vec<Arc<Mutex<ParamValue>>>,
        process_fn: Arc<dyn Fn(&mut [i16], &[ParamValue]) + Send + Sync + 'static>,
        block_size: usize, // Accept block_size parameter,
        cpu_usage: Arc<Mutex<f32>>,
    ) -> Self {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        println!("Audio output stream and sink created.");

        let file = File::open(file_path).expect("Failed to open audio file");
        println!("Audio file opened: {}", file_path);

        let source = Decoder::new(BufReader::new(file)).expect("Failed to decode audio");
        println!(
            "Audio file decoded successfully. Sample rate: {}, channels: {}",
            source.sample_rate(),
            source.channels()
        );

        // Initialise CPU monitor
     

        let dsp_processor = DspProcessor {
            sink: Arc::new(Mutex::new(sink)),
            _stream,
            is_playing,
            bypass,
            params,
            process_fn,
            block_size,
            cpu_usage: Arc::clone(&cpu_usage),
        };

        let dsp_source = dsp_processor.apply_dsp(source);
        dsp_processor.sink.lock().unwrap().append(dsp_source);

        println!("DSP-processed audio appended to the sink.");

        dsp_processor
    }

    fn apply_dsp<S>(&self, source: S) -> BlockProcessor<S>
    where
        S: Source<Item = i16> + Send + 'static,
    {
        BlockProcessor::new(
            source,
            Arc::clone(&self.is_playing),
            Arc::clone(&self.bypass), // Pass Bypass flag
            self.params.clone(),
            Arc::clone(&self.process_fn),
            self.block_size, // Pass block_size
        )
    }
    pub fn process(&self) {
        let sink = Arc::clone(&self.sink);
        let is_playing = Arc::clone(&self.is_playing);
        let bypass = Arc::clone(&self.bypass);
        let params = self.params.clone();
        let process_fn = Arc::clone(&self.process_fn);
        let block_size = self.block_size;
        let cpu_usage = Arc::clone(&self.cpu_usage);
        
        // Assume a sample rate of 44100 Hz
        let sample_rate = 48000.0;
        // Calculate block duration in seconds based on block size and sample rate
        let block_duration = block_size as f32 / sample_rate;
    
        thread::spawn(move || {
            println!("DSP thread started");
            
            sink.lock().unwrap().play();
            println!("Starting audio playback...");
            
            // Total processing time tracker
            let mut total_elapsed = 0.0;
            let mut processing_time = 0.0;
            
            while is_playing.load(Ordering::SeqCst) && !sink.lock().unwrap().empty() {
                let start_time = Instant::now();
                
                // Measure the DSP processing time
                let dsp_start = Instant::now();
                if !bypass.load(Ordering::SeqCst) {
                    let mut buffer = vec![0i16; block_size];
                    let param_values: Vec<ParamValue> = params.iter()
                        .map(|p| p.lock().unwrap().clone())
                        .collect();
                    (process_fn)(&mut buffer, &param_values);
                }
                processing_time += dsp_start.elapsed().as_secs_f32();
                
                // Update total elapsed time with the block duration
                total_elapsed += block_duration;
                
                // Calculate DSP CPU usage
                if total_elapsed > 0.0 {
                    let dsp_cpu_usage = (processing_time / total_elapsed) * 100.0;
                    println!("Estimated CPU Usage for DSP: {:.2}%", dsp_cpu_usage);
                    
                    // Update self.cpu_usage with the new value
                    let mut cpu_usage_lock = cpu_usage.lock().unwrap();
                    *cpu_usage_lock = dsp_cpu_usage;
                }
    
                // Calculate the remaining time in the block and sleep if needed
                let elapsed = start_time.elapsed();
                let block_duration_in_millis = (block_duration * 1000.0) as u64;
                if elapsed < Duration::from_millis(block_duration_in_millis) {
                    thread::sleep(Duration::from_millis(block_duration_in_millis) - elapsed);
                }
            }
            
            sink.lock().unwrap().stop();
            println!("DSP thread ending");
        });
    }

    pub fn stop(&self) {
        self.is_playing.store(false, Ordering::SeqCst);
        self.sink.lock().unwrap().stop();
    }
    pub fn get_cpu_usage(&self) -> f32 {
        *self.cpu_usage.lock().unwrap()
    }
    
}

pub struct BlockProcessor<S> {
    input: S,
    block: Vec<i16>,
    block_pos: usize,
    is_playing: Arc<AtomicBool>,
    bypass: Arc<AtomicBool>, // Bypass flag
    params: Vec<Arc<Mutex<ParamValue>>>,
    process_fn: Arc<dyn Fn(&mut [i16], &[ParamValue]) + Send + Sync + 'static>,
    samples_processed: usize,
    block_size: usize, // Added block_size field
}

impl<S> BlockProcessor<S>
where
    S: Source<Item = i16>,
{
    pub fn new(
        input: S,
        is_playing: Arc<AtomicBool>,
        bypass: Arc<AtomicBool>, // Accept Bypass flag
        params: Vec<Arc<Mutex<ParamValue>>>,
        process_fn: Arc<dyn Fn(&mut [i16], &[ParamValue]) + Send + Sync + 'static>,
        block_size: usize, // Accept block_size parameter
    ) -> Self {
        println!("Creating new BlockProcessor with block size: {}", block_size);
        BlockProcessor {
            input,
            block: Vec::with_capacity(block_size),
            block_pos: 0,
            is_playing,
            bypass,
            params,
            process_fn,
            samples_processed: 0,
            block_size,
        }
    }

    pub fn process_buffer(&mut self) {
        if self.bypass.load(Ordering::SeqCst) {
            // If bypass is active, skip processing
            println!("Bypass is active. Skipping processing.");
            return;
        }

        let param_values: Vec<ParamValue> = self.params.iter()
            .map(|p| p.lock().unwrap().clone())
            .collect();
        (self.process_fn)(&mut self.block, &param_values);
        self.samples_processed += self.block.len();
    }
}

impl<S> Iterator for BlockProcessor<S>
where
    S: Source<Item = i16>,
{
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        if !self.is_playing.load(Ordering::SeqCst) {
            return None;
        }

        if self.block_pos >= self.block.len() {
            let mut new_block = Vec::with_capacity(self.block_size);
            for _ in 0..self.block_size {
                if let Some(sample) = self.input.next() {
                    new_block.push(sample);
                } else {
                    break;
                }
            }

            if new_block.is_empty() {
                println!("End of audio stream reached. Total samples processed: {}", self.samples_processed);
                return None;
            }

            self.block = new_block;
            self.process_buffer();
            self.block_pos = 0;
        }

        if self.block_pos < self.block.len() {
            let sample = self.block[self.block_pos];
            self.block_pos += 1;
            Some(sample)
        } else {
            None
        }
    }
}

impl<S> Source for BlockProcessor<S>
where
    S: Source<Item = i16>,
{
    fn sample_rate(&self) -> u32 {
        self.input.sample_rate()
    }

    fn channels(&self) -> u16 {
        self.input.channels()
    }

    fn current_frame_len(&self) -> Option<usize> {
        self.input.current_frame_len()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.input.total_duration()
    }
}
