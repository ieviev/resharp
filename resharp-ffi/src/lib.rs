use resharp::Regex;
use std::cell::RefCell;
use std::slice;

thread_local!(static LAST_ERR: RefCell<String> = const { RefCell::new(String::new()) });

fn set_err(e: impl std::fmt::Display) {
    LAST_ERR.with(|s| *s.borrow_mut() = e.to_string());
}

/// # Safety
/// `ptr` valid for `len` bytes over `'a`.
unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 {
        &[]
    } else {
        slice::from_raw_parts(ptr, len)
    }
}

/// Compile a pattern. Null on error, see `resharp_last_error`.
///
/// # Safety
/// `pat` valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn resharp_compile(pat: *const u8, len: usize) -> *mut Regex {
    let Ok(s) = std::str::from_utf8(bytes(pat, len)) else {
        set_err("pattern is not valid UTF-8");
        return std::ptr::null_mut();
    };
    match Regex::new(s) {
        Ok(r) => Box::into_raw(Box::new(r)),
        Err(e) => {
            set_err(e);
            std::ptr::null_mut()
        }
    }
}

/// Free a regex, no-op on null.
///
/// # Safety
/// `r` null or unfreed from `resharp_compile`.
#[no_mangle]
pub unsafe extern "C" fn resharp_free(r: *mut Regex) {
    if !r.is_null() {
        drop(Box::from_raw(r));
    }
}

/// 1 on match, 0 on no match, -1 on error.
///
/// # Safety
/// `r` live from `resharp_compile`, `input` valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn resharp_is_match(r: *const Regex, input: *const u8, len: usize) -> i32 {
    match (*r).is_match(bytes(input, len)) {
        Ok(v) => v as i32,
        Err(e) => {
            set_err(e);
            -1
        }
    }
}

/// Writes up to `cap/2` `(start, end)` pairs into `out`; returns total match count (may exceed pairs written) or -1 on error.
///
/// # Safety
/// `r` live from `resharp_compile`, `input`/`out` valid for `len`/`cap`.
#[no_mangle]
pub unsafe extern "C" fn resharp_find_all(
    r: *const Regex,
    input: *const u8,
    len: usize,
    out: *mut usize,
    cap: usize,
) -> isize {
    match (*r).find_all(bytes(input, len)) {
        Ok(ms) => {
            let n = ms.len();
            let w = (cap / 2).min(n);
            let buf = slice::from_raw_parts_mut(out, w * 2);
            for (i, m) in ms.iter().take(w).enumerate() {
                buf[i * 2] = m.start;
                buf[i * 2 + 1] = m.end;
            }
            n as isize
        }
        Err(e) => {
            set_err(e);
            -1
        }
    }
}

/// Longest match at position 0 as `(start, end)` in `out`; 1 found, 0 not found, -1 error.
///
/// # Safety
/// `r` live from `resharp_compile`, `input` valid for `len` bytes, `out` valid for 2 writes.
#[no_mangle]
pub unsafe extern "C" fn resharp_find_anchored(
    r: *const Regex,
    input: *const u8,
    len: usize,
    out: *mut usize,
) -> i32 {
    match (*r).find_anchored(bytes(input, len)) {
        Ok(Some(m)) => {
            *out = m.start;
            *out.add(1) = m.end;
            1
        }
        Ok(None) => 0,
        Err(e) => {
            set_err(e);
            -1
        }
    }
}

/// Capture slots per match (whole match + one per group); the row stride, in pairs, of `resharp_captures_all`.
///
/// # Safety
/// `r` live from `resharp_compile`.
#[cfg(feature = "experimental_capture_groups")]
#[no_mangle]
pub unsafe extern "C" fn resharp_capture_slots(r: *const Regex) -> usize {
    (*r).capture_names().len()
}

/// Writes up to `cap / (2 * slots)` rows into `out`, one per match: `resharp_capture_slots(r)` `(start, end)` pairs each, pair 0 the whole match, absent groups `(SIZE_MAX, SIZE_MAX)`. Returns total match count (may exceed rows written) or -1 on error.
///
/// # Safety
/// `r` live from `resharp_compile`, `input`/`out` valid for `len`/`cap`.
#[cfg(feature = "experimental_capture_groups")]
#[no_mangle]
pub unsafe extern "C" fn resharp_captures_all(
    r: *const Regex,
    input: *const u8,
    len: usize,
    out: *mut usize,
    cap: usize,
) -> isize {
    let slots = (*r).capture_names().len();
    match (*r).captures_all(bytes(input, len)) {
        Ok(all) => {
            let n = all.len();
            let rows = (cap / (2 * slots)).min(n);
            let buf = slice::from_raw_parts_mut(out, rows * slots * 2);
            for (row, caps) in all.iter().take(rows).enumerate() {
                for (i, g) in caps.spans().iter().enumerate() {
                    let (s, e) = g.unwrap_or((usize::MAX, usize::MAX));
                    buf[(row * slots + i) * 2] = s;
                    buf[(row * slots + i) * 2 + 1] = e;
                }
            }
            n as isize
        }
        Err(e) => {
            set_err(e);
            -1
        }
    }
}

/// Copies the last error into `buf` (not null-terminated); returns its full length, which may exceed `cap`.
///
/// # Safety
/// `buf` valid for `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn resharp_last_error(buf: *mut u8, cap: usize) -> usize {
    LAST_ERR.with(|s| {
        let s = s.borrow();
        let n = s.len().min(cap);
        std::ptr::copy_nonoverlapping(s.as_ptr(), buf, n);
        s.len()
    })
}
