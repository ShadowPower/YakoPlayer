/// 改变音量
pub fn change_volume_db(sample: f32, db_gain: f32) -> f32 {
    (sample * db_gain_to_amplitude(db_gain)).clamp(-1., 1.)
}

/// 分贝转振幅比例
pub fn db_gain_to_amplitude(db_gain: f32) -> f32 {
    10f32.powf(db_gain * 0.05)
}

/// 转换音量等级到分贝
/// 
/// 设 a = 最低分贝，b = 系数
/// 
/// 基础公式：y=a𝑒^(-b𝑥)
/// 
/// 其中，𝑥 取值从 0 到正无穷
/// 
/// 为了让输入取值从 0 到 1，将 𝑥 = 1 代入公式，求出 C，即 C = a𝑒^−b
/// 
/// 再将公式改为：y=(a+C)𝑒^(−b𝑥)-C
pub fn volume_level_to_db(volume: f32) -> f32 {
    if volume == 1. {
        0.
    } else {
        let lowest_db: f64 = -100.;
        let coefficient: f64 = 4.397;
        let c = lowest_db * (-coefficient).exp();
        ((lowest_db + c) * (-coefficient * volume as f64).exp() - c).clamp(f64::MIN, 0.) as f32
    }
}