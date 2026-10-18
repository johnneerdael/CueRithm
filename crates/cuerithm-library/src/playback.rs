//! Device interaction stays on its own thread; all codecs play the same cached PCM.
use crate::TrackId;
use anyhow::{Context, Result, ensure};
use cuerithm_media::PreparedAudio;
use rodio::{OutputStream, OutputStreamBuilder, Sink, Source, source::SeekError};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    sync::{
        Arc, Mutex,
        mpsc::{self, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

type Errors = Arc<Mutex<Option<String>>>;
pub struct CacheSource {
    audio: Arc<PreparedAudio>,
    reader: BufReader<File>,
    index: u64,
    errors: Errors,
}
impl CacheSource {
    pub fn open(audio: Arc<PreparedAudio>) -> Result<Self> {
        Self::with_errors(audio, Arc::new(Mutex::new(None)))
    }
    fn with_errors(audio: Arc<PreparedAudio>, errors: Errors) -> Result<Self> {
        let input = File::open(&audio.pcm_path)?;
        ensure!(
            input.metadata()?.len() == audio.frames * 8,
            "Playback cache is incomplete"
        );
        Ok(Self {
            audio,
            reader: BufReader::with_capacity(65536, input),
            index: 0,
            errors,
        })
    }
}
impl Iterator for CacheSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.index >= self.audio.frames * 2 {
            return None;
        }
        let mut b = [0; 4];
        if let Err(error) = self.reader.read_exact(&mut b) {
            self.index = self.audio.frames * 2;
            *self.errors.lock().unwrap() = Some(format!("Cannot read playback cache: {error}"));
            return None;
        }
        self.index += 1;
        Some(f32::from_le_bytes(b))
    }
}
impl Source for CacheSource {
    fn current_span_len(&self) -> Option<usize> {
        Some((self.audio.frames * 2 - self.index) as usize)
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        44100
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(self.audio.clock.duration_seconds()))
    }
    fn try_seek(&mut self, pos: Duration) -> std::result::Result<(), SeekError> {
        let frame = (pos.as_secs_f64() * 44100.)
            .round()
            .min(self.audio.frames as f64) as u64;
        self.reader
            .seek(SeekFrom::Start(frame * 8))
            .map_err(|e| SeekError::Other(Box::new(e)))?;
        self.index = frame * 2;
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackStatus {
    pub track_id: Option<TrackId>,
    pub duration_seconds: f64,
    pub position_seconds: f64,
    pub playing: bool,
    pub volume: f32,
    pub error: Option<String>,
}
impl Default for PlaybackStatus {
    fn default() -> Self {
        Self {
            track_id: None,
            duration_seconds: 0.,
            position_seconds: 0.,
            playing: false,
            volume: 0.65,
            error: None,
        }
    }
}
enum Action {
    Load(TrackId, Arc<PreparedAudio>),
    Play,
    Pause,
    Seek(f64),
    Volume(f32),
}
type Reply = Sender<std::result::Result<(), String>>;
enum Message {
    Action(Action, Reply),
    Stop,
}
pub struct Player {
    tx: Sender<Message>,
    status: Arc<Mutex<PlaybackStatus>>,
    join: Option<JoinHandle<()>>,
}
impl Player {
    pub fn new() -> Result<Self> {
        let errors = Arc::new(Mutex::new(None));
        let notify = errors.clone();
        Self::start(
            move || {
                let errors = notify.clone();
                let builder = OutputStreamBuilder::from_default_device()
                    .map_err(|e| anyhow::anyhow!("Audio output unavailable: {e}"))?;
                let mut output = builder
                    .with_error_callback(move |error| {
                        *errors.lock().unwrap() = Some(format!("Audio output failed: {error}"));
                    })
                    .open_stream()
                    .map_err(|e| anyhow::anyhow!("Audio output unavailable: {e}"))?;
                output.log_on_drop(false);
                Ok(output)
            },
            errors,
        )
    }
    pub fn with_output_factory<F>(factory: F) -> Result<Self>
    where
        F: FnMut() -> Result<OutputStream> + Send + 'static,
    {
        Self::start(factory, Arc::new(Mutex::new(None)))
    }
    fn start<F>(factory: F, errors: Errors) -> Result<Self>
    where
        F: FnMut() -> Result<OutputStream> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(PlaybackStatus::default()));
        let shared = status.clone();
        let join = thread::Builder::new()
            .name("cuerithm-player".into())
            .spawn(move || {
                let mut native = NativePlayer {
                    factory,
                    output: None,
                    sink: None,
                    audio: None,
                    status: PlaybackStatus::default(),
                    errors,
                };
                loop {
                    match rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(Message::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Ok(Message::Action(action, reply)) => {
                            let result = native.action(action).map_err(|e| format!("{e:#}"));
                            if let Err(ref error) = result {
                                native.status.error = Some(error.clone());
                            }
                            *shared.lock().unwrap() = native.status.clone();
                            let _ = reply.send(result);
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    native.tick();
                    *shared.lock().unwrap() = native.status.clone();
                }
            })?;
        Ok(Self {
            tx,
            status,
            join: Some(join),
        })
    }
    fn request(&self, action: Action) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(Message::Action(action, tx))
            .context("Player is closed")?;
        rx.recv()
            .context("Player stopped responding")?
            .map_err(anyhow::Error::msg)
    }
    pub fn load(&mut self, track: TrackId, audio: Arc<PreparedAudio>) -> Result<()> {
        self.request(Action::Load(track, audio))
    }
    pub fn play(&mut self) -> Result<()> {
        self.request(Action::Play)
    }
    pub fn pause(&mut self) -> Result<()> {
        self.request(Action::Pause)
    }
    pub fn seek(&mut self, seconds: f64) -> Result<()> {
        self.request(Action::Seek(seconds))
    }
    pub fn set_volume(&mut self, volume: f32) -> Result<()> {
        self.request(Action::Volume(volume))
    }
    pub fn status(&self) -> PlaybackStatus {
        self.status.lock().unwrap().clone()
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.tx.send(Message::Stop);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
struct NativePlayer<F: FnMut() -> Result<OutputStream>> {
    factory: F,
    output: Option<OutputStream>,
    sink: Option<Sink>,
    audio: Option<Arc<PreparedAudio>>,
    status: PlaybackStatus,
    errors: Errors,
}
impl<F: FnMut() -> Result<OutputStream>> NativePlayer<F> {
    fn action(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Load(track, audio) => {
                CacheSource::open(audio.clone())?;
                if let Some(sink) = self.sink.take() {
                    sink.stop();
                }
                self.status.track_id = Some(track);
                self.status.duration_seconds = audio.clock.duration_seconds();
                self.status.position_seconds = 0.;
                self.status.playing = false;
                self.status.error = None;
                *self.errors.lock().unwrap() = None;
                self.audio = Some(audio);
            }
            Action::Play => {
                let audio = self.audio.as_ref().context("Load a track first")?;
                if self.output.is_none() {
                    self.output = Some((self.factory)()?);
                }
                if self.sink.as_ref().is_none_or(Sink::empty) {
                    let sink = Sink::connect_new(self.output.as_ref().unwrap().mixer());
                    sink.pause();
                    sink.set_volume(self.status.volume);
                    sink.append(CacheSource::with_errors(
                        audio.clone(),
                        self.errors.clone(),
                    )?);
                    if self.status.position_seconds >= self.status.duration_seconds {
                        self.status.position_seconds = 0.;
                    }
                    sink.try_seek(Duration::from_secs_f64(self.status.position_seconds))
                        .map_err(|e| anyhow::anyhow!("Cannot seek: {e:?}"))?;
                    self.sink = Some(sink);
                }
                self.sink.as_ref().unwrap().play();
                self.status.playing = true;
                self.status.error = None;
            }
            Action::Pause => {
                if let Some(sink) = &self.sink {
                    sink.pause();
                    self.status.position_seconds = sink
                        .get_pos()
                        .as_secs_f64()
                        .min(self.status.duration_seconds);
                }
                self.status.playing = false;
            }
            Action::Seek(seconds) => {
                ensure!(self.audio.is_some(), "Load a track first");
                ensure!(
                    seconds.is_finite() && seconds >= 0. && seconds <= self.status.duration_seconds,
                    "Seek is outside the track"
                );
                if let Some(sink) = &self.sink {
                    sink.try_seek(Duration::from_secs_f64(seconds))
                        .map_err(|e| anyhow::anyhow!("Cannot seek: {e:?}"))?;
                }
                self.status.position_seconds = seconds;
                self.status.error = None;
            }
            Action::Volume(volume) => {
                ensure!(
                    volume.is_finite() && (0.0..=1.0).contains(&volume),
                    "Invalid volume"
                );
                self.status.volume = volume;
                if let Some(sink) = &self.sink {
                    sink.set_volume(volume);
                }
            }
        }
        Ok(())
    }
    fn tick(&mut self) {
        let error = self.errors.lock().unwrap().take();
        if let Some(error) = error {
            self.status.error = Some(error);
            self.status.playing = false;
            if let Some(sink) = self.sink.take() {
                sink.stop();
            }
            self.output = None;
            return;
        }
        if let Some(sink) = &self.sink
            && self.status.playing
        {
            self.status.position_seconds = sink
                .get_pos()
                .as_secs_f64()
                .min(self.status.duration_seconds);
            if sink.empty() {
                self.status.position_seconds = self.status.duration_seconds;
                self.status.playing = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_failure_does_not_jump_to_track_end() {
        let (mixer, _source) = rodio::mixer::mixer(2, 44100);
        let mut player = NativePlayer {
            factory: || anyhow::bail!("no device"),
            output: None,
            sink: Some(Sink::connect_new(&mixer)),
            audio: None,
            status: PlaybackStatus {
                track_id: Some("track".into()),
                duration_seconds: 64.,
                position_seconds: 2.,
                playing: true,
                ..Default::default()
            },
            errors: Arc::new(Mutex::new(Some("device disconnected".into()))),
        };
        player.tick();
        assert_eq!(player.status.position_seconds, 2.);
        assert!(!player.status.playing);
        assert!(player.status.error.is_some());
    }
}
