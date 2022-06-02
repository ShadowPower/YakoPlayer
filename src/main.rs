#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::{Duration, Instant};

use iced::{button, Alignment, Button, Column, Element, Settings, Text, Row, slider, Slider, time, Application, Command, Subscription, executor};
use rfd::FileDialog;
use player_core::{player::{YakoPlayer, Player}, audio::volume};

pub fn main() -> iced::Result {
    let open_file_path = std::env::args().nth(1);

    PlayerController::run(Settings {
        window: iced::window::Settings {
            size: (600, 130),
            resizable: false,
            ..iced::window::Settings::default()
        },
        flags: open_file_path,
        ..Settings::default()
    })
}

#[derive(Default)]
struct PlayerController {
    last_seek_time: i64,
    value: f32,
    state: State,
    duration: i64,
    current_time: i64,
    open_button: button::State,
    play_button: button::State,
    pause_button: button::State,
    stop_button: button::State,
    progress_bar_slider: slider::State,
    volume_slider: slider::State,
    volume: f32,
    player: YakoPlayer,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    OpenPressed,
    PlayPressed,
    PausePressed,
    StopPressed,
    ProgressBarChanged(f32),
    VolumeChanged(f32),
    Tick(Instant),
}

enum State {
    Playing,
    Stop,
}

impl Default for State {
    fn default() -> Self {
        Self::Stop
    }
}

impl PlayerController {
    pub fn play_from_file(&mut self, path: String) {
        match self.player.open(&path) {
            Ok(_) => {
                self.duration = self.player.get_duration();
                self.current_time = 0;
            },
            Err(err) => {
                println!("{}", err);
            }
        }
        if let Err(err) = self.player.play() {
            println!("{}", err);
        } else {
            self.state = State::Playing;
        }
    }
}

impl Application for PlayerController {
    type Message = Message;
    type Executor = executor::Default;
    type Flags = Option<String>;

    fn new(flags: Self::Flags) -> (PlayerController, iced::Command<Message>) {
        let mut controller = Self {
            volume: 1.,
            ..Default::default()
        };
        
        if let Some(path) = flags {
            controller.play_from_file(path);
        }

        (controller, Command::none())
    }

    fn title(&self) -> String {
        String::from("ShadowPlayer 2")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::OpenPressed => {
                let files = FileDialog::new()
                    .add_filter("Music", &["wav", "mp3", "flac", "ogg", "opus", "aac", "m4a", "mp4", "wma", "ape", "tak", "alac"])
                    .set_directory("/")
                    .pick_file();
                if let Some(file) = files {
                    match self.player.open(&file) {
                        Ok(_) => {
                            self.duration = self.player.get_duration();
                            self.current_time = 0;
                        },
                        Err(err) => {
                            println!("{}", err);
                        }
                    }
                    if let Err(err) = self.player.play() {
                        println!("{}", err);
                    } else {
                        self.state = State::Playing;
                    }
                }
                self.value = 0.0;
            },
            Message::PlayPressed => {
                if let Err(err) = self.player.play() {
                    println!("{}", err);
                } else {
                    self.state = State::Playing;
                }
            },
            Message::PausePressed => {
                if let Err(err) = self.player.pause() {
                    println!("{}", err);
                } else {
                    self.state = State::Stop;
                }
            },
            Message::StopPressed => {
                if let Err(err) = self.player.stop() {
                    println!("{}", err);
                } else {
                    self.state = State::Stop;
                }
                self.value = 0.0;
            },
            Message::ProgressBarChanged(value) => {
                self.value = value;
                let seek_time = (value * (self.duration as f32)) as i64;
                if seek_time != self.last_seek_time {
                    // 防抖
                    self.last_seek_time = seek_time;

                    if let Err(err) = self.player.seek(seek_time) {
                        println!("{}", err);
                    }
                }
            },
            Message::Tick(_) => match &mut self.state {
                State::Playing => {
                    self.current_time = self.player.get_current_time();
                    self.value = (self.current_time as f32) / (self.duration as f32);
                }
                _ => {}
            },
            Message::VolumeChanged(value) => {
                self.volume = value;
                self.player.set_volume(value).unwrap();
            },
        }

        Command::none()
    }

    fn view(&mut self) -> Element<Message> {
        let row: Element<Message> = Row::new()
            .spacing(4)
            .align_items(Alignment::Center)
            .push(
                Button::new(&mut self.open_button, Text::new("Open"))
                    .on_press(Message::OpenPressed),
            )
            .push(
                Button::new(&mut self.play_button, Text::new("Play"))
                    .on_press(Message::PlayPressed),
            )
            .push(
                Button::new(&mut self.pause_button, Text::new("Pause"))
                    .on_press(Message::PausePressed),
            )
            .push(
                Button::new(&mut self.stop_button, Text::new("Stop"))
                    .on_press(Message::StopPressed),
            )
            .push(Text::new(" Volume:").size(20))
            .push(Slider::new(
                    &mut self.volume_slider,
                    0.0..=1.0,
                    self.volume,
                    Message::VolumeChanged,
                )
                .step(0.01).width(iced::Length::Units(160)),)
            .push(Text::new(format!(" {:.2} dB", volume::volume_level_to_db(self.volume))).size(20))
            .into();

        Column::new()
            .padding(20)
            .spacing(6)
            .align_items(Alignment::Start)
            .push(
                Slider::new(
                    &mut self.progress_bar_slider,
                    0.0..=1.0,
                    self.value,
                    Message::ProgressBarChanged,
                )
                .step(0.01),
            )
            .push(Text::new({
                let mut s = String::from("Current time: ");
                s.push_str(format!("{:0>2}:{:0>2}", &self.current_time / 60000, (&self.current_time / 1000) % 60).as_str());
                s.push_str(", Total time: ");
                s.push_str(format!("{:0>2}:{:0>2}", &self.duration / 60000, (&self.duration / 1000) % 60).as_str());
                s
            }).size(20))
            .push(row)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        match self.state {
            State::Stop => Subscription::none(),
            State::Playing { .. } => {
                time::every(Duration::from_millis(100)).map(Message::Tick)
            }
        }
    }
}