use crate::{
    Error, Result, recovery,
    stream::{self, Consumer, Producer},
    wav::{self, Depth},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const CAPACITY: usize = 8192;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct State {
    stop: AtomicBool,
    busy: AtomicBool,
    failed: AtomicBool,
    accepted: AtomicU64,
}
#[derive(Clone)]
pub struct Control(Arc<State>);
impl Control {
    pub fn interrupt(&self) {
        self.0.failed.store(true, Ordering::SeqCst);
        self.0.stop.store(true, Ordering::SeqCst);
    }
}
struct Busy<'a>(&'a AtomicBool);
impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
pub struct Input {
    queues: Vec<Producer>,
    state: Arc<State>,
    maximum: u64,
}
impl Input {
    pub fn control(&self) -> Control {
        Control(self.state.clone())
    }
    pub fn push(&mut self, interleaved: &[f32]) -> usize {
        self.state.busy.store(true, Ordering::SeqCst);
        let _busy = Busy(&self.state.busy);
        if self.state.stop.load(Ordering::SeqCst) {
            return 0;
        }
        let channels = self.queues.len() * 2;
        if interleaved.len() > 65536 || !interleaved.len().is_multiple_of(channels) {
            self.state.failed.store(true, Ordering::SeqCst);
            self.state.stop.store(true, Ordering::SeqCst);
            return 0;
        }
        let received = self.state.accepted.load(Ordering::Relaxed);
        let requested =
            (interleaved.len() / channels).min(self.maximum.saturating_sub(received) as usize);
        let count = self
            .queues
            .iter()
            .map(Producer::available)
            .min()
            .unwrap_or(0)
            .min(requested);
        let mut block = [[0.; 2]; 512];
        for (pair, queue) in self.queues.iter_mut().enumerate() {
            for first in (0..count).step_by(512) {
                let frames = (count - first).min(512);
                for (index, output) in block[..frames].iter_mut().enumerate() {
                    for channel in 0..2 {
                        let value = interleaved[(first + index) * channels + pair * 2 + channel];
                        output[channel] = if value.is_finite() {
                            value.clamp(-1., 1.)
                        } else {
                            0.
                        };
                    }
                }
                queue.push(&block[..frames]);
            }
        }
        self.state
            .accepted
            .store(received + count as u64, Ordering::Release);
        if count < requested {
            self.state.failed.store(true, Ordering::SeqCst);
        }
        if count < requested || received + count as u64 == self.maximum {
            self.state.stop.store(true, Ordering::SeqCst);
        }
        count
    }
    pub fn interrupt(&self) {
        self.state.failed.store(true, Ordering::SeqCst);
        self.state.stop.store(true, Ordering::SeqCst);
    }
}

pub fn recover(path: &Path) -> Result<(crate::model::Sample, bool)> {
    let mut bytes = crate::storage::read(path)?;
    if bytes.len() < 44
        || &bytes[..4] != b"RIFF"
        || &bytes[8..16] != b"WAVEfmt "
        || bytes[16..20] != 16u32.to_le_bytes()
        || bytes[20..24] != [3, 0, 2, 0]
        || bytes[32..36] != [8, 0, 32, 0]
        || &bytes[36..40] != b"data"
    {
        return Err(Error::Unsupported(
            "La recuperación necesita una toma WAV float32 estéreo de Garrido Studio",
        ));
    }
    let length = (bytes.len() - 44) / 8 * 8;
    let original = u32::from_le_bytes(
        bytes[40..44]
            .try_into()
            .map_err(|_| Error::Invalid("Toma truncada"))?,
    ) as usize;
    let form = u32::from_le_bytes(
        bytes[4..8]
            .try_into()
            .map_err(|_| Error::Invalid("Toma truncada"))?,
    ) as usize;
    let repaired = original != length || bytes.len() != length + 44 || form != length + 36;
    bytes.truncate(length + 44);
    bytes[4..8].copy_from_slice(&(length as u32 + 36).to_le_bytes());
    bytes[40..44].copy_from_slice(&(length as u32).to_le_bytes());
    Ok((wav::decode(&bytes)?, repaired))
}
impl Drop for Input {
    fn drop(&mut self) {
        self.state.stop.store(true, Ordering::SeqCst);
    }
}

pub struct Take {
    pub path: PathBuf,
    pub rate: u32,
    pub frames: u64,
}
pub struct Report {
    pub takes: Vec<Take>,
    pub interrupted: bool,
    pub warning: Option<String>,
}
pub struct Recording {
    state: Arc<State>,
    handle: Option<thread::JoinHandle<Result<Report>>>,
    folder: PathBuf,
}
impl Recording {
    pub fn start(parent: &Path, rate: u32, pairs: usize, maximum: u64) -> Result<(Input, Self)> {
        if !(8000..=192000).contains(&rate)
            || !(1..=16).contains(&pairs)
            || maximum == 0
            || maximum > 4_000_000 / pairs as u64
        {
            return Err(Error::Limit(
                "Grabación: 1–16 pares y hasta cuatro millones de frames en total",
            ));
        }
        recovery::directory(parent)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Invalid("Reloj no válido"))?
            .as_nanos();
        let id = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let folder = parent.join(format!("Take-{stamp}-{}-{id}", std::process::id()));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let mut builder = builder;
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&folder)?;
        let state = Arc::new(State {
            stop: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            accepted: AtomicU64::new(0),
        });
        let mut inputs = Vec::with_capacity(pairs);
        let mut sinks = Vec::with_capacity(pairs);
        for pair in 0..pairs {
            let path = folder.join(format!("Input-{:02}.wav", pair + 1));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&path)?;
            file.write_all(&wav::encode_channels(&[], rate, Depth::Float32, false, 2)?)?;
            file.sync_all()?;
            let (tx, rx) = stream::pipe(CAPACITY)?;
            inputs.push(tx);
            sinks.push(Sink {
                file,
                path,
                queue: rx,
                frames: 0,
            });
        }
        #[cfg(target_os = "linux")]
        {
            File::open(&folder)?.sync_all()?;
            File::open(parent)?.sync_all()?;
        }
        let worker = state.clone();
        let handle = thread::Builder::new()
            .name("garrido-recording".into())
            .spawn(move || drain(sinks, worker, rate))?;
        Ok((
            Input {
                queues: inputs,
                state: state.clone(),
                maximum,
            },
            Self {
                state,
                handle: Some(handle),
                folder,
            },
        ))
    }
    pub fn done(&self) -> bool {
        self.state.stop.load(Ordering::SeqCst)
    }
    pub fn folder(&self) -> &Path {
        &self.folder
    }
    pub fn frames(&self) -> u64 {
        self.state.accepted.load(Ordering::Acquire)
    }
    pub fn interrupt(&self) {
        self.state.failed.store(true, Ordering::SeqCst);
        self.state.stop.store(true, Ordering::SeqCst);
    }
    pub fn finish(mut self) -> Result<Report> {
        self.state.stop.store(true, Ordering::SeqCst);
        self.handle
            .take()
            .ok_or(Error::Invalid("La grabación ya está cerrada"))?
            .join()
            .map_err(|_| {
                Error::Invalid("El escritor se ha detenido; se conservan las tomas en disco")
            })?
    }
}
impl Drop for Recording {
    fn drop(&mut self) {
        self.state.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
struct Sink {
    file: File,
    path: PathBuf,
    queue: Consumer,
    frames: u64,
}
impl Sink {
    fn checkpoint(&mut self) -> Result<()> {
        let length = self.file.metadata()?.len().saturating_sub(44) / 8 * 8;
        self.file.set_len(44 + length)?;
        self.file.seek(SeekFrom::Start(4))?;
        self.file.write_all(&(length as u32 + 36).to_le_bytes())?;
        self.file.seek(SeekFrom::Start(40))?;
        self.file.write_all(&(length as u32).to_le_bytes())?;
        self.file.seek(SeekFrom::End(0))?;
        self.file.sync_all()?;
        self.frames = length / 8;
        Ok(())
    }
}
fn drain(mut sinks: Vec<Sink>, state: Arc<State>, rate: u32) -> Result<Report> {
    let mut frames = [[0.; 2]; 512];
    let mut bytes = [0; 4096];
    let mut checkpoint = Instant::now();
    let checkpoint_period = Duration::from_micros(1_000_000 / sinks.len() as u64);
    let mut checkpoint_index = 0;
    let mut warning = None;
    'recording: loop {
        let mut received = false;
        for sink in &mut sinks {
            let count = sink.queue.pop_capture(&mut frames);
            if count == 0 {
                continue;
            }
            received = true;
            for (index, frame) in frames[..count].iter().enumerate() {
                bytes[index * 8..index * 8 + 4].copy_from_slice(&frame[0].to_le_bytes());
                bytes[index * 8 + 4..index * 8 + 8].copy_from_slice(&frame[1].to_le_bytes());
            }
            if let Err(error) = sink.file.write_all(&bytes[..count * 8]) {
                warning = Some(error.to_string());
                break 'recording;
            }
        }
        if state.stop.load(Ordering::SeqCst)
            && !state.busy.load(Ordering::SeqCst)
            && sinks.iter().all(|s| s.queue.available() == 0)
        {
            break;
        }
        if checkpoint.elapsed() >= checkpoint_period {
            if let Err(error) = sinks[checkpoint_index].checkpoint() {
                warning = Some(error.to_string());
                break 'recording;
            }
            checkpoint_index = (checkpoint_index + 1) % sinks.len();
            checkpoint = Instant::now();
        }
        if !received {
            thread::sleep(Duration::from_millis(1));
        }
    }
    if warning.is_some() {
        state.failed.store(true, Ordering::SeqCst);
    }
    state.stop.store(true, Ordering::SeqCst);
    let mut takes = Vec::with_capacity(sinks.len());
    for sink in &mut sinks {
        if let Err(error) = sink.checkpoint() {
            warning = Some(error.to_string());
            state.failed.store(true, Ordering::SeqCst);
        }
        takes.push(Take {
            path: sink.path.clone(),
            rate,
            frames: sink.frames,
        });
    }
    Ok(Report {
        takes,
        interrupted: state.failed.load(Ordering::SeqCst),
        warning,
    })
}
