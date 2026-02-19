//! Frozen C ABI for codec plugins.
//!
//! A plugin is a shared library that exports one symbol:
//!
//! ```c
//! const SixcyCodecPlugin *sixcy_codec_register(void);
//! ```
//!
//! The returned pointer is **static** — the host never frees it.
//!
//! # Stability contract
//! - `SIXCY_PLUGIN_ABI_VERSION` is **monotonically increasing and never
//!   decremented**.
//! - New fields are appended **at the end** of `SixcyCodecPlugin` only.
//! - Existing field offsets and calling conventions are frozen forever.
//! - A plugin compiled against ABI version N is compatible with any host ≥ N.
//!   The host ignores fields beyond what the plugin's `abi_version` declares.
//!
//! # Thread safety
//! Both `compress` and `decompress` MUST be safe to call concurrently from
//! multiple threads on different buffer pairs.  The plugin MUST NOT use any
//! global mutable state.  No allocator is shared with the host; all memory is
//! owned by the caller and passed via explicit length-annotated buffers.
//!
//! # Memory model
//! The plugin never allocates or frees memory on behalf of the host.
//! The host pre-allocates output buffers using the upper bound returned by
//! `compress_bound`.  A plugin that needs scratch space must manage its own
//! memory independently.

/// ABI version of this header.  Written into `SixcyCodecPlugin::abi_version`.
pub const SIXCY_PLUGIN_ABI_VERSION: u32 = 1;

/// Return codes from plugin compress/decompress functions.
pub mod rc {
    /// Success — `*out_len` contains the number of bytes written.
    pub const OK:           i32 = 0;
    /// Output buffer too small — caller must retry with a larger buffer.
    pub const OVERFLOW:     i32 = -1;
    /// Input data is corrupt or truncated.
    pub const CORRUPT:      i32 = -2;
    /// Codec-internal error (OOM, bad level, etc.).
    pub const INTERNAL:     i32 = -3;
}

/// Frozen C ABI descriptor for a codec plugin.
///
/// # Safety
/// All function pointers are `unsafe extern "C"` because they cross an FFI
/// boundary.  The Rust wrapper ([`PluginCodec`]) enforces the safety
/// invariants documented on each field before delegating to the raw pointer.
///
/// # Layout
/// `#[repr(C)]` is mandatory.  Do not reorder fields.  New fields go at the
/// end only.
#[repr(C)]
pub struct SixcyCodecPlugin {
    /// 16-byte codec UUID in little-endian field order.
    ///
    /// This value appears in every block header written by this codec.
    /// It is the authoritative identity; `short_id` is advisory.
    pub uuid: [u8; 16],

    /// Optional fast-path numeric alias.  `0` means no short ID is assigned.
    /// The host may use this for O(1) dispatch but MUST NOT rely on it for
    /// on-disk identity.
    pub short_id: u32,

    /// Must equal `SIXCY_PLUGIN_ABI_VERSION`.  The host rejects plugins with
    /// a higher `abi_version` than it was compiled against.
    pub abi_version: u32,

    /// Compress `in_len` bytes from `in_buf` into `out_buf`.
    ///
    /// On entry, `*out_len` is the capacity of `out_buf` in bytes.
    /// On `rc::OK`, `*out_len` is set to the number of bytes written.
    ///
    /// Thread safety: reentrant — safe to call from multiple threads
    ///   simultaneously with non-overlapping buffer pairs.
    ///
    /// # Safety
    /// - `in_buf[0..in_len]` must be a valid readable region.
    /// - `out_buf[0..*out_len]` must be a valid writable region.
    /// - The two regions must not overlap.
    /// - Neither pointer is null.
    pub compress: Option<unsafe extern "C" fn(
        in_buf:  *const u8, in_len:  u32,
        out_buf: *mut   u8, out_len: *mut u32,
        level:   i32,
    ) -> i32>,

    /// Decompress `in_len` bytes from `in_buf` into `out_buf`.
    ///
    /// On entry, `*out_len` is the capacity of `out_buf` in bytes.
    /// On `rc::OK`, `*out_len` is set to the number of bytes written.
    ///
    /// Thread safety: reentrant — same guarantee as `compress`.
    ///
    /// # Safety  (same as `compress`)
    pub decompress: Option<unsafe extern "C" fn(
        in_buf:  *const u8, in_len:  u32,
        out_buf: *mut   u8, out_len: *mut u32,
    ) -> i32>,

    /// Returns a guaranteed upper bound on the compressed output size for
    /// `in_len` bytes of input at any level.  Used by the host to pre-allocate
    /// the `out_buf` passed to `compress`.
    ///
    /// MUST be a pure function: deterministic, no side effects, no I/O,
    /// no global state reads.  Safe to call from any thread at any time.
    pub compress_bound: Option<unsafe extern "C" fn(in_len: u32) -> u32>,
}

// Safety: the ABI contract declares all fn pointers reentrant.
unsafe impl Send for SixcyCodecPlugin {}
unsafe impl Sync for SixcyCodecPlugin {}

/// Safe Rust wrapper around a loaded [`SixcyCodecPlugin`].
pub struct PluginCodec {
    /// Raw descriptor — lifetime must outlive this wrapper.
    desc: &'static SixcyCodecPlugin,
}

impl PluginCodec {
    /// Wrap a static plugin descriptor after validating the ABI version.
    ///
    /// # Errors
    /// Returns an error string if `abi_version` exceeds
    /// `SIXCY_PLUGIN_ABI_VERSION` (forward-incompatible plugin).
    pub fn new(desc: &'static SixcyCodecPlugin) -> Result<Self, String> {
        if desc.abi_version > SIXCY_PLUGIN_ABI_VERSION {
            return Err(format!(
                "Plugin ABI version {} is newer than host ABI version {}",
                desc.abi_version, SIXCY_PLUGIN_ABI_VERSION,
            ));
        }
        Ok(Self { desc })
    }

    pub fn uuid(&self) -> &[u8; 16] { &self.desc.uuid }

    pub fn compress(&self, data: &[u8], level: i32) -> Result<Vec<u8>, String> {
        let f = self.desc.compress.ok_or("Plugin missing compress fn")?;
        let bound_fn = self.desc.compress_bound.ok_or("Plugin missing compress_bound fn")?;
        let cap = unsafe { bound_fn(data.len() as u32) } as usize;
        let mut out = vec![0u8; cap];
        let mut out_len = cap as u32;
        let rc = unsafe {
            f(data.as_ptr(), data.len() as u32,
              out.as_mut_ptr(), &mut out_len,
              level)
        };
        if rc != rc::OK {
            return Err(format!("Plugin compress returned error code {rc}"));
        }
        out.truncate(out_len as usize);
        Ok(out)
    }

    pub fn decompress(&self, data: &[u8], orig_size: usize) -> Result<Vec<u8>, String> {
        let f = self.desc.decompress.ok_or("Plugin missing decompress fn")?;
        let mut out = vec![0u8; orig_size];
        let mut out_len = orig_size as u32;
        let rc = unsafe {
            f(data.as_ptr(), data.len() as u32,
              out.as_mut_ptr(), &mut out_len)
        };
        if rc != rc::OK {
            return Err(format!("Plugin decompress returned error code {rc}"));
        }
        out.truncate(out_len as usize);
        Ok(out)
    }
}
