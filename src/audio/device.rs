use std::{sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}, cell::Cell};

use cpal::{Device, Stream, SampleFormat, traits::{HostTrait, DeviceTrait, StreamTrait}, Sample};
use ringbuf::{Producer, Consumer, RingBuffer};
use snafu::{Snafu, OptionExt, ResultExt, ensure};

use super::{volume, sample::AudioSample};

pub static BUFFER_CAPACITY: usize = 64_000;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to init audio device: {}", message))]
    InitDevice {
        message: String,
    },

    #[snafu(display("failed to get supported configs: {}, {}", message, source))]
    DeviceConfig {
        message: String,
        #[snafu(source(from(cpal::SupportedStreamConfigsError, Box::new)))]
        source: Box::<dyn std::error::Error + Send + Sync>
    },

    #[snafu(display("failed to build output stream: {}", source))]
    BuildStream {
        #[snafu(source(from(cpal::BuildStreamError, Box::new)))]
        source: Box<dyn std::error::Error + Send + Sync>
    },

    #[snafu(display("failed to open device: {}", message))]
    OpenDevice {
        message: String,
    },

    #[snafu(display("failed to play output stream: {}", source))]
    PlayStream {
        #[snafu(source(from(cpal::PlayStreamError, Box::new)))]
        source: Box<dyn std::error::Error + Send + Sync>
    },

    #[snafu(display("failed to pause output stream: {}", source))]
    PauseStream {
        #[snafu(source(from(cpal::PauseStreamError, Box::new)))]
        source: Box<dyn std::error::Error + Send + Sync>
    },
}

fn audio_output_stream<T: Sample>(
    data: &mut[T],
    context: &Arc<AudioDeviceContext>,
    consumer: &Arc<Mutex<ringbuf::Consumer<AudioSample>>>,
    channels: u16,
) {
    let volume = context.volume_amplitude.lock().unwrap().get();

    let zero_frame = |frame: &mut [T]| {
        for sample in frame {
            *sample = T::from(&0.0);
        }
    };
    
    let audio_sample_write_to_frame = |frame: &mut [T], audio_sample: &AudioSample| {
        audio_sample
            .apply_process(|sample| (sample * volume).clamp(-1., 1.))
            .write_slice_convert(frame, |sample| T::from(&sample));
    };
    
    for frame in data.chunks_exact_mut(channels as usize) {
        if context.playing.load(Ordering::Relaxed) {
            let buffed_sample = consumer.lock().unwrap().pop();
            if context.mute.load(Ordering::Relaxed) {
                zero_frame(frame);
            } else {
                match buffed_sample {
                    Some(audio_sample) => {
                        audio_sample_write_to_frame(frame, &audio_sample);
                    },
                    None => {
                        zero_frame(frame);
                    }
                }
            }
        } else {
            zero_frame(frame)
        }
    }
}


/// 音频设备上下文
#[derive(Debug)]
struct AudioDeviceContext {
    /// 是否静音
    mute: AtomicBool,
    /// 输出音量增益（振幅比例）
    volume_amplitude: Mutex<Cell<f32>>,
    /// 是否消费缓冲区的数据并播放
    playing: AtomicBool,
}

/// 设备输出采样格式
#[derive(Debug, Clone, Copy)]
pub struct DeviceSampleFormat {
    /// 设备采样率
    pub sample_rate: u32,
    /// 设备音频格式
    pub sample_format: SampleFormat,
    /// 设备音频通道数
    pub channel_count: u16,
}

/// 音频设备
pub struct AudioDevice {
    /// 设备可用状态
    available: Arc<AtomicBool>,
    /// 音频设备
    device: Option<Device>,
    /// 音频输出流
    output_stream: Option<Stream>,
    /// 音频输出缓冲区：生产者
    output_buffer_producer: Arc<Mutex<Producer<AudioSample>>>,
    /// 音频输出缓冲区：消费者
    output_buffer_consumer: Arc<Mutex<Consumer<AudioSample>>>,
    /// 设备输出采样格式
    pub sample_format: Option<DeviceSampleFormat>,
    /// 音频设备上下文
    context: Arc<AudioDeviceContext>
}

impl AudioDevice {
    pub fn new() -> AudioDevice {
        // 创建音频缓冲区
        let buffer = RingBuffer::<AudioSample>::new(BUFFER_CAPACITY);
        let (producer, consumer) = buffer.split();

        AudioDevice {
            available: Arc::new(AtomicBool::new(false)),
            output_buffer_producer: Arc::new(Mutex::new(producer)),
            output_buffer_consumer: Arc::new(Mutex::new(consumer)),
            device: None,
            output_stream: None,
            sample_format: None,
            context: Arc::new(AudioDeviceContext {
                mute: AtomicBool::new(false),
                volume_amplitude: Mutex::new(Cell::new(0.0)),
                playing: AtomicBool::new(true),
            }),
        }
    }

    /// 初始化默认音频设备
    pub fn init_default_device(&mut self) -> Result<(), Error> {
        let device = cpal::default_host()
            .default_output_device()
            .context(InitDeviceSnafu {
                message: "failed to get default output device".to_string(),
            })?;

        let supported_config_range = device.supported_output_configs()
            .context(DeviceConfigSnafu {
                message: "failed to get supported output configs".to_string(),
            })?
            .next()
            .context(InitDeviceSnafu {
                message: "the audio device does not have a supported output format".to_string(),
            })?;

        // 获取最高采样率的输出格式
        let device_config = supported_config_range.with_max_sample_rate();

        self.sample_format = Some(DeviceSampleFormat {
            sample_rate: device_config.sample_rate().0,
            sample_format: device_config.sample_format(),
            channel_count: device_config.channels(),
        });

        // 创建音频设备输出流，从缓冲区读取数据
        let device_avaliabled = self.available.clone();
        let error_callback = move |err| {
            eprintln!("An error occurred while playing the audio: {}", err);
            // 标记设备已经失效
            device_avaliabled.store(false, Ordering::Release);
        };

        let consumer_f32 = self.output_buffer_consumer.clone();
        let status = self.context.clone();
        let channels = device_config.channels();
        let device_output_stream = match &device_config.sample_format() {
            SampleFormat::I16 => device.build_output_stream(&device_config.into(), move |data: &mut[i16], _| {
                audio_output_stream(data, &status, &consumer_f32, channels);
            }, error_callback),
            SampleFormat::U16 => device.build_output_stream(&device_config.into(), move |data: &mut[u16], _| {
                audio_output_stream(data, &status, &consumer_f32, channels);
            }, error_callback),
            SampleFormat::F32 => device.build_output_stream(&device_config.into(), move |data: &mut[f32], _| {
                audio_output_stream(data, &status, &consumer_f32, channels);
            }, error_callback),
        }.context(BuildStreamSnafu)?;

        self.device = Some(device);
        self.output_stream = Some(device_output_stream);
        // 标记设备可用
        let device_avaliabled = self.available.clone();
        device_avaliabled.store(true, Ordering::Release);

        Ok(())
    }

    /// 开始音频输出
    pub fn open(&self) -> Result<(), Error> {
        ensure!(self.is_available(), OpenDeviceSnafu {
            message: "the audio device is not available".to_string(),
        });

        self.output_stream.as_ref()
            .context(OpenDeviceSnafu {
                message: "audio device has not been initialized".to_string(),
            })?
            .play()
            .context(PlayStreamSnafu)?;
        Ok(())
    }

    /// 不关闭音频设备，暂停音频输出
    pub fn pause(&self) {
        self.context.clone().playing.store(false, Ordering::Relaxed);
    }

    /// 取消暂停音频输出
    pub fn resume(&self) {
        self.context.clone().playing.store(true, Ordering::Relaxed);
    }

    /// 停止音频输出
    pub fn close(&self) -> Result<(), Error> {
        if self.is_available() {
            self.output_stream.as_ref()
            .context(OpenDeviceSnafu {
                message: "audio device has not been initialized".to_string(),
            })?
            .pause()
            .context(PauseStreamSnafu)?;
        }
        todo!()
    }

    /// 获取音频输出缓冲区生产者
    pub fn get_output_buffer_producer(&self) -> &Arc<Mutex<Producer<AudioSample>>> {
        &self.output_buffer_producer
    }

    /// 获取音频输出缓冲区消费者
    pub fn get_output_buffer_consumer(&self) -> &Arc<Mutex<Consumer<AudioSample>>> {
        &self.output_buffer_consumer
    }

    /// 清空音频输出缓冲区
    pub fn clear_output_buffer(&self) {
        self.output_buffer_consumer.lock().unwrap().discard(BUFFER_CAPACITY);
    }

    /// 判断设备是否可用
    pub fn is_available(&self) -> bool {
        self.available.clone().load(Ordering::Acquire)
    }

    /// 改变输出音量
    pub fn set_volume(&self, db_gain: f32) {
        let amplitude = if db_gain == 0. { 1. } else { volume::db_gain_to_amplitude(db_gain) };
        self.context.clone().volume_amplitude.lock().unwrap().set(amplitude);
    }

    /// 开关静音
    pub fn set_mute(&self, mute: bool) {
        self.context.clone().mute.store(mute, Ordering::Relaxed);
    }

    /// 清除指定的缓冲区
    pub fn clear_buffer(buffer_consumer: &Arc<Mutex<Consumer<AudioSample>>>) {
        buffer_consumer.lock().unwrap().discard(BUFFER_CAPACITY);
    }
}