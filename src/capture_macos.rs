use garrido_studio::{Error, Result, model::Sample};
use std::{
    cell::UnsafeCell,
    ffi::c_void,
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering},
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
    fn AudioQueueNewInput(
        format: *const Format,
        callback: unsafe extern "C" fn(
            *mut c_void,
            Queue,
            *mut Buffer,
            *const c_void,
            u32,
            *const c_void,
        ),
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
        packets: *const c_void,
    ) -> i32;
    fn AudioQueueStart(queue: Queue, time: *const c_void) -> i32;
    fn AudioQueueStop(queue: Queue, immediate: u8) -> i32;
    fn AudioQueueDispose(queue: Queue, immediate: u8) -> i32;
}
struct Context {
    frames: UnsafeCell<Vec<[f32; 2]>>,
    count: AtomicUsize,
    error: AtomicI32,
    limit: usize,
    writing: AtomicBool,
    stopping: AtomicBool,
}
unsafe extern "C" fn input(
    user: *mut c_void,
    queue: Queue,
    buffer: *mut Buffer,
    _time: *const c_void,
    _packets: u32,
    _description: *const c_void,
) {
    if user.is_null() || buffer.is_null() {
        return;
    }
    // SAFETY: la cola retiene el contexto y entrega un búfer exclusivo.
    let (context, buffer) = unsafe { (&*(user as *const Context), &mut *buffer) };
    if context.stopping.load(Ordering::Acquire) {
        return;
    }
    if buffer.data.is_null()
        || buffer.size > buffer.capacity
        || buffer.size > 4096
        || !buffer.size.is_multiple_of(4)
        || !(buffer.data as usize).is_multiple_of(std::mem::align_of::<f32>())
    {
        context.error.store(-1, Ordering::Relaxed);
        return;
    }
    if context
        .writing
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        context.error.store(-2, Ordering::Relaxed);
        return;
    }
    let start = context.count.load(Ordering::Relaxed);
    let count = (buffer.size as usize / 4).min(context.limit.saturating_sub(start));
    // SAFETY: AudioQueue produce float32 mono; solo este callback escribe frames.
    let (source, target) = unsafe {
        (
            std::slice::from_raw_parts(buffer.data as *const f32, count),
            &mut *context.frames.get(),
        )
    };
    for (i, &value) in source.iter().enumerate() {
        let v = if value.is_finite() {
            value.clamp(-1., 1.)
        } else {
            0.
        };
        target[start + i] = [v, v];
    }
    context.count.store(start + count, Ordering::Release);
    context.writing.store(false, Ordering::Release);
    if start + count < context.limit {
        // SAFETY: se devuelve el mismo búfer a su cola de entrada.
        let status = unsafe { AudioQueueEnqueueBuffer(queue, buffer, 0, ptr::null()) };
        if status != 0 {
            context.error.store(status, Ordering::Relaxed);
        }
    }
}
pub struct Capture {
    queue: Queue,
    context: Option<Box<Context>>,
}
impl Capture {
    pub fn start() -> Result<Self> {
        let mut context = Box::new(Context {
            frames: UnsafeCell::new(vec![[0.; 2]; 48_000 * 30]),
            count: AtomicUsize::new(0),
            error: AtomicI32::new(0),
            limit: 48_000 * 30,
            writing: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
        });
        let mut queue = ptr::null_mut();
        let format = Format {
            rate: 48_000.,
            id: u32::from_be_bytes(*b"lpcm"),
            flags: 9,
            bytes_packet: 4,
            frames_packet: 1,
            bytes_frame: 4,
            channels: 1,
            bits: 32,
            reserved: 0,
        };
        // SAFETY: formato C y contexto estable hasta Dispose inmediato.
        let status = unsafe {
            AudioQueueNewInput(
                &format,
                input,
                (&mut *context) as *mut Context as *mut c_void,
                ptr::null_mut(),
                ptr::null(),
                0,
                &mut queue,
            )
        };
        if status != 0 || queue.is_null() {
            if !queue.is_null() {
                let mut failed = Self {
                    queue,
                    context: Some(context),
                };
                failed.close()?;
            }
            return Err(Error::Unsupported(
                "Entrada no disponible. Comprueba el permiso de micrófono en macOS",
            ));
        }
        let mut capture = Self {
            queue,
            context: Some(context),
        };
        for _ in 0..3 {
            let mut buffer = ptr::null_mut();
            // SAFETY: cola válida; el sistema reserva y alinea el búfer.
            let status = unsafe { AudioQueueAllocateBuffer(queue, 4096, &mut buffer) };
            if status != 0 || buffer.is_null() {
                return Err(Error::Unsupported("No se pudo reservar la entrada"));
            }
            // SAFETY: búfer de esta cola; aún no se procesa audio.
            if unsafe { AudioQueueEnqueueBuffer(queue, buffer, 0, ptr::null()) } != 0 {
                return Err(Error::Unsupported("No se pudo preparar la entrada"));
            }
        }
        // SAFETY: búferes preparados; solo se llama tras pulsar Grabar.
        if unsafe { AudioQueueStart(queue, ptr::null()) } != 0 {
            capture.close()?;
            return Err(Error::Unsupported("No se pudo iniciar la grabación"));
        }
        Ok(capture)
    }
    pub fn done(&self) -> bool {
        self.context.as_ref().is_none_or(|c| {
            c.count.load(Ordering::Acquire) >= c.limit || c.error.load(Ordering::Relaxed) != 0
        })
    }
    fn close(&mut self) -> Result<()> {
        if self.queue.is_null() {
            return Ok(());
        }
        if let Some(c) = &self.context {
            c.stopping.store(true, Ordering::Release);
        }
        // SAFETY: Dispose inmediato espera a que termine el callback.
        let status = unsafe {
            AudioQueueStop(self.queue, 1);
            AudioQueueDispose(self.queue, 1)
        };
        self.queue = ptr::null_mut();
        if status != 0 {
            if let Some(c) = self.context.take() {
                Box::leak(c);
            }
            return Err(Error::Unsupported(
                "No se pudo cerrar la entrada. Reinicia Garrido Studio",
            ));
        }
        Ok(())
    }
    pub fn finish(mut self) -> Result<(Sample, bool)> {
        self.close()?;
        let c = self
            .context
            .take()
            .ok_or(Error::Invalid("Grabación no disponible"))?;
        let interrupted = c.error.load(Ordering::Relaxed) != 0;
        let count = c.count.load(Ordering::Acquire);
        if count == 0 {
            return Err(Error::Invalid("No se ha recibido audio"));
        }
        let mut frames = c.frames.into_inner();
        frames.truncate(count);
        Ok((
            Sample {
                rate: 48_000,
                frames: Arc::new(frames),
            },
            interrupted,
        ))
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interrupted_take_keeps_received_frames() {
        let capture = Capture {
            queue: ptr::null_mut(),
            context: Some(Box::new(Context {
                frames: UnsafeCell::new(vec![[0.25; 2]; 4]),
                count: AtomicUsize::new(2),
                error: AtomicI32::new(-1),
                limit: 4,
                writing: AtomicBool::new(false),
                stopping: AtomicBool::new(false),
            })),
        };
        let (sample, interrupted) = capture.finish().unwrap();
        assert!(interrupted);
        assert_eq!(sample.frames.as_ref(), &vec![[0.25; 2]; 2]);
    }
    fn run(source: &[f32], capacity: u32) -> (Vec<[f32; 2]>, usize, i32) {
        let context = Context {
            frames: UnsafeCell::new(vec![[0.; 2]; 4]),
            count: AtomicUsize::new(0),
            error: AtomicI32::new(0),
            limit: 4,
            writing: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
        };
        let mut buffer = Buffer {
            capacity,
            data: source.as_ptr() as *mut c_void,
            size: source.len() as u32 * 4,
            user: ptr::null_mut(),
            packet_capacity: 0,
            packets: ptr::null_mut(),
            packet_count: 0,
        };
        // SAFETY: memoria propia alineada; el límite evita reenviar a una cola real.
        unsafe {
            input(
                (&context as *const Context).cast_mut().cast(),
                ptr::null_mut(),
                &mut buffer,
                ptr::null(),
                0,
                ptr::null(),
            );
        }
        let count = context.count.load(Ordering::Acquire);
        let error = context.error.load(Ordering::Relaxed);
        (context.frames.into_inner(), count, error)
    }
    #[test]
    fn input_is_finite_stereo_and_bounded() {
        let (frames, count, error) = run(&[0.25, -2., f32::NAN, 0.5], 16);
        assert_eq!(count, 4);
        assert_eq!(error, 0);
        assert_eq!(frames, vec![[0.25; 2], [-1.; 2], [0.; 2], [0.5; 2]]);
    }
    #[test]
    fn input_stops_at_preallocated_budget() {
        let (frames, count, error) = run(&[0.5; 8], 32);
        assert_eq!(count, 4);
        assert_eq!(frames.len(), 4);
        assert_eq!(error, 0);
    }
    #[test]
    fn malformed_driver_buffer_is_rejected() {
        let (_, count, error) = run(&[0.5; 4], 8);
        assert_eq!(count, 0);
        assert_ne!(error, 0);
    }
}
