# Original music test fixtures

These files contain synthesized sine tones, generated with FFmpeg's `lavfi`
`sine` source. They contain no commercial recordings. The MP3s use libmp3lame
variable bitrate; FLACs use mono/stereo PCM with 16- and 24-bit encoding.

| File | Tone | Rate | Channels | Length |
|---|---|---|---|---|
| mono441.mp3 | 440 Hz | 44,100 Hz | 1 | 4 s |
| mono220.mp3 | 330 Hz | 22,050 Hz | 1 | 2 s |
| stereo480.flac | 880 Hz | 48,000 Hz | 2 | 4 s |
| stereo960.flac | 660 Hz | 96,000 Hz | 2 | 1 s |
| mono320.flac | 220 Hz | 32,000 Hz | 1 | 1.5 s (16-bit) |

The 44.1 kHz MP3 and 48 kHz FLAC have album/artist/title/track tags for scanning
tests. `embedded.mp3` contains an original small PNG as front-cover artwork.
Example generator: `ffmpeg -f lavfi -i sine=frequency=440:sample_rate=44100:duration=4
-ac 1 -c:a libmp3lame -q:a 4 mono441.mp3`.
