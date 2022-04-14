use std::path::Path;

use snafu::{Snafu, ResultExt};

use crate::audio::device::AudioDevice;
use crate::audio::source::AudioSource;
use crate::audio::device;
use crate::audio::source;
use crate::audio::source::FFmpegSource;
use crate::audio::volume;
use crate::info::media::MediaInfo;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("{}", source))]
    Device { 
        #[snafu(source(from(device::Error, Box::new)))]
        source: Box::<dyn std::error::Error + Send + Sync>,
    },

    #[snafu(display("{}", source))]
    Source {
        #[snafu(source(from(source::Error, Box::new)))]
        source: Box::<dyn std::error::Error + Send + Sync>
    },
}

pub trait Player {
    fn init_device_defalut(&mut self) -> Result<(), Error>;
    fn open<P: AsRef<Path>>(&mut self, filepath: &P) -> Result<(), Error>;
    fn close(&mut self) -> Result<(), Error>;
    fn play(&mut self) -> Result<(), Error>;
    fn stop(&self) -> Result<(), Error>;
    fn pause(&self) -> Result<(), Error>;
    fn seek(&self, time: i64) -> Result<(), Error>;

    fn get_bitrate(&self) -> u32;
    fn get_duration(&self) -> i64;
    fn get_current_time(&self) -> i64;
    fn is_playing(&self) -> bool;
    fn get_volume(&self) -> f32;

    fn set_volume(&mut self, volume: f32) -> Result<(), Error>;
    fn set_mute(&self, mute: bool) -> Result<(), Error>;

    fn get_media_info(&self) -> Option<&MediaInfo>;
}

pub struct YakoPlayer {
    device: Option<AudioDevice>,
    source: Option<Box<dyn AudioSource>>,
    volume: f32,
}

impl YakoPlayer {
    pub fn new() -> YakoPlayer {
        YakoPlayer {
            device: None,
            source: None,
            volume: 1.,
        }
    }
}

impl Default for YakoPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Player for YakoPlayer {
    fn init_device_defalut(&mut self) -> Result<(), Error> {
        let mut open_device = |device: &mut AudioDevice| -> Result<(), Error> {
            device.init_default_device().context(DeviceSnafu)?;
            device.set_volume(volume::volume_level_to_db(self.volume));
            device.open().context(DeviceSnafu)?;

            // 如果已经打开了播放源，重新设置动态缓冲区大小
            if let Some(source) = self.source.as_mut() {
                let device_sample_format = device.sample_format.unwrap();
                let dynamic_device_buffer_size = (device_sample_format.sample_rate as f64 * 0.08) as usize;
                source.set_dynamic_device_buffer_size(dynamic_device_buffer_size);
            }

            Ok(())
        };

        match &mut self.device {
            Some(device) => {
                open_device(device)?;
            },
            None => {
                let mut device = AudioDevice::new();
                open_device(&mut device)?;
                self.device = Some(device);
            },
        }

        Ok(())
    }

    fn open<P: AsRef<Path>>(&mut self, filepath: &P) -> Result<(), Error> {
        if self.device.is_none() || !self.device.as_ref().unwrap().is_available() {
            self.init_device_defalut().unwrap();
        }

        // TODO: 检测文件类型

        let device_sample_format = self.device.as_ref().unwrap().sample_format.unwrap();
        let dynamic_device_buffer_size = (device_sample_format.sample_rate as f64 * 0.08) as usize;

        if let Some(device) = self.device.as_ref() {
            if let Some(source) = self.source.as_mut() {
                source.close().context(SourceSnafu)?;
            }

            // TODO: 重新打开设备后缓冲区实现
            let mut source = FFmpegSource::new(
                device.get_output_buffer_producer(),
                device.get_output_buffer_consumer(),
                dynamic_device_buffer_size);
            source.open(filepath, &device.sample_format.unwrap()).context(SourceSnafu)?;
            self.source = Some(Box::new(source));
        }
        Ok(())
    }

    fn close(&mut self) -> Result<(), Error> {
        if let Some(source) = self.source.as_mut() {
            let source = &mut **source;
            source.close().context(SourceSnafu)?;
        }
        Ok(())
    }

    fn play(&mut self) -> Result<(), Error> {
        if self.device.is_none() || !self.device.as_ref().unwrap().is_available() {
            self.init_device_defalut().unwrap();
        }

        if let Some(device) = self.device.as_ref() {
            device.resume();
            if let Some(source) = self.source.as_ref() {
                source.streaming().context(SourceSnafu)?;
            }
        }
        Ok(())
    }

    fn stop(&self) -> Result<(), Error> {
        if let Some(source) = self.source.as_ref() {
            source.pause().context(SourceSnafu)?;
            source.clear_buffer();
            source.seek(0).context(SourceSnafu)?;
        }
        Ok(())
    }

    fn pause(&self) -> Result<(), Error> {
        if let Some(device) = self.device.as_ref() {
            device.pause();
            if let Some(source) = self.source.as_ref() {
                source.pause().context(SourceSnafu)?;
            }
        }
        Ok(())
    }

    fn seek(&self, time: i64) -> Result<(), Error> {
        if let Some(source) = self.source.as_ref() {
            source.seek(time).context(SourceSnafu)?;
        }
        Ok(())
    }

    fn get_bitrate(&self) -> u32 {
        match self.source.as_ref() {
            Some(source) => source.get_bitrate() as u32,
            None => 0,
        }
    }

    fn get_duration(&self) -> i64 {
        match self.source.as_ref() {
            Some(source) => source.get_duration(),
            None => 0,
        }
    }

    fn get_current_time(&self) -> i64 {
        match self.source.as_ref() {
            Some(source) => source.get_current_time(),
            None => 0,
        }
    }

    fn is_playing(&self) -> bool {
        match self.source.as_ref() {
            Some(source) => source.is_streaming(),
            None => false,
        }
    }

    fn get_volume(&self) -> f32 {
        self.volume
    }

    fn set_volume(&mut self, volume: f32) -> Result<(), Error> {
        self.volume = volume;
        if let Some(device) = self.device.as_ref() {
            device.set_volume(volume::volume_level_to_db(volume));
        }
        Ok(())
    }

    fn set_mute(&self, mute: bool) -> Result<(), Error> {
        if let Some(device) = self.device.as_ref() {
            device.set_mute(mute);
        }
        Ok(())
    }

    fn get_media_info(&self) -> Option<&MediaInfo> {
        self.source.as_ref().map(|source| source.get_media_info())
    }
}