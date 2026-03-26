use resharp::Regex;
use std::cell::RefCell;
use std::slice;

thread_local!(static LAST_ERR: RefCell<String> = const { RefCell::new(String::new()) });

fn set_err(e: impl std::fmt::Display) {
    LAST_ERR.with(|s| *s.borrow_mut() = e.to_string());
}

/// # Safety
/// caller must ensure `ptr` is valid for `len` bytes for the duration of `'a`
unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 {
        &[]
    } else {
        slice::from_raw_parts(ptr, len)
    }
}

/// Compile a regex pattern. Returns null on error (call `resharp_last_error`).
///
/// # Safety
/// `pat` must be valid for reading `len` bytes.
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

/// Free a compiled regex. No-op if `r` is null.
///
/// # Safety
/// `r` must be null or a pointer returned by `resharp_compile` that has not been freed.
#[no_mangle]
pub unsafe extern "C" fn resharp_free(r: *mut Regex) {
    if !r.is_null() {
        drop(Box::from_raw(r));
    }
}

/// Returns 1 if `input` matches, 0 if not, -1 on error.
///
/// # Safety
/// `r` must be a live pointer from `resharp_compile`. `input` must be valid for `len` bytes.
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

/// Find all matches. Writes `(start, end)` pairs into `out` (max `cap/2` matches).
/// Returns total match count (may exceed `cap/2`), or -1 on error.
///
/// # Safety
/// `r` must be a live pointer from `resharp_compile`. `input` must be valid for `len` bytes.
/// `out` must be valid for writing `cap` `u32` values.
#[no_mangle]
pub unsafe extern "C" fn resharp_find_all(
    r: *const Regex,
    input: *const u8,
    len: usize,
    out: *mut u32,
    cap: usize,
) -> i32 {
    match (*r).find_all(bytes(input, len)) {
        Ok(ms) => {
            let n = ms.len();
            let w = (cap / 2).min(n);
            let buf = slice::from_raw_parts_mut(out, w * 2);
            for (i, m) in ms.iter().take(w).enumerate() {
                buf[i * 2] = m.start as u32;
                buf[i * 2 + 1] = m.end as u32;
            }
            n as i32
        }
        Err(e) => {
            set_err(e);
            -1
        }
    }
}

/// Find an anchored match at position 0. Writes `(start, end)` into `out`.
/// Returns 1 if found, 0 if not, -1 on error.
///
/// # Safety
/// `r` must be a live pointer from `resharp_compile`. `input` must be valid for `len` bytes.
/// `out` must be valid for writing 2 `u32` values.
#[no_mangle]
pub unsafe extern "C" fn resharp_find_anchored(
    r: *const Regex,
    input: *const u8,
    len: usize,
    out: *mut u32,
) -> i32 {
    match (*r).find_anchored(bytes(input, len)) {
        Ok(Some(m)) => {
            *out = m.start as u32;
            *out.add(1) = m.end as u32;
            1
        }
        Ok(None) => 0,
        Err(e) => {
            set_err(e);
            -1
        }
    }
}

/// Copy the last error message into `buf`. Returns the full error length
/// (may exceed `cap`; not null-terminated).
///
/// # Safety
/// `buf` must be valid for writing `cap` bytes.
#[no_mangle]
pub unsafe extern "C" fn resharp_last_error(buf: *mut u8, cap: usize) -> usize {
    LAST_ERR.with(|s| {
        let s = s.borrow();
        let n = s.len().min(cap);
        std::ptr::copy_nonoverlapping(s.as_ptr(), buf, n);
        s.len()
    })
}
