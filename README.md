# RustyPlayer: An audio player built with Rust
RustyPlayer is an audio player built in pure Rust using egui for the GUI (https://crates.io/crates/egui) and rodio (https://crates.io/crates/rodio) for audio playback. It uses Symphonia (https://crates.io/crates/symphonia/0.3.0) and id3 (https://crates.io/crates/id3) to extract audio metadata like album title, artist name, and album artwork.

![image of RustyPlayer interface](https://i.imgur.com/kCOrcbW.png)
## How to run
Just run `cargo run —release` in the terminal and the app should launch!
## How it worked and what didn’t work
This was a fairly straightforward project. Rodio provided a relatively easy and performant way to play a variety of audio formats. The same can be said about egui and the GUI. However, what I had trouble with was extracting the metadata. Symphonia worked for formats that weren’t mp3 files. I tried figuring it out but I couldn’t find out what was wrong, even after following the documentation. I had to ask Claude 3.7 to help me with it, and it suggested I use the id3 crate, and that worked like a charm. Overall, this project wasn’t too complicated for me to figure out, with only a few minor hiccups, but it was fun!
## License
MIT + Apache 2.0