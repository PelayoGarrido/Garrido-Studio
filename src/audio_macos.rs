use garrido_studio::{Error, Result};
use std::{
    ffi::c_void,
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering},
    },
};

type Queue = *mut c_void;
#[repr(C)]
struct Format {
    rate: f64,
    id: u32,
    flags: u32,
    bytes_packet: u32,
    frames_packet: u32,
    bytes_frame: u32,
    channels: u32,
    bits: u32,
    reserved: u32,
}
#[repr(C)]
struct Buffer {
    capacity: u32,
    data: *mut c_void,
    size: u32,
    user: *mut c_void,
    packet_capacity: u32,
    packets: *mut c_void,
    packet_count: u32,
}

#[link(name = "AudioToolbox", kind = "framework")]
unsafe extern "C" {
    fn AudioQueueNewOutput(
        format: *const Format,
        callback: unsafe extern "C" fn(*mut c_void, Queue, *mut Buffer),
        user: *mut c_void,
        run_loop: *mut c_void,
        mode: *const c_void,
        flags: u32,
        out: *mut Queue,
    ) -> i32;
    fn AudioQueueAllocateBuffer(queue: Queue, size: u32, out: *mut *mut Buffer) -> i32;
    fn AudioQueueEnqueueBuffer(
        queue: Queue,
        buffer: *mut Buffer,
        count: u32,
        descriptions: *const c_void,
    ) -> i32;
    fn AudioQueueStart(queue: Queue, time: *const c_void) -> i32;
    fn AudioQueueStop(queue: Queue, immediate: u8) -> i32;
    fn AudioQueueDispose(queue: Queue, immediate: u8) -> i32;
    fn AudioQueueSetParameter(queue: Queue, id: u32, value: f32) -> i32;
}

struct Context {
    frames: Arc<Vec<[f32; 2]>>,
    cursor: AtomicU64,
    error: AtomicI32,
    stopping: AtomicBool,
}

unsafe extern "C" fn fill(user: *mut c_void, queue: Queue, buffer: *mut Buffer) {
    if user.is_null() || buffer.is_null() {
        return;
    }
    // SAFETY: la cola conserva el contexto hasta terminar Dispose.
    let context = unsafe { &*(user as *const Context) };
    // SAFETY: AudioQueue entrega este búfer en exclusiva al callback.
    let buffer = unsafe { &mut *buffer };
    if context.stopping.load(Ordering::Acquire) {
        return;
    }
    if context.frames.is_empty()
        || buffer.data.is_null()
        || buffer.capacity < 8
        || !(buffer.data as usize).is_multiple_of(std::mem::align_of::<f32>())
    {
        context.error.store(-1, Ordering::Relaxed);
        return;
    }
    let count = (buffer.capacity / 8).min(512) as usize;
    let start = context.cursor.load(Ordering::Relaxed);
    // SAFETY: AudioQueue reserva este bloque para float32 estéreo.
    let output = unsafe { std::slice::from_raw_parts_mut(buffer.data as *mut [f32; 2], count) };
    for (i, frame) in output.iter_mut().enumerate() {
        *frame = context.frames[((start + i as u64) % context.frames.len() as u64) as usize];
    }
    buffer.size = (count * 8) as u32;
    context
        .cursor
        .store(start + count as u64, Ordering::Relaxed);
    // SAFETY: solo se devuelve el búfer de esta misma cola.
    let status = unsafe { AudioQueueEnqueueBuffer(queue, buffer, 0, ptr::null()) };
    if status != 0 {
        context.error.store(status, Ordering::Relaxed);
    }
}

pub struct Playback {
    queue: Queue,
    context: Option<Box<Context>>,
}
impl Playback {
    pub fn start(mut frames: Vec<[f32; 2]>, rate: u32) -> Result<Self> {
        if frames.is_empty()
            || frames.len() > 12_000_000
            || !(8_000..=192_000).contains(&rate)
            || frames
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 0.801)
        {
            return Err(Error::Invalid("Salida de audio no válida"));
        }
        let fade = (frames.len() / 2).min(128);
        for i in 0..fade {
            let gain = i as f32 / fade as f32;
            let last = frames.len() - 1 - i;
            frames[i][0] *= gain;
            frames[i][1] *= gain;
            frames[last][0] *= gain;
            frames[last][1] *= gain;
        }
        let mut context = Box::new(Context {
            frames: Arc::new(frames),
            cursor: AtomicU64::new(0),
            error: AtomicI32::new(0),
            stopping: AtomicBool::new(false),
        });
        let format = Format {
            rate: rate as f64,
            id: u32::from_be_bytes(*b"lpcm"),
            flags: 9,
            bytes_packet: 8,
            frames_packet: 1,
            bytes_frame: 8,
            channels: 2,
            bits: 32,
            reserved: 0,
        };
        let mut queue = ptr::null_mut();
        // SAFETY: formato C válido; contexto estable hasta Dispose.
        let status = unsafe {
            AudioQueueNewOutput(
                &format,
                fill,
                (&mut *context) as *mut Context as *mut c_void,
                ptr::null_mut(),
                ptr::null(),
                0,
                &mut queue,
            )
        };
        if status != 0 || queue.is_null() {
            if !queue.is_null() {
                drop(Self {
                    queue,
                    context: Some(context),
                });
            }
            return Err(Error::Unsupported("No se pudo abrir la salida de audio"));
        }
        let result = (|| -> Result<()> {
            for _ in 0..3 {
                let mut buffer = ptr::null_mut();
                // SAFETY: cola válida; AudioQueue reserva y alinea la memoria.
                let status = unsafe { AudioQueueAllocateBuffer(queue, 4096, &mut buffer) };
                if status != 0 || buffer.is_null() {
                    return Err(Error::Unsupported("No se pudo reservar el búfer de audio"));
                }
                // SAFETY: el callback recibe su contexto y su propio búfer.
                unsafe {
                    fill(
                        (&mut *context) as *mut Context as *mut c_void,
                        queue,
                        buffer,
                    );
                }
            }
            if context.error.load(Ordering::Relaxed) != 0 {
                return Err(Error::Unsupported("La salida ha rechazado los búferes"));
            }
            // SAFETY: volumen local de una cola válida, no volumen del sistema.
            let volume = unsafe { AudioQueueSetParameter(queue, 1, 0.5) };
            // SAFETY: los tres búferes están reservados y encolados.
            let started = if volume == 0 {
                // SAFETY: cola preparada con sus tres búferes válidos.
                unsafe { AudioQueueStart(queue, ptr::null()) }
            } else {
                volume
            };
            if started != 0 {
                return Err(Error::Unsupported("No se pudo iniciar el audio"));
            }
            Ok(())
        })();
        if let Err(error) = result {
            context.stopping.store(true, Ordering::Release);
            // SAFETY: Dispose inmediato espera a los callbacks pendientes.
            let disposed = unsafe { AudioQueueDispose(queue, 1) };
            if disposed != 0 {
                Box::leak(context);
            }
            return Err(error);
        }
        Ok(Self {
            queue,
            context: Some(context),
        })
    }
    pub fn frame(&self) -> usize {
        self.context.as_ref().map_or(0, |context| {
            context.cursor.load(Ordering::Relaxed).saturating_sub(1536) as usize
                % context.frames.len()
        })
    }
    pub fn healthy(&self) -> bool {
        self.context
            .as_ref()
            .is_some_and(|c| c.error.load(Ordering::Relaxed) == 0)
    }
}

pub fn probe() -> Result<()> {
    let playback = Playback::start(vec![[0.; 2]; 48_000], 48_000)?;
    std::thread::sleep(std::time::Duration::from_millis(180));
    if !playback.healthy() || playback.frame() == 0 {
        return Err(Error::Unsupported(
            "No se han confirmado callbacks del dispositivo",
        ));
    }
    println!("AudioQueue: callbacks activos, señal digital cero, cierre síncrono");
    drop(playback);
    Ok(())
}
impl Drop for Playback {
    fn drop(&mut self) {
        if let Some(c) = &self.context {
            c.stopping.store(true, Ordering::Release);
        }
        // SAFETY: no liberar el contexto mientras pueda entrar el callback.
        let disposed = unsafe {
            AudioQueueStop(self.queue, 1);
            AudioQueueDispose(self.queue, 1)
        };
        if disposed != 0
            && let Some(context) = self.context.take()
        {
            Box::leak(context);
        }
    }
}
