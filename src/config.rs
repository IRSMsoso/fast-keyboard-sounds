use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DeviceConfig {
    pub(crate) host: Option<String>,
    pub(crate) device_name: Option<String>,
    pub(crate) num_channels: Option<u16>,
    pub(crate) sample_rate: Option<u32>,
    pub(crate) buffer_size: Option<u32>,
    pub(crate) format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Config {
    pub(crate) use_default: bool,
    pub(crate) device_config: DeviceConfig,
}

impl Config {
    pub(crate) fn default_config() -> Self {
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
