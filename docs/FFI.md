# Native ABI review

The core library has `#![forbid(unsafe_code)]`. AppKit, AudioQueue and plugin integration remain separate native adapters. The legacy AppKit GUI is built only for macOS aarch64; Intel Objective-C aggregate return conventions are not claimed as reviewed. The common eframe GUI separately targets macOS arm64, Windows x64 and Linux x86_64.

## AppKit

State is main-thread local. Typed argument tuples fix Objective-C signatures; static selectors/classes never come from project data. C structures use `repr(C)`. NSString data is copied while alive. AppKit manages event autorelease pools. The window/delegate/timer remain associated until shutdown, and closing the last window requests termination.

State access uses `try_borrow_mut` and guarded callbacks. Native menu actions enqueue a tag for the next timer tick, avoiding nested modal callbacks being dropped while the state is borrowed. Controls restore the canvas responder for shortcuts. Drawn gestures keep baseline snapshots and commit once. A caught panic is not a successful operation and still needs diagnosis.

Custom window actions execute outside the mutable state borrow. Maximize stores/restores native frames; close requests normal application termination, allowing the delegate to retain unsaved work or a pending take. This avoids a reentrant close callback being refused only because the action already holds the state.

System home/free-space APIs supply the recovery location and reserve check. User strings are values, not selectors, code, shell arguments or file instructions.

CoreText registers only the packaged Cuprum/Tabler files in process scope. Build/audit verifies both fonts and their license files against recorded SHA256 values. Registration error objects are released through CoreFoundation; fonts are never installed system-wide. The native font parser remains part of macOS's trust boundary.

## AudioQueue output/input

Output is packed stereo float32; input is packed mono float32 at 48 kHz. System-allocated buffers are size/alignment checked. Output callbacks copy at most 512 frames; input callbacks write at most 1024 frames into the preallocated take. Neither performs file I/O, allocation or monitoring. The input write guard prevents overlapping access to its `UnsafeCell` storage. Output samples are immutable; cursors/errors/stop state are atomic.

The stable boxed context is retained until immediate synchronous disposal. A stop flag prevents further enqueues after shutdown starts. Failed disposal deliberately retains the context instead of freeing memory that a callback could reach. Startup failures with non-null queues also dispose them. Input errors preserve received frames after a successful close. Disposal/device errors still require physical testing. Queue output volume is 0.5; the system's volume is unchanged.

## CLAP

The raw ABI is `repr(C)` with nullable function pointers. Own descriptors and feature arrays contain immutable static strings. Unit tests check public structure sizes, lifecycle rejection, instrument note processing, parameter conversion and partial state streams. Audio processing uses preallocated voices/effects and atomic gain; text/state operations do not retain host buffers.

Inside a worker, library/factory/plugin pointers remain confined to that process. Module and instance guards close processing before deactivation/destruction/unloading. The parent exchanges only bounded metadata/audio, validates response headers and finite output, and kills/reaps children on cancellation, deadline, error or dropped ownership. No GUI process loads a plugin. Raw pointer validity inside a native plugin depends on its host ABI contract; an arbitrary native binary cannot be made memory-safe by signature checks alone.

The first-party-only package/hash gate and per-OS resource controls are documented in SECURITY.md. Windows uses explicit system-call declarations and repr(C) job limits; target tests check Win64 structure sizes and offsets. Linux rlimit values use the x86_64 ABI, and variadic prctl receives pointer-width arguments. The Windows job handle remains owned for the worker lifetime. The OS APIs still require actual target acceptance. No VST or external editor ABI has been implemented.

## Shared desktop

The common app contains no direct unsafe/FFI blocks. CPAL, eframe/WGPU, rfd, fs4 and their dependencies provide system integration under their original licenses. The own CLAP adapter remains confined to the separately compiled host crate and worker process. All platforms reject external modules; Windows/Linux now implement loaders and resource controls for the included plugin, with target runtime acceptance pending. Dependency wrappers do not prove that external system/driver code is memory-safe; device and platform acceptance remains required.
