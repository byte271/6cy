/*
 * sixcy_plugin.h — Frozen C ABI for .6cy codec plugins
 *
 * ABI version: 1
 * Format:      .6cy v3+
 *
 * ── Stability contract ─────────────────────────────────────────────────────
 *
 *  This header is FROZEN at ABI version 1.
 *
 *  The following NEVER change:
 *    - struct field offsets and types
 *    - function pointer signatures
 *    - return code values
 *    - SIXCY_CODEC_UUID_LEN
 *
 *  New fields are ONLY appended at the end of SixcyCodecPlugin.
 *  The host uses abi_version to know which fields are present.
 *
 *  A plugin compiled against ABI version N is binary-compatible with any
 *  host whose ABI version >= N.  The host ignores fields beyond what the
 *  plugin's abi_version declares.
 *
 * ── Entry point ────────────────────────────────────────────────────────────
 *
 *  Every plugin MUST export exactly one symbol with C linkage:
 *
 *    const SixcyCodecPlugin *sixcy_codec_register(void);
 *
 *  The returned pointer MUST be static — the host never frees it.
 *  The function MUST be idempotent and return the same address every call.
 *
 * ── Thread safety ──────────────────────────────────────────────────────────
 *
 *  `fn_compress` and `fn_decompress` MUST be safe to call concurrently from
 *  multiple threads with non-overlapping buffer pairs.
 *
 *  The plugin MUST NOT use any global mutable state.
 *  The plugin MUST NOT call malloc/free/realloc on the host's behalf.
 *  All memory is owned by the caller and passed as explicit length-tagged
 *  pointers.  The plugin manages any internal scratch space privately.
 *
 * ── Memory model ───────────────────────────────────────────────────────────
 *
 *  No allocator is shared between host and plugin.
 *  The host pre-allocates output buffers using fn_compress_bound().
 *  Input and output buffers MUST NOT overlap.
 *  All pointer parameters are non-null when the function is called.
 *
 * ── Endianness ─────────────────────────────────────────────────────────────
 *
 *  `codec_uuid` is stored as 16 raw bytes in little-endian UUID field order
 *  (RFC 4122 §4.1.2 wire format, same as written into block headers).
 *  The host matches this value byte-for-byte against block header codec_uuid
 *  fields.  No byte-swapping is performed; the plugin author is responsible
 *  for using the correct byte order.
 *
 * ── Codec identity ─────────────────────────────────────────────────────────
 *
 *  `codec_uuid` is the authoritative identity for a codec.
 *  `short_id` is an advisory in-process alias (0 = none assigned).
 *  The host MUST use `codec_uuid` for on-disk matching.
 *  The host MAY use `short_id` for fast in-process dispatch; it MUST NOT
 *  use `short_id` for any persistent operation.
 */

#ifndef SIXCY_PLUGIN_H
#define SIXCY_PLUGIN_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ── Constants ────────────────────────────────────────────────────────────── */

/** ABI version implemented by this header.
 *  Written into SixcyCodecPlugin::abi_version by every plugin. */
#define SIXCY_PLUGIN_ABI_VERSION  UINT32_C(1)

/** Byte length of a codec UUID in little-endian field order. */
#define SIXCY_CODEC_UUID_LEN      16

/* ── Return codes ─────────────────────────────────────────────────────────── */

/** Success. *out_len contains bytes written. */
#define SIXCY_RC_OK               0

/** Output buffer too small. Caller MUST retry with a larger buffer.
 *  *out_len is set to the minimum required size when possible. */
#define SIXCY_RC_OVERFLOW        (-1)

/** Input data is corrupt or truncated. */
#define SIXCY_RC_CORRUPT         (-2)

/** Codec-internal error (OOM, invalid level, etc.). */
#define SIXCY_RC_INTERNAL        (-3)

/* ── Plugin descriptor ────────────────────────────────────────────────────── */

/**
 * SixcyCodecPlugin — static descriptor for one codec implementation.
 *
 * Layout is #pragma pack(1) to guarantee field offsets across compilers.
 * Do NOT add fields between existing ones.  Append at the end only.
 */
#pragma pack(push, 1)
typedef struct SixcyCodecPlugin {

    /**
     * [offset 0, 16 bytes]
     * Codec UUID in little-endian RFC 4122 field order.
     *
     * This value is written verbatim into every block header on disk.
     * It is the sole authoritative identity for this codec.
     * It MUST match the frozen UUID assigned in the sixcy specification.
     */
    uint8_t codec_uuid[SIXCY_CODEC_UUID_LEN];

    /**
     * [offset 16, 4 bytes]
     * In-process advisory short ID.  0 = none assigned.
     *
     * The host MAY use this for O(1) dispatch.
     * The host MUST NOT use this for any on-disk operation.
     * Short IDs are local to a process invocation and are NOT stable
     * across builds or plugin versions.
     */
    uint32_t short_id;

    /**
     * [offset 20, 4 bytes]
     * ABI version this plugin was compiled against.
     * MUST equal SIXCY_PLUGIN_ABI_VERSION from the header used at compile time.
     *
     * A host compiled against ABI version H rejects any plugin with
     * abi_version > H (the plugin is newer than the host understands).
     */
    uint32_t abi_version;

    /**
     * [offset 24]
     * Compress in_len bytes from in_buf into out_buf.
     *
     * On entry:  *out_len is the capacity of out_buf in bytes.
     * On SIXCY_RC_OK: *out_len is set to bytes written.
     * On SIXCY_RC_OVERFLOW: *out_len is set to the minimum required size
     *   when determinable; otherwise left unchanged.
     *
     * Thread safety: MUST be reentrant.  Safe to call simultaneously from
     *   multiple threads with non-overlapping (in_buf, out_buf) pairs.
     *
     * @param in_buf   Non-null.  in_buf[0..in_len) is readable.
     * @param in_len   Byte count of input.
     * @param out_buf  Non-null.  out_buf[0..*out_len) is writable.
     * @param out_len  Non-null.  In: capacity.  Out: bytes written.
     * @param level    Compression level (codec-defined range).
     * @return         SIXCY_RC_OK or a SIXCY_RC_* error code.
     */
    int32_t (*fn_compress)(
        const uint8_t *in_buf,  uint32_t  in_len,
              uint8_t *out_buf, uint32_t *out_len,
        int32_t level
    );

    /**
     * [next offset after fn_compress pointer]
     * Decompress in_len bytes from in_buf into out_buf.
     *
     * On entry:  *out_len is the capacity of out_buf in bytes.
     * On SIXCY_RC_OK: *out_len is set to bytes written.
     *
     * The host passes orig_size (from the block header) as the initial
     * *out_len to give the plugin a tight bound.  Plugins that require a
     * larger scratch space must manage it internally.
     *
     * Thread safety: same guarantee as fn_compress.
     *
     * @param in_buf   Non-null.  in_buf[0..in_len) is readable.
     * @param in_len   Byte count of compressed input.
     * @param out_buf  Non-null.  out_buf[0..*out_len) is writable.
     * @param out_len  Non-null.  In: capacity.  Out: bytes written.
     * @return         SIXCY_RC_OK or a SIXCY_RC_* error code.
     */
    int32_t (*fn_decompress)(
        const uint8_t *in_buf,  uint32_t  in_len,
              uint8_t *out_buf, uint32_t *out_len
    );

    /**
     * Upper bound on compressed output size for any in_len bytes at any level.
     *
     * The host uses this to pre-allocate the output buffer passed to
     * fn_compress.  The bound MUST be exact or conservative — never optimistic.
     *
     * MUST be a pure function: deterministic, no side effects, no I/O,
     * no global state.  Safe to call from any thread at any time, including
     * before and after codec initialisation.
     *
     * @param in_len  Byte count of uncompressed input.
     * @return        Maximum possible compressed size in bytes.
     */
    uint32_t (*fn_compress_bound)(uint32_t in_len);

    /*
     * ── ABI v2+ fields appended here ────────────────────────────────────────
     *
     * Example (not present in v1):
     *
     *   uint32_t (*fn_dict_compress)(
     *       const uint8_t *dict, uint32_t dict_len,
     *       const uint8_t *in,   uint32_t in_len,
     *             uint8_t *out,  uint32_t *out_len,
     *       int32_t level
     *   );
     */

} SixcyCodecPlugin;
#pragma pack(pop)

/* ── Plugin entry point ───────────────────────────────────────────────────── */

/**
 * The sole required export from a .6cy codec plugin shared library.
 *
 * Returns a pointer to a static SixcyCodecPlugin descriptor.
 * The pointer MUST remain valid for the lifetime of the process.
 * The function MUST be idempotent.
 *
 * The host calls this once at plugin load time (dlopen / LoadLibrary).
 * If abi_version > SIXCY_PLUGIN_ABI_VERSION the host rejects the plugin
 * and dlcloses the library.
 */
typedef const SixcyCodecPlugin *(*sixcy_codec_register_fn)(void);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* SIXCY_PLUGIN_H */
