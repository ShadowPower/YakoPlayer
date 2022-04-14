extern crate ffmpeg_next as ffmpeg;
extern crate ffmpeg_sys_next as ffmpeg_c_api;

use ffmpeg::format;
use ffmpeg_c_api::AVPacket;

pub fn first_picture_from_input_context(input_ctx: &format::context::input::Input) -> Option<Vec<u8>> {
    input_ctx.streams()
        .into_iter()
        .filter(|stream| stream.disposition().contains(format::stream::Disposition::ATTACHED_PIC))
        .next()
        .map(|stream| {
            let picture_data = unsafe {
                let picture_packet = AVPacket::from((*stream.as_ptr()).attached_pic);
                std::slice::from_raw_parts(picture_packet.data, picture_packet.size as usize)
            };
            picture_data.to_vec()
        })
}