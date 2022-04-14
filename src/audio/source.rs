extern crate ffmpeg_next as ffmpeg;
extern crate ffmpeg_sys_next as ffmpeg_c_api;

use std::cell::Cell;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Arc};
use std::sync::mpsc::{self, channel};

use ffmpeg::{codec, decoder, frame, format, media};
use ffmpeg::software::resampling::context::Context as SwrContext;
use ffmpeg::{rescale, Rescale};
use ringbuf::{Producer, Consumer};
use snafu::{Snafu, ResultExt, OptionExt};

use crate::info::media::MediaInfo;
use crate::metadata;

use super::device::{DeviceSampleFormat, AudioDevice};
use super::sample::AudioSample;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to open media file: {}", message))]
    OpenMediaFileWithFFmpeg {
        message: String,
        #[snafu(source(from(ffmpeg::Error, Box::new)))]
        source: Box<dyn std::error::Error + Send + Sync>
    },

    #[snafu(display("failed to open media file: {}", message))]
    OpenMediaFile {
        message: String,
    },

    #[snafu(display("failed to close media file: {}", message))]
    CloseMediaFile {
        message: String,
    },

    #[snafu(display("failed to seek position: {}", message))]
    Seek {
        message: String,
    },

    #[snafu(display("{}", message))]
    ChannelRecv {
        message: String,
        #[snafu(source(from(std::sync::mpsc::RecvError, Box::new)))]
        source: Box<dyn std::error::Error + Send + Sync>
    },

    #[snafu(display("{}", message))]
    SendSeek {
        message: String,
        #[snafu(source(from(std::sync::mpsc::SendError<i64>, Box::new)))]
        source: Box<dyn std::error::Error + Send + Sync>
    },
}

pub trait AudioSource {
    fn close(&mut self) -> Result<(), Error>;
    fn streaming(&self) -> Result<(), Error>;
    fn pause(&self) -> Result<(), Error>;
    fn seek(&self, time: i64) -> Result<(), Error>;
    fn clear_buffer(&self);
    fn get_duration(&self) -> i64;
    fn get_bitrate(&self) -> i64;
    fn get_current_time(&self) -> i64;
    fn set_buffer_chunk_size(&mut self, size: usize);
    fn is_end(&self) -> bool;
    fn is_streaming(&self) -> bool;
    fn set_dynamic_device_buffer_size(&self, size: usize);
    fn get_media_info(&self) -> &MediaInfo;
}

pub struct FFmpegSourceStatus {
    pub dropping_frames: AtomicBool,
    pub avaliable: AtomicBool,
    pub playing: AtomicBool,
    pub current_time: Mutex<Cell<i64>>,
    pub is_end: AtomicBool,
}

pub struct FFmpegSource {
    media_info: MediaInfo,
    seek_channel_tx: Option<mpsc::Sender<i64>>,
    decode_thread: Option<std::thread::JoinHandle<()>>,
    decode_thread_suspend_rx: Option<mpsc::Receiver<u8>>,
    pub status: Arc<FFmpegSourceStatus>,
    buffer_producer: Arc<Mutex<Producer<AudioSample>>>,
    buffer_consumer: Arc<Mutex<Consumer<AudioSample>>>,
    buffer_chunk_size: Arc<Mutex<Cell<usize>>>,
    dynamic_device_buffer_size: Arc<Mutex<Cell<usize>>>,
}

impl FFmpegSource {
    pub fn new(
        buffer_producer: &Arc<Mutex<Producer<AudioSample>>>,
        buffer_consumer: &Arc<Mutex<Consumer<AudioSample>>>,
        dynamic_device_buffer_size: usize,
    ) -> FFmpegSource {
        FFmpegSource {
            media_info: MediaInfo::default(),
            seek_channel_tx: None,
            decode_thread: None,
            decode_thread_suspend_rx: None,
            status: Arc::new(FFmpegSourceStatus { 
                dropping_frames: AtomicBool::new(false),
                avaliable: AtomicBool::new(false),
                playing: AtomicBool::new(false),
                current_time: Mutex::new(Cell::new(0)),
                is_end: AtomicBool::new(false),
            }),
            buffer_producer: buffer_producer.clone(),
            buffer_consumer: buffer_consumer.clone(),
            buffer_chunk_size: Arc::new(Mutex::new(Cell::new(dynamic_device_buffer_size / 2))),
            dynamic_device_buffer_size: Arc::new(Mutex::new(Cell::new(dynamic_device_buffer_size))),
        }
    }

    fn ffmpeg_frame_to_slice(frame: &frame::Audio) -> Vec<AudioSample> {
        if !frame.is_packed() {
            panic!("音频帧数据不是交错格式");
        }
        
        let pcm = unsafe {
            std::slice::from_raw_parts((*frame.as_ptr()).data[0] as *const f32, frame.samples() * frame.channels() as usize)
        };

        pcm.chunks_exact(frame.channels() as usize)
            .map(AudioSample::from_slice)
            .collect()
    }

    fn clear_resampler_buffer(resampler: &mut SwrContext) {
        loop {
            let mut resampled = frame::Audio::empty();
            if let Ok(delay) = resampler.flush(&mut resampled) {
                if delay.is_none() {
                    break;
                }
            } else {
                break;
            }
        }
        unsafe {
            // 关闭重采样器来释放 FIFO 里不足以写入一帧的数据
            // https://ffmpeg.org/doxygen/4.1/group__lswr.html#gaa4bf1048740dfc08d68aba9f1b4db22e
            let resampler_ptr = resampler.as_mut_ptr();
            ffmpeg_c_api::swr_close(resampler_ptr);
            ffmpeg_c_api::swr_init(resampler_ptr);
        }
    }

    fn blocking_write_buffer(
        status: &Arc<FFmpegSourceStatus>,
        chunk_size: usize,
        dynamic_device_buffer_size: usize,
        slice: &[AudioSample],
        producer: &mut ringbuf::Producer<AudioSample>
    ) {
        // 先分块，避免缓冲区容量比帧小，产生死锁
        let chunks = slice.chunks(chunk_size);
        for chunk in chunks {
            if !status.avaliable.load(Ordering::Relaxed) {
                return;
            }
            
            // 根据采样率动态调整缓冲区大小
            // producer.remaining() < chunk.len() + producer.capacity() - dynamic_device_buffer_size

            // 缓冲区满或者暂停则等待
            while producer.remaining() < chunk.len() + producer.capacity() - dynamic_device_buffer_size
                || !status.playing.load(Ordering::Relaxed) {
                if !status.avaliable.load(Ordering::Relaxed) {
                    return;
                }
                // 系统需要丢弃未写入缓冲区的帧数据
                if status.dropping_frames.load(Ordering::Relaxed) {
                    println!("丢弃不再需要的音频帧");
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            // 向缓冲区写入数据，如果需要丢弃帧数据则直接跳出循环
            if !status.dropping_frames.load(Ordering::Relaxed) {
                producer.push_slice(chunk);
            } else {
                return;
            }
        }
    }

    fn decode_to_buffer (
        status: &Arc<FFmpegSourceStatus>,
        chunck_size: &Arc<Mutex<Cell<usize>>>,
        dynamic_device_buffer_size: &Arc<Mutex<Cell<usize>>>,
        decoder: &mut decoder::Audio,
        producer: &mut ringbuf::Producer<AudioSample>,
        resampler: &mut SwrContext,
    ) -> Result<(), ffmpeg::Error> {
        let chunk_size = chunck_size.lock().unwrap().get();
        let dynamic_device_buffer_size = dynamic_device_buffer_size.lock().unwrap().get();

        let mut decoded = frame::Audio::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            if !status.avaliable.load(Ordering::Relaxed) {
                return Ok(());
            }

            let mut resampled = frame::Audio::empty();
            let mut delay = resampler.run(&decoded, &mut resampled)?;
            loop {
                if !status.avaliable.load(Ordering::Relaxed) {
                    return Ok(());
                }
                // 将重采样后的将音频数据写入对应的缓冲区中
                FFmpegSource::blocking_write_buffer(
                    status,
                    chunk_size,
                    dynamic_device_buffer_size,
                    FFmpegSource::ffmpeg_frame_to_slice(&resampled).as_slice(),
                    producer);
                // 输出的大小装不下的部分会在重采样器里缓存，需要循环读取到缓存为空
                if delay == None {
                    break;
                }
                delay = resampler.flush(&mut resampled)?;
            }
        }
        Ok(())
    }

    pub fn open<P: AsRef<Path>>(&mut self, uri: &P, device_sample_format: &DeviceSampleFormat) -> Result<(), Error> {
        // 打开文件，获取音频流
        let mut input_ctx = format::input(&uri).context(OpenMediaFileWithFFmpegSnafu {
            message: "the file could not be opened, either because the file does not exist, cannot be accessed, or the file format is not supported".to_string(),
        })?;

        // 获取专辑封面
        self.media_info.cover = metadata::ffmpeg::first_picture_from_input_context(&input_ctx);

        let stream = input_ctx.streams().best(media::Type::Audio).context(OpenMediaFileSnafu {
            message: "failed to get audio stream".to_string(),
        })?;

        let stream_index = stream.index();

        // 创建解码器
        let context = codec::context::Context::from_parameters(stream.parameters()).context(OpenMediaFileWithFFmpegSnafu {
            message: "failed to create codec context".to_string(),
        })?;
        let mut decoder = context.decoder().audio().context(OpenMediaFileWithFFmpegSnafu {
            message: "failed to create audio decoder".to_string(),
        })?;
        // 将音频流相关信息拷贝到 AVCodecContext 中
        decoder.set_parameters(stream.parameters()).context(OpenMediaFileWithFFmpegSnafu {
            message: "failed to set codec parameters".to_string(),
        })?;

        let device_channels = device_sample_format.channel_count;
        let device_sample_rate = device_sample_format.sample_rate;

        // 计算总长度（毫秒）
        let duration = input_ctx.duration() as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE) * 1000.0;
        self.media_info.duration = duration as i64;

        // 有些格式（例如 WAV）没有 channel layout
        // 重采样器会检查 input stream 的配置和输入配置是否一致
        if decoder.channel_layout().is_empty() {
            decoder.set_channel_layout(ffmpeg::ChannelLayout::default(decoder.channels().into()));
        };

        // 创建重采样器，转换音频数据为音频设备支持的格式
        let mut resampler =  SwrContext::get(
            // 输入格式
            decoder.format(),
            decoder.channel_layout(),
            decoder.rate(),
            // 输出格式 (一律使用32位浮点)
            format::Sample::F32(format::sample::Type::Packed),
            ffmpeg::ChannelLayout::default(device_channels.into()),
            device_sample_rate
        ).context(OpenMediaFileWithFFmpegSnafu {
            message: "failed to create resampler".to_string(),
        })?;

        // 用来接收解码线程退出消息的通道
        let (decode_thread_suspend_tx, decode_thread_suspend_rx) = channel::<u8>();
        self.decode_thread_suspend_rx = Some(decode_thread_suspend_rx);

        let (seek_tx, seek_rx) = channel::<i64>();
        self.seek_channel_tx = Some(seek_tx);

        let producer = self.buffer_producer.clone();
        let consumer = self.buffer_consumer.clone();

        let status = self.status.clone();
        let buffer_chunk_size = self.buffer_chunk_size.clone();
        let dynamic_device_buffer_size = self.dynamic_device_buffer_size.clone();
        self.decode_thread = Some(
            std::thread::spawn(move || {
                loop {
                    if !status.avaliable.load(Ordering::Relaxed) {
                        break;
                    }
                    // 可以实时定位的解码逻辑
                    let mut seek: Option<i64> = None;
                    loop {
                        if !status.avaliable.load(Ordering::Relaxed) {
                            break;
                        }
                        if let Some(seek_time) = seek {
                            // 更改 input_ctx 的位置，然后清除定位信息
                            if let Err(err) = input_ctx.seek(seek_time, ..seek_time) {
                                eprintln!("failed to seek: {}", err);
                            } else {
                                decoder.flush();
                                FFmpegSource::clear_resampler_buffer(&mut resampler);
                                // TODO: 解耦合
                                AudioDevice::clear_buffer(&consumer);
                            }

                            seek = None;
                        }
                        for (stream, packet) in input_ctx.packets() {
                            if !status.avaliable.load(Ordering::Relaxed) {
                                break;
                            }
                            // 已经没有音频帧了，关闭丢弃帧模式
                            status.dropping_frames.store(false, Ordering::Relaxed);

                            if let Ok(seek_time) = seek_rx.try_recv() {
                                // 如果接收到定位请求，则跳出循环
                                seek = Some(seek_time);
                                break;
                            };

                            // 阻塞暂停和停止状态（避免清除帧数据的过程中继续解码数据）
                            while !status.playing.load(Ordering::Relaxed) {
                                if !status.avaliable.load(Ordering::Relaxed) {
                                    break;
                                }
                                std::thread::sleep(std::time::Duration::from_millis(10));
                            }

                            if stream.index() == stream_index {
                                // 更新当前时间
                                packet.pts().map(|pts| {
                                    let current_time = pts as f64 * f64::from(stream.time_base()) * 1000.0;
                                    status.current_time.lock().unwrap().set(current_time as i64);
                                });

                                decoder.send_packet(&packet).unwrap();
                                FFmpegSource::decode_to_buffer(
                                    &status.clone(),
                                    &buffer_chunk_size,
                                    &dynamic_device_buffer_size,
                                    &mut decoder,
                                    &mut producer.lock().unwrap(),
                                    &mut resampler)
                                    .unwrap();
                            }
                        }
                        if seek == None {
                            // 如果没有定位信息，表示正常播放结束
                            break;
                        }
                    }
                    
                    // 播放完毕
                    status.playing.store(false, Ordering::Relaxed);
                    let current_time = status.current_time.lock().unwrap();
                    current_time.set(0);
                    std::mem::drop(current_time);

                    // TODO: 发送播放完毕的消息，程序可以决定停止播放、下一首或者单曲循环

                    status.is_end.store(true, Ordering::Relaxed);

                    loop {
                        // 文件已关闭
                        if !status.avaliable.load(Ordering::Relaxed) {
                            decode_thread_suspend_tx.send(0).unwrap();
                            return;
                        }

                        // 用户启动播放
                        if status.playing.load(Ordering::Relaxed) {
                            status.is_end.store(false, Ordering::Relaxed);
                            input_ctx.seek(0, ..0).unwrap();
                            break;
                        }
                    }                    
                }
            })
        );

        self.status.avaliable.store(true, Ordering::Relaxed);

        Ok(())
    }
}

impl AudioSource for FFmpegSource {
    fn close(&mut self) -> Result<(), Error> {
        // 结束解码线程
        self.status.clone().avaliable.store(false, Ordering::Relaxed);
        self.decode_thread_suspend_rx.as_ref()
        .context(CloseMediaFileSnafu {
            message: "no file opened".to_string(),
        })?.recv().ok();

        // TODO：清理资源
        self.decode_thread = None;
        self.seek_channel_tx = None;
        self.decode_thread_suspend_rx = None;

        AudioDevice::clear_buffer(&self.buffer_consumer);

        Ok(())
    }

    fn streaming(&self) -> Result<(), Error> {
        self.status.clone().playing.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn pause(&self) -> Result<(), Error> {
        self.status.clone().playing.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn seek(&self, time: i64) -> Result<(), Error> {
        // 相当于 time * ( 1 / 1000 ) / AV_TIME_BASE
        let time_base = time.rescale((1, 1000), rescale::TIME_BASE);
        self.seek_channel_tx.as_ref().context(SeekSnafu {
            message: "no file opened".to_string(),
        })?
        .send(time_base).context(SendSeekSnafu {
            message: "decoding thread may have terminated".to_string(),
        })?;
        let status = self.status.clone();
        status.dropping_frames.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn clear_buffer(&self) {
        self.status.clone().dropping_frames.store(true, Ordering::Relaxed);
    }

    fn get_duration(&self) -> i64 {
        self.media_info.duration
    }

    fn get_bitrate(&self) -> i64 {
        self.media_info.bitrate
    }

    fn get_current_time(&self) -> i64 {
        self.status.current_time.lock().unwrap().get()
    }

    fn set_buffer_chunk_size(&mut self, size: usize) {
        self.buffer_chunk_size.clone().lock().unwrap().set(size);
    }

    fn is_end(&self) -> bool {
        self.status.is_end.load(Ordering::Relaxed)
    }

    fn is_streaming(&self) -> bool {
        self.status.clone().playing.load(Ordering::Relaxed)
    }

    fn set_dynamic_device_buffer_size(&self, size: usize) {
        self.dynamic_device_buffer_size.clone().lock().unwrap().set(size);
        self.buffer_chunk_size.clone().lock().unwrap().set(size / 2);
    }

    fn get_media_info(&self) -> &MediaInfo {
        &self.media_info
    }
}