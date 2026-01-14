mod integers;
mod traits;

use core::fmt::Write as _;

use crate::{Format, Formatter, Str};

pub use self::integers::*;
pub use bitflags::bitflags;

// Re-export the attribute macro so downstream crates do NOT need a direct dependency on `linkme`.
#[cfg(feature = "linkme-registry")]
pub use linkme::distributed_slice;
/// Re-export the runtime crate path so `distributed_slice` can be configured to not rely on
/// `::linkme` being a direct dependency of downstream crates.
#[cfg(feature = "linkme-registry")]
pub use linkme as linkme;

/// A runtime-accessible registry entry describing a `defmt` symbol.
///
/// This is used on host targets to avoid parsing the executable (ELF/Mach-O + DWARF) at runtime.
#[cfg(feature = "linkme-registry")]
#[derive(Clone, Copy)]
#[repr(C)]
pub struct RegistryEntry {
    /// Address of the exported `static` for this defmt symbol.
    pub symbol_ptr: *const u8,
    /// The mangled symbol name (JSON) used as `#[export_name]`.
    pub raw_symbol: &'static str,
    /// Call-site file path (best-effort; meaningful for log statements).
    pub file: &'static str,
    /// Call-site line number (best-effort; meaningful for log statements).
    pub line: u32,
    /// Call-site module path (best-effort; meaningful for log statements).
    pub module: &'static str,
    /// True if this symbol was emitted for a log statement (e.g. `info!`, `warn!`, etc.).
    pub is_log_statement: bool,
}

// SAFETY: Registry entries only contain pointers to `'static` data and string literals.
#[cfg(feature = "linkme-registry")]
unsafe impl Sync for RegistryEntry {}

/// A distributed slice containing all defmt symbols in the final binary.
///
/// Populated by the defmt proc-macros (when `linkme-registry` is enabled).
#[cfg(feature = "linkme-registry")]
#[distributed_slice]
pub static DEFMT_REGISTRY: [RegistryEntry] = [..];

pub trait UnsignedInt {}
impl UnsignedInt for u8 {}
impl UnsignedInt for u16 {}
impl UnsignedInt for u32 {}
impl UnsignedInt for u64 {}
impl UnsignedInt for u128 {}

#[cfg(feature = "unstable-test")]
thread_local! {
    static I: core::sync::atomic::AtomicU16 = const { core::sync::atomic::AtomicU16::new(0) };
    static BYTES: core::cell::RefCell<Vec<u8>> = const { core::cell::RefCell::new(Vec::new()) };
}

/// For testing purposes
#[cfg(feature = "unstable-test")]
pub fn fetch_string_index() -> u16 {
    I.with(|i| i.load(core::sync::atomic::Ordering::Relaxed))
}

/// For testing purposes
#[cfg(feature = "unstable-test")]
pub fn fetch_add_string_index() -> u16 {
    I.with(|i| i.fetch_add(1, core::sync::atomic::Ordering::Relaxed))
}

/// Get and clear the logged bytes
#[cfg(feature = "unstable-test")]
pub fn fetch_bytes() -> Vec<u8> {
    BYTES.with(|b| core::mem::take(&mut *b.borrow_mut()))
}

/// Only to be used by the defmt macros
/// Safety: must be paired with a later call to release()
#[cfg(feature = "unstable-test")]
pub unsafe fn acquire() {}

/// Only to be used by the defmt macros
/// Safety: must be paired with a later call to release()
#[cfg(not(feature = "unstable-test"))]
#[inline(always)]
pub unsafe fn acquire() {
    extern "Rust" {
        fn _defmt_acquire();
    }
    _defmt_acquire()
}

/// Only to be used by the defmt macros
/// Safety: must follow an earlier call to acquire()
#[cfg(feature = "unstable-test")]
pub unsafe fn release() {}

/// Only to be used by the defmt macros
/// Safety: must follow an earlier call to acquire()
#[cfg(not(feature = "unstable-test"))]
#[inline(always)]
pub unsafe fn release() {
    extern "Rust" {
        fn _defmt_release();
    }
    _defmt_release()
}

#[cfg(feature = "unstable-test")]
pub fn write(bytes: &[u8]) {
    BYTES.with(|b| b.borrow_mut().extend(bytes))
}

#[cfg(not(feature = "unstable-test"))]
#[inline(always)]
pub fn write(bytes: &[u8]) {
    extern "Rust" {
        fn _defmt_write(bytes: &[u8]);
    }
    unsafe { _defmt_write(bytes) }
}

/// For testing purposes
#[cfg(feature = "unstable-test")]
pub fn timestamp(_fmt: crate::Formatter<'_>) {}

#[cfg(not(feature = "unstable-test"))]
#[inline(always)]
pub fn timestamp(fmt: crate::Formatter<'_>) {
    extern "Rust" {
        fn _defmt_timestamp(_: crate::Formatter<'_>);
    }
    unsafe { _defmt_timestamp(fmt) }
}

/// For bare-metal targets there is no ASLR, so the base address is always 0.
#[cfg(target_os = "none")]
fn binary_base() -> u16 {
    0
}

/// For Linux (ELF), use the linker-provided `__executable_start` symbol.
#[cfg(target_os = "linux")]
fn binary_base() -> u16 {
    extern "C" {
        static __executable_start: u8;
    }
    // SAFETY: `__executable_start` is a linker-provided symbol marking the start of the executable.
    (unsafe { core::ptr::addr_of!(__executable_start) as usize }) as u16
}

/// For macOS (Mach-O), use the linker-provided `_mh_execute_header` symbol.
#[cfg(target_os = "macos")]
fn binary_base() -> u16 {
    extern "C" {
        static _mh_execute_header: u8;
    }
    // SAFETY: `_mh_execute_header` is a linker-provided symbol marking the Mach-O header.
    (unsafe { core::ptr::addr_of!(_mh_execute_header) as usize }) as u16
}

/// For Windows, use the DOS header address from the PE format.
#[cfg(target_os = "windows")]
fn binary_base() -> u16 {
    extern "C" {
        static __ImageBase: u8;
    }
    // SAFETY: `__ImageBase` is a linker-provided symbol marking the base of the PE image.
    (unsafe { core::ptr::addr_of!(__ImageBase) as usize }) as u16
}

/// Fallback for other platforms - assume no ASLR, so the base address is always 0.
#[cfg(not(any(target_os = "none", target_os = "linux", target_os = "macos", target_os = "windows")))]
fn binary_base() -> u16 {
    0
}

/// Returns the interned string at `address`.
pub fn make_istr(address: u16) -> Str {
    Str {
        address: address.wrapping_sub(binary_base()),
    }
}

/// Computes the base-relative `u16` ID used on the wire for a given defmt symbol.
///
/// This mirrors how `defmt` derives IDs from symbol addresses (truncate to `u16` then rebase).
#[cfg(feature = "linkme-registry")]
#[inline(always)]
pub fn registry_id_from_symbol(symbol_ptr: *const u8) -> u16 {
    ((symbol_ptr as usize).wrapping_sub(binary_base() as usize)) as u16
}

/// Expose the current binary base (truncated to `u16`) used by `defmt` for ID rebasing.
#[cfg(feature = "linkme-registry")]
#[inline(always)]
pub fn registry_binary_base_u16() -> u16 {
    binary_base()
}

/// Create a Formatter.
pub fn make_formatter<'a>() -> Formatter<'a> {
    Formatter {
        _phantom: core::marker::PhantomData,
    }
}

pub fn truncate<T>(x: impl traits::Truncate<T>) -> T {
    x.truncate()
}

pub fn into_result<T: traits::IntoResult>(x: T) -> Result<T::Ok, T::Error> {
    x.into_result()
}

/// For testing purposes
#[cfg(feature = "unstable-test")]
pub fn panic() -> ! {
    panic!()
}

#[cfg(not(feature = "unstable-test"))]
#[inline(always)]
pub fn panic() -> ! {
    extern "Rust" {
        fn _defmt_panic() -> !;
    }
    unsafe { _defmt_panic() }
}

/// Implementation detail
pub fn fmt<T: Format + ?Sized>(f: &T) {
    istr(&T::_format_tag());
    f._format_data();
}

/// Implementation detail
pub fn fmt_slice<T: Format>(values: &[T]) {
    usize(&values.len());
    istr(&T::_format_tag());
    for value in values {
        value._format_data();
    }
}

/// Implementation detail
pub fn f32(b: &f32) {
    write(&f32::to_bits(*b).to_le_bytes())
}

/// Implementation detail
pub fn f64(b: &f64) {
    write(&f64::to_bits(*b).to_le_bytes())
}

/// Implementation detail
pub fn char(b: &char) {
    write(&(*b as u32).to_le_bytes())
}

pub fn str(s: &str) {
    usize(&s.len());
    write(s.as_bytes());
}

pub fn slice(s: &[u8]) {
    usize(&s.len());
    write(s);
}

// NOTE: This is passed `&[u8; N]` – it's just coerced to a slice.
pub fn u8_array(a: &[u8]) {
    write(a);
}

// NOTE: This is passed `&[u8; N]` – it's just coerced to a slice.
pub fn fmt_array<T: Format>(a: &[T]) {
    istr(&T::_format_tag());
    for value in a {
        value._format_data();
    }
}

/// Implementation detail
pub fn istr(s: &Str) {
    write(&s.address.to_le_bytes())
}

/// Implementation detail
pub fn bool(b: &bool) {
    u8(&(*b as u8));
}

/// Implementation detail
pub fn debug(val: &dyn core::fmt::Debug) {
    core::write!(FmtWrite, "{val:?}").ok();
    write(&[0xff]);
}

/// Implementation detail
pub fn display(val: &dyn core::fmt::Display) {
    core::write!(FmtWrite, "{val}").ok();
    write(&[0xff]);
}

#[inline(never)]
pub unsafe fn acquire_and_header(s: &Str) {
    acquire();
    istr(s);
    timestamp(make_formatter());
}

#[inline(never)]
pub fn acquire_header_and_release(s: &Str) {
    // safety: will be released a few lines further down
    unsafe { acquire() };
    istr(s);
    timestamp(make_formatter());
    // safety: acquire() was called a few lines above
    unsafe { release() };
}

struct FmtWrite;

impl core::fmt::Write for FmtWrite {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write(s.as_bytes());
        Ok(())
    }
}
