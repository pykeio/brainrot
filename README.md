# `brainrot`
A live chat interface for Twitch & YouTube written in Rust.

## Features
- <img src="https://www.twitch.tv/favicon.ico" width="14" /> **Twitch**
	* ⚡ Live IRC
	* 🔓 No authentication required
- <img src="https://www.youtube.com/favicon.ico" width="14" /> **YouTube**
	* 🏆 Receive chats in real time - first library to do so
	* ⚡ Low latency
	* ⏪ Supports VODs
	* 🔓 No authentication required

## Usage
See [`examples/twitch.rs`](https://github.com/vitri-ent/brainrot/blob/examples/twitch.rs) & [`examples/youtube.rs`](https://github.com/vitri-ent/brainrot/blob/examples/youtube.rs).

```shell
$ cargo run --example twitch -- sinder
Spartan_N1ck: Very Generous
luisfelipee23: GIGACHAD
wifi882: GIGACHAD
Arigreenzai: @thrillgamer2002 edennWave
notkenooooo: Im going to break into your house and toast all your bread then leave
buddy_boy_joe: @sharkboticus ah LOL fair enough sinder6Laugh sinder6Laugh sinder6Laugh
KateRosaline14: Merry Christmas
ThrillGamer2002: FirstTimeChatter
...

$ cargo run --example youtube -- "@FUWAMOCOch"
Konami Code: makes sense
Wicho4568🐾: thank you biboo
retro: Lol
GLC H 🪐: Thanks Biboo? :face-blue-smiling::FUWAhm:
Ar5eN Vines: lol
Jic: HAHAHA
Rukh 397: :FUWAhm:
PaakType: :FUWApat::MOCOpat::FUWApat::MOCOpat:
...
```
