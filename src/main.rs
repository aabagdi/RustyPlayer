use eframe::egui;
use id3::TagLike;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

struct AudioPlayer {
    sink: Option<Arc<Mutex<Sink>>>,
    _stream: OutputStream,
    _stream_handle: rodio::OutputStreamHandle,
    current_file: Option<PathBuf>,
    current_reader: Option<Arc<Mutex<BufReader<File>>>>,
    volume: f32,
    is_playing: bool,
    duration: Option<Duration>,
    position: Duration,
    seek_position: f32,
    last_update: Option<Instant>,
    repeat: bool,
    album_art: Option<egui::TextureHandle>,
    title: String,
    artist: String,
    album: String,
}

impl AudioPlayer {
    fn new() -> Self {
        let (stream, stream_handle) = OutputStream::try_default().unwrap();
        AudioPlayer {
            sink: None,
            _stream: stream,
            _stream_handle: stream_handle,
            current_file: None,
            current_reader: None,
            volume: 1.0,
            is_playing: false,
            duration: None,
            position: Duration::from_secs(0),
            seek_position: 0.0,
            last_update: None,
            repeat: false,
            album_art: None,
            title: String::from("No track loaded"),
            artist: String::from(""),
            album: String::from(""),
        }
    }

    fn load_file(&mut self, path: PathBuf, ctx: &egui::Context) {
        self.sink = None;

        if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            self.current_reader = Some(Arc::new(Mutex::new(reader)));

            if let Ok(file) = File::open(&path) {
                let reader = BufReader::new(file);
                if let Ok(decoder) = Decoder::new(reader) {
                    if let Some(duration) = decoder.total_duration() {
                        self.duration = Some(duration + Duration::from_secs(1));
                    }
                }
            }

            self.extract_metadata_and_art(&path, ctx);

            self.current_file = Some(path);
            self.position = Duration::from_secs(0);
            self.seek_position = 0.0;

            self.start_playback();

            self.is_playing = true;

            self.last_update = Some(Instant::now());

            if self.is_playing {
                if let Some(sink) = &self.sink {
                    let sink = sink.lock().unwrap();
                    sink.play();
                }
            }
        }
    }

    fn extract_metadata_and_art(&mut self, path: &Path, ctx: &egui::Context) {
        self.title = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();
        self.artist = String::from("Unknown Artist");
        self.album = String::from("Unknown Album");
        self.album_art = None;

        if let Some(extension) = path.extension() {
            if extension.to_string_lossy().to_lowercase() == "mp3" {
                if let Ok(tag) = id3::Tag::read_from_path(path) {
                    if let Some(title) = tag.title() {
                        self.title = title.to_string();
                    }

                    if let Some(artist) = tag.artist() {
                        self.artist = artist.to_string();
                    }

                    if let Some(album) = tag.album() {
                        self.album = album.to_string();
                    }

                    if let Some(picture) = tag.pictures().next() {
                        if let Ok(img) = image::load_from_memory(&picture.data) {
                            let rgba_image = img.to_rgba8();
                            let pixels = rgba_image.as_flat_samples();

                            let size = [rgba_image.width() as _, rgba_image.height() as _];
                            self.album_art = Some(ctx.load_texture(
                                "album_art",
                                egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()),
                                Default::default(),
                            ));
                        }
                    }

                    return;
                }
            }
        }

        if let Ok(file) = File::open(path) {
            let media_source = MediaSourceStream::new(Box::new(file), Default::default());

            let mut hint = Hint::new();
            if let Some(extension) = path.extension() {
                if let Some(ext_str) = extension.to_str() {
                    hint.with_extension(ext_str);
                }
            }

            let format_opts = FormatOptions::default();
            let metadata_opts = MetadataOptions::default();

            if let Ok(probed) = symphonia::default::get_probe().format(
                &hint,
                media_source,
                &format_opts,
                &metadata_opts,
            ) {
                let mut format = probed.format;

                if let Some(metadata_rev) = format.metadata().current() {
                    for tag in metadata_rev.tags() {
                        match tag.std_key {
                            Some(symphonia::core::meta::StandardTagKey::TrackTitle) => {
                                self.title = tag.value.to_string();
                            }
                            Some(symphonia::core::meta::StandardTagKey::Artist) => {
                                self.artist = tag.value.to_string();
                            }
                            Some(symphonia::core::meta::StandardTagKey::Album) => {
                                self.album = tag.value.to_string();
                            }
                            _ => {}
                        }
                    }

                    let visuals = metadata_rev.visuals();
                    if !visuals.is_empty() {
                        let visual = &visuals[0];

                        if let Ok(img) = image::load_from_memory(&visual.data) {
                            let rgba_image = img.to_rgba8();
                            let pixels = rgba_image.as_flat_samples();

                            let size = [rgba_image.width() as _, rgba_image.height() as _];
                            self.album_art = Some(ctx.load_texture(
                                "album_art",
                                egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()),
                                Default::default(),
                            ));
                        }
                    }
                }
            }
        }
    }

    fn start_playback(&mut self) {
        if let Some(reader) = &self.current_reader {
            let reader = reader.lock().unwrap();

            if let Ok(decoder) = Decoder::new(BufReader::new(reader.get_ref().try_clone().unwrap()))
            {
                let sink = Sink::try_new(&self._stream_handle).unwrap();
                sink.append(decoder);
                sink.set_volume(self.volume);

                if !self.is_playing {
                    sink.pause();
                }

                self.sink = Some(Arc::new(Mutex::new(sink)));
            }
        }
    }

    fn seek(&mut self, position: f32) {
        if let Some(duration) = self.duration {
            let seek_position = Duration::from_secs_f32(position * duration.as_secs_f32());

            if let Some(sink) = &self.sink {
                let sink = sink.lock().unwrap();

                let prev_volume = self.volume;
                sink.set_volume(0.0);
                sink.pause();

                if sink.try_seek(seek_position).is_ok() {
                    self.position = seek_position;
                    if self.is_playing {
                        sink.play();
                        self.last_update = Some(Instant::now());
                    }
                }

                std::thread::sleep(Duration::from_millis(50));
                sink.set_volume(prev_volume);
            }
        }
    }

    fn play_pause(&mut self) {
        if let Some(duration) = self.duration {
            if self.position >= duration {
                self.reset_playback();
            }
        }

        if let Some(sink) = &self.sink {
            let sink = sink.lock().unwrap();
            if self.is_playing {
                sink.pause();
                self.last_update = None;
            } else {
                if sink.empty() {
                    drop(sink);
                    self.reset_playback();
                    if let Some(new_sink) = &self.sink {
                        let new_sink = new_sink.lock().unwrap();
                        new_sink.play();
                    }
                } else {
                    sink.play();
                }
                self.last_update = Some(Instant::now());
            }
            self.is_playing = !self.is_playing;
        }
    }

    fn set_volume(&mut self, volume: f32) {
        if let Some(sink) = &self.sink {
            let sink = sink.lock().unwrap();
            sink.set_volume(volume);
            self.volume = volume;
        }
    }

    fn reset_playback(&mut self) {
        self.sink = None;
        self.position = Duration::from_secs(0);
        self.seek_position = 0.0;
        self.last_update = None;
        self.is_playing = false;

        if let Some(path) = &self.current_file {
            if let Ok(file) = File::open(path) {
                let reader = BufReader::new(file);
                self.current_reader = Some(Arc::new(Mutex::new(reader)));
                self.start_playback();
            }
        }
    }

    fn update_position(&mut self) {
        if self.is_playing {
            if let Some(duration) = self.duration {
                let now = Instant::now();
                let elapsed = if let Some(last) = self.last_update {
                    now.duration_since(last)
                } else {
                    self.last_update = Some(now);
                    Duration::from_secs(0)
                };
                self.last_update = Some(now);

                self.position += elapsed;

                if self.position >= duration {
                    if self.repeat {
                        self.reset_playback();
                        if let Some(sink) = &self.sink {
                            let sink = sink.lock().unwrap();
                            sink.play();
                        }
                        self.is_playing = true;
                        self.last_update = Some(Instant::now());
                    } else {
                        self.is_playing = false;
                        self.position = Duration::from_secs(0);
                        self.seek_position = 0.0;
                        self.last_update = None;

                        if let Some(sink) = &self.sink {
                            let sink = sink.lock().unwrap();
                            sink.pause();
                        }
                        self.reset_playback();
                    }
                } else {
                    self.seek_position = self.position.as_secs_f32() / duration.as_secs_f32();

                    if let Some(sink) = &self.sink {
                        let sink = sink.lock().unwrap();
                        if sink.empty() && self.is_playing {
                            drop(sink);
                            if self.repeat {
                                self.reset_playback();
                                if let Some(new_sink) = &self.sink {
                                    let new_sink = new_sink.lock().unwrap();
                                    new_sink.play();
                                }
                                self.is_playing = true;
                                self.last_update = Some(Instant::now());
                            } else {
                                self.reset_playback();
                                self.is_playing = false;
                            }
                        }
                    }
                }
            }
        }
    }

    fn format_duration(duration: Duration) -> String {
        let secs = duration.as_secs();
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }
}

impl eframe::App for AudioPlayer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_position();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("RustyPlayer");

            ui.columns(2, |columns| {
                columns[0].with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    if ui.button("Open File").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Audio", &["mp3", "wav", "ogg", "flac"])
                            .pick_file()
                        {
                            self.load_file(path, ctx);
                        }
                    }

                    if let Some(path) = &self.current_file {
                        ui.label(format!(
                            "File: {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ));
                    }

                    ui.add_space(10.0);
                    ui.heading(&self.title);
                    ui.label(format!("Artist: {}", self.artist));
                    ui.label(format!("Album: {}", self.album));

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        if ui.button(if self.is_playing { "⏸" } else { "▶" }).clicked() {
                            self.play_pause();
                        }

                        let repeat_button = egui::Button::new("Repeat").fill(if self.repeat {
                            egui::Color32::from_rgb(100, 150, 255)
                        } else {
                            ui.style().visuals.widgets.inactive.bg_fill
                        });

                        if ui.add(repeat_button).clicked() {
                            self.repeat = !self.repeat;
                        }

                        ui.add(
                            egui::Slider::new(&mut self.volume, 0.0..=1.0)
                                .text("Volume")
                                .show_value(true),
                        );
                        self.set_volume(self.volume);
                    });

                    ui.add_space(10.0);

                    if let Some(duration) = self.duration {
                        ui.horizontal(|ui| {
                            ui.label(Self::format_duration(self.position));

                            let mut seek_pos = self.seek_position;
                            ui.spacing_mut().slider_width = ui.available_width() - 80.0;
                            if ui
                                .add(egui::Slider::new(&mut seek_pos, 0.0..=1.0).show_value(false))
                                .changed()
                            {
                                self.seek(seek_pos);
                            }

                            ui.label(Self::format_duration(duration));
                        });
                    }
                });

                columns[1].with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    let available_size = ui.available_size();
                    let art_size =
                        egui::Vec2::new(available_size.x.min(200.0), available_size.x.min(200.0));

                    if let Some(texture) = &self.album_art {
                        ui.add(egui::Image::new(texture).fit_to_exact_size(art_size));
                    } else {
                        let rect = ui.allocate_space(art_size).1;
                        ui.painter()
                            .rect_filled(rect, 5.0, egui::Color32::from_rgb(60, 60, 60));

                        let font_id = egui::FontId::proportional(32.0);
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "🎵",
                            font_id,
                            egui::Color32::from_rgb(180, 180, 180),
                        );
                    }
                });
            });
        });

        ctx.request_repaint();
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "RustyPlayer",
        options,
        Box::new(|_cc| Box::new(AudioPlayer::new())),
    )
}
