//! Folder-defined music albums. Paths are relative to the card root, never titles.
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use symphonia::core::{
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::{Limit, MetadataOptions, StandardTagKey, StandardVisualKey},
    probe::Hint,
};

pub const MUSIC_DIR: &str = "Games/Music";
pub const SETTINGS_FILE: &str = "Config/music.json";

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub number: Option<u32>,
    pub disc: Option<u32>,
    pub duration: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Artwork {
    Local(PathBuf),
    Embedded(PathBuf),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Album {
    pub revision: u64,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub tracks: Vec<Track>,
    pub artwork: Option<Artwork>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Repeat {
    #[default]
    Off,
    Album,
    Track,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualizerStyle {
    #[default]
    Bars,
    Waveform,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MusicResume {
    pub album: PathBuf,
    pub track: PathBuf,
    pub position: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MusicSettings {
    pub shuffle: bool,
    pub repeat: Repeat,
    pub style: VisualizerStyle,
    pub keep_playing_closed: bool,
    pub resume: Option<MusicResume>,
}

impl Default for MusicSettings {
    fn default() -> Self {
        Self {
            shuffle: false,
            repeat: Repeat::Off,
            style: VisualizerStyle::Bars,
            keep_playing_closed: true,
            resume: None,
        }
    }
}

impl MusicSettings {
    pub fn read(root: &Path) -> Self {
        let mut settings: Self = std::fs::read(root.join(SETTINGS_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        if settings.resume.as_ref().is_some_and(|r| {
            !valid_path(&r.album)
                || !valid_path(&r.track)
                || !r.position.is_finite()
                || r.position < 0.0
        }) {
            settings.resume = None;
        }
        settings
    }
    pub fn write(&self, root: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(root.join("Config"))?;
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        crate::atomic_write(&root.join(SETTINGS_FILE), &bytes)
    }
}

pub fn valid_path(path: &Path) -> bool {
    path.starts_with(MUSIC_DIR) && path.components().all(|c| matches!(c, Component::Normal(_)))
}

pub fn accepts(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("mp3") || s.eq_ignore_ascii_case("flac"))
}

#[derive(Default)]
struct Tags {
    album: Option<String>,
    artist: Option<String>,
    title: Option<String>,
    number: Option<u32>,
    disc: Option<u32>,
    duration: Option<f64>,
    art: Option<Vec<u8>>,
}

fn read_tags(path: &Path, artwork: bool) -> Option<Tags> {
    let file = std::fs::File::open(path).ok()?;
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }
    let mut probe = symphonia::default::get_probe()
        .format(
            &hint,
            MediaSourceStream::new(Box::new(file), Default::default()),
            &FormatOptions::default(),
            &MetadataOptions {
                limit_metadata_bytes: Limit::Maximum(128 * 1024),
                limit_visual_bytes: Limit::Maximum(20 * 1024 * 1024),
            },
        )
        .ok()?;
    let mut tags = Tags::default();
    let metadata = probe
        .metadata
        .get()
        .and_then(|m| m.current().cloned())
        .or_else(|| probe.format.metadata().current().cloned());
    if let Some(m) = metadata {
        for tag in m.tags() {
            let value = tag.value.to_string();
            if value.trim().is_empty() {
                continue;
            }
            match tag.std_key {
                Some(StandardTagKey::Album) => tags.album = Some(value),
                Some(StandardTagKey::AlbumArtist) => tags.artist = Some(value),
                Some(StandardTagKey::Artist) if tags.artist.is_none() => tags.artist = Some(value),
                Some(StandardTagKey::TrackTitle) => tags.title = Some(value),
                Some(StandardTagKey::TrackNumber) => {
                    tags.number = value.split('/').next().and_then(|v| v.trim().parse().ok())
                }
                Some(StandardTagKey::DiscNumber) => {
                    tags.disc = value.split('/').next().and_then(|v| v.trim().parse().ok())
                }
                _ => {}
            }
        }
        let visual = m
            .visuals()
            .iter()
            .find(|v| v.usage == Some(StandardVisualKey::FrontCover))
            .or_else(|| m.visuals().first());
        if let Some(v) = visual {
            tags.art = Some(if artwork { v.data.to_vec() } else { Vec::new() });
        }
    }
    if let Some(t) = probe.format.default_track() {
        tags.duration = t
            .codec_params
            .n_frames
            .zip(t.codec_params.sample_rate)
            .map(|(n, rate)| n as f64 / rate as f64);
    }
    Some(tags)
}

pub fn embedded_art(root: &Path, track: &Path) -> Option<Vec<u8>> {
    read_tags(&root.join(track), true)?.art
}

pub fn scan(root: &Path) -> Vec<Album> {
    let mut albums = Vec::new();
    let mut pending = vec![root.join(MUSIC_DIR)];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut files = Vec::new();
        for entry in entries.flatten() {
            let p = entry.path();
            if crate::is_hidden(&p) {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                pending.push(p);
            } else if kind.is_file() {
                files.push(p);
            }
        }
        files.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
        let mut tracks = Vec::new();
        let mut album_title = None;
        let mut album_artist = None;
        let mut embedded = None;
        for file in files.iter().filter(|p| accepts(p)) {
            let tags = read_tags(file, false).unwrap_or_default();
            if album_title.is_none() {
                album_title = tags.album;
            }
            if album_artist.is_none() {
                album_artist = tags.artist;
            }
            if embedded.is_none() && tags.art.is_some() {
                embedded = file.strip_prefix(root).ok().map(Path::to_path_buf);
            }
            tracks.push(Track {
                path: file.strip_prefix(root).unwrap().to_path_buf(),
                title: tags.title.unwrap_or_else(|| {
                    file.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                }),
                number: tags.number,
                disc: tags.disc,
                duration: tags.duration.filter(|d| d.is_finite() && *d >= 0.0),
            });
        }
        if tracks.is_empty() {
            continue;
        }
        tracks.sort_by(|a, b| {
            a.disc
                .unwrap_or(1)
                .cmp(&b.disc.unwrap_or(1))
                .then(
                    a.number
                        .unwrap_or(u32::MAX)
                        .cmp(&b.number.unwrap_or(u32::MAX)),
                )
                .then_with(|| natural_cmp(&a.path.to_string_lossy(), &b.path.to_string_lossy()))
        });
        let art = ["cover.jpg", "cover.jpeg", "cover.png"]
            .iter()
            .find_map(|name| {
                files
                    .iter()
                    .find(|p| p.file_name().is_some_and(|s| s.eq_ignore_ascii_case(name)))
                    .and_then(|p| p.strip_prefix(root).ok())
                    .map(|p| Artwork::Local(p.to_path_buf()))
            })
            .or_else(|| embedded.map(Artwork::Embedded));
        let title = album_title.unwrap_or_else(|| {
            if dir == root.join(MUSIC_DIR) {
                "Music".into()
            } else {
                dir.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            }
        });
        let artist = album_artist.unwrap_or_else(|| {
            dir.parent()
                .filter(|p| *p != root.join(MUSIC_DIR) && dir != root.join(MUSIC_DIR))
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Unknown Artist".into())
        });
        let mut revision = std::collections::hash_map::DefaultHasher::new();
        for file in &files {
            file.file_name().hash(&mut revision);
            if let Ok(m) = std::fs::metadata(file) {
                m.len().hash(&mut revision);
                m.modified().ok().hash(&mut revision);
            }
        }
        albums.push(Album {
            revision: revision.finish(),
            path: dir.strip_prefix(root).unwrap().to_path_buf(),
            title,
            artist,
            tracks,
            artwork: art,
        });
    }
    albums.sort_by(|a, b| {
        natural_cmp(&a.artist, &b.artist)
            .then_with(|| natural_cmp(&a.title, &b.title))
            .then(a.path.cmp(&b.path))
    });
    albums
}

/// Compare digit runs without parsing into an integer (track names may be long).
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let (start_i, start_j) = (i, j);
            while i < a.len() && a[i].is_ascii_digit() {
                i += 1;
            }
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let (mut x, mut y) = (start_i, start_j);
            while x + 1 < i && a[x] == b'0' {
                x += 1;
            }
            while y + 1 < j && b[y] == b'0' {
                y += 1;
            }
            let order = (i - x).cmp(&(j - y)).then(a[x..i].cmp(&b[y..j]));
            if order != Ordering::Equal {
                return order;
            }
        } else {
            let order = a[i].cmp(&b[j]);
            if order != Ordering::Equal {
                return order;
            }
            i += 1;
            j += 1;
        }
    }
    a.len()
        .saturating_sub(i)
        .cmp(&b.len().saturating_sub(j))
        .then(a.cmp(b))
}
