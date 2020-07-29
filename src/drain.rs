use builder::SyslogBuilder;
use format::{DefaultMsgFormat, format, MsgFormat};
use libc::{self, c_char, c_int};
use slog::{self, Drain, Level, Record, OwnedKVList};
use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::CStr;
use std::io::{self, Write};
use std::ptr;
use std::sync::{Mutex, MutexGuard};

#[cfg(not(test))]
use libc::{closelog, openlog, syslog};
#[cfg(test)]
use mock::{self, closelog, openlog, syslog};

thread_local! {
    static TL_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(128))
}

lazy_static! {
    /// Keeps track of which `ident` string was most recently passed to `openlog`.
    /// 
    /// The mutex is to be locked while calling `openlog` or `closelog`. It
    /// contains a possibly-null pointer to the `ident` string most recently passed
    /// to `openlog`, if that pointer came from `CStr::as_ptr`.
    /// 
    /// The pointer is stored as a `usize` because pointers are `!Send`. It is only
    /// used for comparison, never dereferenced.
    /// 
    /// # Purpose and rationale
    /// 
    /// The POSIX `openlog` function accepts a pointer to a C string. Though POSIX
    /// does not specify the expected lifetime of the string, all known
    /// implementations either
    /// 
    /// 1. keep the pointer in a global variable, or
    /// 2. copy the string into an internal buffer, which is kept in a global
    ///    variable.
    /// 
    /// When running with an implementation in the second category, the string may
    /// be safely freed right away. When running with an implementation in the
    /// first category, however, the string must not be freed until either
    /// `closelog` is called or `openlog` is called with a *different, non-null*
    /// `ident`.
    /// 
    /// This mutex keeps track of which `ident` was most recently passed, making it
    /// possible to decide whether `closelog` needs to be called before a given
    /// `ident` string is dropped.
    /// 
    /// (Note: In the original 4.4BSD source code, the pointer is kept in a global
    /// variable, but `closelog` does *not* clear the pointer. In this case, it is
    /// only safe to free the string after `openlog` has been called with a
    /// different, non-null `ident`. Fortunately, all present-day implementations
    /// of `closelog` either clear the pointer or don't retain it at all.)
    static ref LAST_UNIQUE_IDENT: Mutex<usize> = Mutex::new(ptr::null::<c_char>() as usize);
}

/// [`Drain`] implementation that sends log messages to syslog.
/// 
/// [`Drain`]: https://docs.rs/slog/2/slog/trait.Drain.html
#[derive(Debug)]
pub struct SyslogDrain<F: MsgFormat> {
    /// The `ident` string, if it is owned by this `SyslogDrain`.
    /// 
    /// This is kept so that the string can be freed (and `closelog` called, if
    /// necessary) when this `SyslogDrain` is dropped.
    unique_ident: Option<Box<CStr>>,

    /// Log all messages with the given priority
    log_priority: libc::c_int,

    /// The format for log messages.
    format: F,
}

impl SyslogDrain<DefaultMsgFormat> {
    /// Creates a new `SyslogDrain` with all default settings.
    /// 
    /// Equivalent to `SyslogBuilder::new().build()`.
    pub fn new() -> Self {
        SyslogBuilder::new().build()
    }
}

impl<F: MsgFormat> SyslogDrain<F> {
    /// Creates a new `SyslogBuilder`.
    /// 
    /// Equivalent to `SyslogBuilder::new()`.
    #[inline]
    pub fn builder() -> SyslogBuilder {
        SyslogBuilder::new()
    }

    pub(crate) fn from_builder(builder: SyslogBuilder<F>) -> Self {
        // `ident` is the pointer that will be passed to `openlog`, maybe null.
        // 
        // `unique_ident` is the same pointer, wrapped in `Some` and `NonNull`,
        // but only if the `ident` string provided by the application is owned.
        // Otherwise it's `None`, indicating that `ident` either is null or
        // points to a `&'static` string.
        let (ident, unique_ident): (*const c_char, Option<Box<CStr>>) = match builder.ident.clone() {
            Some(Cow::Owned(ident_s)) => {
                let unique_ident = ident_s.into_boxed_c_str();

                // Calling `NonNull:new_unchecked` is correct because
                // `CString::into_raw` never returns a null pointer.
                (unique_ident.as_ptr(), Some(unique_ident))
            }
            Some(Cow::Borrowed(ident_s)) => (ident_s.as_ptr(), None),
            None => (ptr::null(), None),
        };

        {
            // `openlog` and `closelog` are only called while holding the mutex
            // around `last_unique_ident`.
            let mut last_unique_ident: MutexGuard<usize> = LAST_UNIQUE_IDENT.lock().unwrap();

            // Here, we call `openlog`. This has to happen *before* freeing the
            // previous `ident` string, if applicable.
            unsafe { openlog(ident, builder.option, builder.facility.into()); }

            // If `openlog` is called with a null `ident` pointer, then the
            // `ident` string passed to it previously will remain in use. But
            // if the `ident` pointer is not null, then `last_unique_ident`
            // needs updating.
            if !ident.is_null() {
                *last_unique_ident = match &unique_ident {
                    // If the `ident` string is owned, store the pointer to it.
                    Some(s) => s.as_ptr(),

                    // If the `ident` string is not owned, set the stored
                    // pointer to null.
                    None => ptr::null::<c_char>(),
                } as usize;
            }
        }

        SyslogDrain {
            unique_ident,
            log_priority: builder.log_priority,
            format: builder.format,
        }
    }
}

impl<F: MsgFormat> Drop for SyslogDrain<F> {
    fn drop(&mut self) {
        // Check if this `SyslogDrain` was created with an owned `ident`
        // string.
        if let Some(my_ident) = self.unique_ident.take() {
            // If so, then we need to check if that string is the one that
            // was most recently passed to `openlog`.
            let mut last_unique_ident: MutexGuard<usize> = match LAST_UNIQUE_IDENT.lock() {
                Ok(locked) => locked,

                // If the mutex was poisoned, then we'll just let the
                // string leak.
                // 
                // There's no point in panicking here, and if there was a
                // panic after `openlog` but before the pointer in the
                // mutex was updated, then trying to free the pointed-to
                // string may result in undefined behavior from a double
                // free.
                // 
                // Thankfully, Rust's standard mutex implementation
                // supports poisoning. Some alternative mutex
                // implementations, such as in the `parking_lot` crate,
                // don't support poisoning and would expose us to the
                // aforementioned undefined behavior.
                //
                // It would be nice if we could un-poison a poisoned mutex,
                // though. We have a perfectly good recovery strategy for
                // that situation (resetting its pointer to null), but no way
                // to use it.
                Err(_) => {
                    Box::leak(my_ident);
                    return;
                }
            };

            if my_ident.as_ptr() as usize == *last_unique_ident {
                // Yes, the most recently used string was ours. We need to
                // call `closelog` before our string is dropped.
                //
                // Note that this isn't completely free of races. It's still
                // possible for some other code to call `openlog` independently
                // of this module, after our `openlog` call. In that case, this
                // `closelog` call will incorrectly close *that* logging handle
                // instead of the one belonging to this `SyslogDrain`.
                //
                // Behavior in that case is still well-defined. Subsequent
                // calls to `syslog` will implicitly reopen the logging handle
                // anyway. The only problem is that the `openlog` options
                // (facility, program name, etc) will all be reset. For this
                // reason, it is a bad idea for a library to call `openlog` (or
                // construct a `SyslogDrain`!) except when instructed to do so
                // by the main program.
                unsafe { closelog(); }

                // Also, be sure to reset the pointer stored in the mutex.
                // Although it is never dereferenced, letting it dangle may
                // cause the above `if` to test true when it shouldn't, which
                // would result in `closelog` being called when it shouldn't.
                *last_unique_ident = ptr::null::<c_char>() as usize;
            }

            // When testing, before dropping the owned string, copy it into
            // a mock event. We'll still drop it, though, in order to test for 
            // double-free bugs.
            #[cfg(test)]
            mock::push_event(mock::Event::DropOwnedIdent(
                String::from(my_ident.to_string_lossy())
            ));

            // Now that `closelog` has been called, it's safe for our string to
            // be dropped, which will happen here.
        }
    }
}

impl<F: MsgFormat> Drain for SyslogDrain<F> {
    type Ok = ();
    type Err = slog::Never;

    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        TL_BUF.with(|tl_buf_ref| {
            let mut tl_buf_mut = tl_buf_ref.borrow_mut();
            let mut tl_buf = &mut *tl_buf_mut;

            // Figure out the priority.
            let priority = if self.log_priority > 0 { self.log_priority } else { get_priority(record.level()) };

            // Format the message. 
            let fmt_err = format(&self.format, &mut tl_buf, record, values).err();

            // If formatting fails, use an effectively null format (which shouldn't 
            // ever fail), and separately log the error.
            if fmt_err.is_some() {
                tl_buf.clear();
                assert_format_success(write!(tl_buf, "{}", record.msg()));
            }

            {
                // Convert both strings to C strings.
                let msg = make_cstr_lossy(tl_buf);

                // All set. Submit the log message.
                unsafe {
                    syslog(
                        priority,
                        CStr::from_bytes_with_nul_unchecked(b"%s\0").as_ptr(),
                        msg.as_ptr()
                    );
                }
            }

            // Clean up.
            tl_buf.clear();

            // If there was a formatting error, log that too.
            if let Some(fmt_err) = fmt_err {
                assert_format_success(write!(tl_buf, "{}", fmt_err));

                {
                    let msg = make_cstr_lossy(tl_buf);

                    unsafe {
                        syslog(
                            libc::LOG_ERR,
                            CStr::from_bytes_with_nul_unchecked(b"Error fully formatting the previous log message: %s\0").as_ptr(),
                            msg.as_ptr()
                        );
                    }
                }

                // Clean up again.
                tl_buf.clear();
            }

            // Done.
            Ok(())
        })
    }
}

/// Creates a `&CStr` from the given `Vec<u8>`, removing middle null bytes and
/// adding a null terminator as needed.
fn make_cstr_lossy(s: &mut Vec<u8>) -> &CStr {
    // Strip any null bytes from the string.
    s.retain(|b| *b != 0);

    // Add a null terminator.
    s.push(0);

    // This is sound because we just stripped all the null bytes from the
    // input (except the one at the end).
    unsafe { CStr::from_bytes_with_nul_unchecked(&*s) }
}

/// Panics on I/O error, but only in debug builds.
/// 
/// Used for `io::Write`s into a `Vec`, which should never fail.
#[inline]
fn assert_format_success(_result: io::Result<()>) {
    #[cfg(debug)]
    _result.expect("unexpected formatting error");
}


pub(crate) fn get_priority(level: Level) -> c_int {
    match level {
        Level::Critical => libc::LOG_CRIT,
        Level::Error => libc::LOG_ERR,
        Level::Warning => libc::LOG_WARNING,
        Level::Debug | Level::Trace => libc::LOG_DEBUG,

        // `slog::Level` isn't non-exhaustive, so adding any more levels
        // would be a breaking change. That is highly unlikely to ever
        // happen. Still, we'll handle the possibility here, just in case.
        _ => libc::LOG_INFO
    }
}