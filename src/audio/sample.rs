
/// 最多 8 声道的音频样本数据
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioSample {
    channels: u8,
    data: [f32; 8],
}

impl AudioSample {
    /// 从切片生成音频样本
    pub fn from_slice(slice: &[f32]) -> Self {
        let mut audio_sample = Self::default();
        audio_sample.channels = slice.len() as u8;
        audio_sample.data[..slice.len()].copy_from_slice(slice);
        audio_sample
    }

    /// 将音频样本写入切片
    pub fn write_slice(&self, slice: &mut [f32]) {
        slice.copy_from_slice(&self.data[..slice.len()]);
    }

    /// 将音频样本写入切片，并将其转换为指定格式
    pub fn write_slice_convert<T>(&self, slice: &mut [T], convert: impl Fn(f32) -> T) {
        (0..slice.len())
            .for_each(|i| slice[i] = convert(self.data[i]));
    }

    /// 将音频样本应用指定处理逻辑，并返回处理后的样本
    pub fn apply_process(&self, processor: impl Fn(f32) -> f32) -> Self {
        let mut audio_sample = self.clone();
        (0..audio_sample.channels())
            .for_each(|i| audio_sample.data[i] = processor(audio_sample.data[i]));
        audio_sample
    }

    /// 获取音频样本的声道数
    pub fn channels(&self) -> usize {
        self.channels as usize
    }

    pub fn ch1(&self) -> f32 {
        self.data[0]
    }

    pub fn ch2(&self) -> f32 {
        self.data[1]
    }

    pub fn ch3(&self) -> f32 {
        self.data[2]
    }

    pub fn ch4(&self) -> f32 {
        self.data[3]
    }

    pub fn ch5(&self) -> f32 {
        self.data[4]
    }

    pub fn ch6(&self) -> f32 {
        self.data[5]
    }

    pub fn ch7(&self) -> f32 {
        self.data[6]
    }

    pub fn ch8(&self) -> f32 {
        self.data[7]
    }
}