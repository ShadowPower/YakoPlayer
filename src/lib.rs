extern crate libc;
#[macro_use]
extern crate ffi_helpers;

pub mod audio;
pub mod metadata;

pub mod info;
pub mod player;

#[cfg(not(windows))]
use std::ffi::CStr;

use ffi_helpers::null_pointer_check;
use libc::c_char;
use player::{YakoPlayer, Player};

#[cfg(windows)]
use widestring::U16CStr;

ffi_helpers::export_error_handling_functions!();

#[no_mangle]
pub extern fn yako_player_new() -> *mut YakoPlayer {
    Box::into_raw(Box::new(YakoPlayer::new()))
}

#[no_mangle]
pub extern fn yako_player_free(player: *mut YakoPlayer) {
    null_pointer_check!(player);
    unsafe {
        Box::from_raw(player);
    }
}

#[no_mangle]
pub extern fn yako_player_open(player: *mut YakoPlayer, path: *const c_char) -> i32 {
    null_pointer_check!(player);
    null_pointer_check!(path);

    let player = unsafe {
        &mut *player
    };

    #[cfg(not(windows))]
    let path = unsafe {
        CStr::from_ptr(path).to_str().unwrap()
    };
        
    #[cfg(windows)]
    let path = unsafe {
        U16CStr::from_ptr_str(path as *const u16).to_string().unwrap()
    };

    match player.open(&path) {
        Ok(_) => 0,
        Err(err) => {
            ffi_helpers::update_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern fn yako_player_play(player: *mut YakoPlayer) -> i32 {
    null_pointer_check!(player);
    let player = unsafe {
        &mut *player
    };
    match player.play() {
        Ok(_) => 0,
        Err(err) => {
            ffi_helpers::update_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern fn yako_player_pause(player: *const YakoPlayer) -> i32 {
    null_pointer_check!(player);
    let player = unsafe {
        &*player
    };
    match player.pause() {
        Ok(_) => 0,
        Err(err) => {
            ffi_helpers::update_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern fn yako_player_stop(player: *const YakoPlayer) -> i32 {
    null_pointer_check!(player);
    let player = unsafe {
        &*player
    };
    match player.stop() {
        Ok(_) => 0,
        Err(err) => {
            ffi_helpers::update_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern fn yako_player_seek(player: *const YakoPlayer, position: i64) -> i32 {
    null_pointer_check!(player);
    let player = unsafe {
        &*player
    };
    match player.seek(position) {
        Ok(_) => 0,
        Err(err) => {
            ffi_helpers::update_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern fn yako_player_get_bitrate(player: *const YakoPlayer) -> u32 {
    null_pointer_check!(player);
    let player = unsafe {
        &*player
    };
    player.get_bitrate()
}

#[no_mangle]
pub extern fn yako_player_get_duration(player: *const YakoPlayer) -> i64 {
    null_pointer_check!(player);
    let player = unsafe {
        &*player
    };
    player.get_duration()
}

#[no_mangle]
pub extern fn yako_player_get_current_time(player: *const YakoPlayer) -> i64 {
    null_pointer_check!(player);
    let player = unsafe {
        &*player
    };
    player.get_current_time()
}

#[no_mangle]
pub extern fn yako_player_is_playing(player: *const YakoPlayer) -> i32 {
    null_pointer_check!(player);
    let player = unsafe {
        &*player
    };
    if player.is_playing() {
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern fn yako_player_get_volume(player: *const YakoPlayer) -> f32 {
    let player = unsafe {
        assert!(!player.is_null());
        &*player
    };
    player.get_volume()
}

#[no_mangle]
pub extern fn yako_player_set_volume(player: *mut YakoPlayer, volume: f32) -> i32 {
    null_pointer_check!(player);
    let player = unsafe {
        &mut *player
    };
    match player.set_volume(volume) {
        Ok(_) => 0,
        Err(err) => {
            ffi_helpers::update_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern fn yako_player_set_mute(player: *const YakoPlayer, mute: i32) -> i32 {
    null_pointer_check!(player);
    let player = unsafe {
        &*player
    };
    match player.set_mute(mute != 0) {
        Ok(_) => 0,
        Err(err) => {
            ffi_helpers::update_last_error(err);
            -1
        }
    }
}

#[no_mangle]
pub extern fn yako_player_get_album_cover(player: *const YakoPlayer) -> *const u8 {
    null_pointer_check!(player);
    let player = unsafe {
        &*player
    };
    match player.get_media_info() {
        Some(media_info) => {
            match media_info.cover.as_ref() {
                Some(cover) => {
                    let mut cover_bytes = cover.to_vec();
                    let cover_ptr = cover_bytes.as_mut_ptr();
                    // std::mem::forget(cover_bytes);
                    cover_ptr
                },
                None => std::ptr::null(),
            }
        },
        None => std::ptr::null(),
    }
}

#[no_mangle]
pub extern fn yako_player_get_album_cover_size(player: *const YakoPlayer) -> u32 {
    null_pointer_check!(player);
    let player = unsafe {
        &*player
    };
    match player.get_media_info() {
        Some(media_info) => {
            match media_info.cover.as_ref() {
                Some(cover) => cover.len() as u32,
                None => 0,
            }
        },
        None => 0,
    }
}

