use slot_store::AudioOutput;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioRoute {
    pub output: AudioOutput,
    pub docked: bool,
    pub bluetooth: Option<[u8; 6]>,
}

impl AudioRoute {
    pub fn wants_bluetooth(self) -> bool {
        self.output == AudioOutput::Auto && self.bluetooth.is_some()
    }
    pub fn wants_hdmi(self) -> bool {
        self.docked && self.output != AudioOutput::Device && !self.wants_bluetooth()
    }
}

/// Discover playback endpoints rather than assuming that HDMI is card 1.
/// Card descriptions matter when the PCM itself is named only "pcm0".
pub fn playback_devices(cards: &str, pcms: &str, route: AudioRoute) -> Vec<String> {
    routed_devices(cards, pcms, route)
        .into_iter()
        .map(|device| device.name)
        .collect()
}

pub(super) struct PlaybackDevice {
    pub name: String,
    pub hdmi: bool,
    // A default alias must resolve to this onboard card; raw PCMs are explicit.
    pub codec_card: Option<u32>,
}

pub(super) fn routed_devices(cards: &str, pcms: &str, route: AudioRoute) -> Vec<PlaybackDevice> {
    if route.wants_bluetooth() {
        // Do not spill headphone audio onto speakers if the transport fails.
        // The session returns to its normal route after BlueZ reports disconnect.
        return vec![PlaybackDevice {
            name: format!(
                "bluealsa:DEV={},PROFILE=a2dp",
                crate::bluetooth::format_address(route.bluetooth.unwrap())
            ),
            hdmi: false,
            codec_card: None,
        }];
    }
    let mut card_names = std::collections::BTreeMap::<u32, String>::new();
    let mut current = None;
    for line in cards.lines() {
        if let Some((number, rest)) = line.trim().split_once('[') {
            if let Ok(number) = number.trim().parse::<u32>() {
                current = Some(number);
                card_names.insert(number, rest.to_ascii_lowercase());
                continue;
            }
        }
        if let Some(number) = current {
            card_names
                .entry(number)
                .or_default()
                .push_str(&line.to_ascii_lowercase());
        }
    }
    let mut hdmi = Vec::new();
    let mut device = Vec::new();
    let mut codec_card = None;
    for line in pcms.lines() {
        let Some((address, description)) = line.split_once(':') else {
            continue;
        };
        let Some((card, pcm)) = address.trim().split_once('-') else {
            continue;
        };
        let (Ok(card), Ok(pcm)) = (card.parse::<u32>(), pcm.parse::<u32>()) else {
            continue;
        };
        let description = description.to_ascii_lowercase();
        if !description.contains("playback") {
            continue;
        }
        let name = format!(
            "{} {description}",
            card_names.get(&card).map_or("", String::as_str)
        );
        let endpoint = format!("plughw:{card},{pcm}");
        if name.contains("hdmi") {
            hdmi.push(PlaybackDevice {
                name: endpoint,
                hdmi: true,
                codec_card: None,
            });
        } else if !name.contains("usb")
            && ["codec", "sndac", "analog", "analogue", "onboard"]
                .iter()
                .any(|word| name.contains(word))
        {
            codec_card.get_or_insert(card);
            device.push(PlaybackDevice {
                name: endpoint,
                hdmi: false,
                codec_card: None,
            });
        }
    }
    let known_hdmi = !hdmi.is_empty();
    let mut candidates = if route.wants_hdmi() { hdmi } else { Vec::new() };
    // Firmware's default PCM may contain hooks that enable the codec mixers.
    // Bypassing it with plughw can open successfully while leaving speakers mute.
    // Alsa verifies these aliases resolve to a discovered onboard card before use.
    if codec_card.is_some() {
        candidates.extend(["plug:default", "default"].map(|name| PlaybackDevice {
            name: name.to_owned(),
            hdmi: false,
            codec_card,
        }));
    }
    candidates.extend(device);
    // An unknown default is useful on older firmware, but must not accidentally
    // redirect an explicit Device choice to a known HDMI card.
    if candidates.is_empty() && !known_hdmi {
        candidates.extend(
            ["plug:default", "default", "plughw:0,0", "hw:0,0"].map(|name| PlaybackDevice {
                name: name.to_owned(),
                hdmi: false,
                codec_card: None,
            }),
        );
    }
    candidates
}
