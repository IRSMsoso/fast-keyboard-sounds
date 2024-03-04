use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, Mutex};

use cpal::{SampleFormat, SupportedBufferSize, SupportedStreamConfig};
use cpal::HostId::{Asio, Wasapi};
use cpal::traits::{DeviceTrait, HostTrait};
use glob::glob;
use log::{debug, error, info, warn};
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rdev::{EventType, Key, listen};
use rdev::EventType::KeyPress;
use rodio::{cpal, Decoder, OutputStream, Source};
use rodio::cpal::{BufferSize, SampleRate, StreamConfig};
use rodio::source::Buffered;
use serde::{Deserialize, Serialize};

struct ListenState {
    key_states: HashMap<Key, bool>,
}

impl ListenState {
    fn new() -> Self {
        ListenState {
            key_states: HashMap::new(),
        }
    }
}

fn get_buffered_sounds_from_directory(directory: &str) -> Vec<Buffered<Decoder<BufReader<File>>>> {
    let mut sounds = Vec::new();

    let full_glob = directory.to_owned() + "/*.wav";

    debug!("Full glob: {}", full_glob);

    for entry in glob(&full_glob).expect("Invalid glob pattern") {
        match entry {
            Ok(path) => {
                debug!("Found file: {:?}", path);

                let file = BufReader::new(File::open(path).unwrap());

                sounds.push(Decoder::new(file).unwrap().buffered());
            }
            Err(error) => {
                println!("Glob Error: {}", error);
            }
        }
    }

    sounds
}

#[derive(Debug, Serialize, Deserialize)]
struct DeviceConfig {
    host: Option<String>,
    device_name: Option<String>,
    num_channels: Option<u16>,
    sample_rate: Option<u32>,
    buffer_size: Option<u32>,
    format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    use_default: bool,
    device_config: DeviceConfig,
}

impl Config {
    fn default_config() -> Self {
        Self {
            use_default: true,
            device_config: DeviceConfig {
                host: None,
                device_name: None,
                num_channels: None,
                sample_rate: None,
                buffer_size: None,
                format: None,
            },
        }
    }
}

fn main() {
    // Initialize logger
    stderrlog::new()
        .module(module_path!())
        .verbosity(log::Level::Info)
        .init()
        .expect("Failed to initialize stderrlog");

    info!("Loading config...");

    let file_path = Path::new("./config.json");

    let config = match file_path.exists() {
        true => {
            info!("Config file found!");
            let file = File::open(file_path).expect("Cannot open config file despite it existing");
            serde_json::from_reader(file).expect("Failed to parse config")
        }
        false => {
            warn!("Could not find config file... Creating one.");
            let new_file = File::create(file_path).expect("Cannot create config file");
            let default_config = Config::default_config();
            serde_json::to_writer_pretty(new_file, &default_config)
                .expect("Couldn't write default config to file");
            default_config
        }
    };

    let (_stream, stream_handle) = match config.use_default {
        true => OutputStream::try_default().unwrap(),
        false => {
            let device = cpal::host_from_id(
                match config
                    .device_config
                    .host
                    .expect("Host not specified")
                    .to_lowercase()
                    .as_str()
                {
                    "asio" => Asio,
                    "wasapi" => Wasapi,
                    _ => {
                        panic!("Invalid host");
                    }
                },
            )
            .unwrap()
            .output_devices()
            .unwrap()
            .find(|device| {
                device.name().unwrap()
                    == config
                        .device_config
                        .device_name
                        .clone()
                        .expect("Device name not specified")
            })
            .expect("Couldn't find device");

            let buffer_size = config
                .device_config
                .buffer_size
                .expect("Buffer size not specified");

            // Parsing config
            let desired_stream_config = SupportedStreamConfig::new(
                config
                    .device_config
                    .num_channels
                    .expect("Number of channels not specified"),
                SampleRate(
                    config
                        .device_config
                        .sample_rate
                        .expect("Sample rate not specified"),
                ),
                SupportedBufferSize::Range {
                    min: buffer_size,
                    max: buffer_size,
                },
                match config
                    .device_config
                    .format
                    .expect("Sample format not specifed")
                    .to_lowercase()
                    .as_str()
                {
                    "i8" => SampleFormat::I8,
                    "i16" => SampleFormat::I16,
                    "i32" => SampleFormat::I32,
                    "i64" => SampleFormat::I64,
                    "u8" => SampleFormat::U8,
                    "u16" => SampleFormat::U16,
                    "u32" => SampleFormat::U32,
                    "u64" => SampleFormat::U64,
                    "f32" => SampleFormat::F32,
                    "f64" => SampleFormat::F64,
                    _ => {
                        panic!("Invalid sample format");
                    }
                },
            );

            OutputStream::try_from_device_config(&device, desired_stream_config).unwrap()
        }
    };

    // Load audio into memory
    let key_down_sounds = get_buffered_sounds_from_directory("./audio/keydown");
    let key_up_sounds = get_buffered_sounds_from_directory("./audio/keyup");
    let mouse_down_sounds = get_buffered_sounds_from_directory("./audio/mousedown");
    let mouse_up_sounds = get_buffered_sounds_from_directory("./audio/mouseup");

    if key_down_sounds.is_empty() {
        error!("No sounds in keydown folder");
        return;
    }

    if key_up_sounds.is_empty() {
        error!("No sounds in keyup folder");
        return;
    }

    if mouse_down_sounds.is_empty() {
        error!("No sounds in mousedown folder");
        return;
    }

    if mouse_up_sounds.is_empty() {
        error!("No sounds in mouseup folder");
        return;
    }

    let listen_state = Arc::new(Mutex::new(ListenState::new()));

    if let Err(error) = listen(move |event| {
        let listen_state_copy = listen_state.clone();

        let mut listen_state_lock = listen_state_copy.lock().unwrap();

        match event.event_type {
            KeyPress(key) => match listen_state_lock.key_states.get_mut(&key) {
                Some(key_is_pressed) => {
                    if !*key_is_pressed {
                        let sound = key_down_sounds.choose(&mut thread_rng()).unwrap();
                        stream_handle
                            .play_raw(sound.clone().convert_samples())
                            .unwrap();

                        *key_is_pressed = true;
                    }
                }
                None => {
                    let sound = key_down_sounds.choose(&mut thread_rng()).unwrap();
                    stream_handle
                        .play_raw(sound.clone().convert_samples())
                        .unwrap();

                    listen_state_lock.key_states.insert(key, true);
                }
            },
            EventType::KeyRelease(key) => match listen_state_lock.key_states.get_mut(&key) {
                Some(key_is_pressed) => {
                    if *key_is_pressed {
                        let sound = key_up_sounds.choose(&mut thread_rng()).unwrap();
                        stream_handle
                            .play_raw(sound.clone().convert_samples())
                            .unwrap();

                        *key_is_pressed = false;
                    }
                }
                None => {
                    let sound = key_up_sounds.choose(&mut thread_rng()).unwrap();
                    stream_handle
                        .play_raw(sound.clone().convert_samples())
                        .unwrap();

                    listen_state_lock.key_states.insert(key, false);
                }
            },
            EventType::ButtonPress(button) => {
                let sound = mouse_down_sounds.choose(&mut thread_rng()).unwrap();
                stream_handle
                    .play_raw(sound.clone().convert_samples())
                    .unwrap();
            }
            EventType::ButtonRelease(button) => {
                let sound = mouse_up_sounds.choose(&mut thread_rng()).unwrap();
                stream_handle
                    .play_raw(sound.clone().convert_samples())
                    .unwrap();
            }
            _ => {}
        }
    }) {
        println!("Error: {:?}", error);
    }
}
