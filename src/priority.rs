use Facility;
use Level;
use libc::c_int;
use std::cmp::{Eq, PartialEq};
use std::hash::{Hash, Hasher};

#[cfg(feature = "serde")]
use serde::{Deserialize, ser, Serialize, Serializer};

/// A syslog priority (combination of [severity level] and [facility]).
/// 
/// Each message sent to syslog has a “priority”, which consists of a
/// required [severity level] and an optional [facility]. This structure
/// represents a priority, either as symbolic level and facility (created with
/// the [`new`] method), or as a raw numeric value (created with the
/// [`from_raw`] method).
/// 
/// To customize the syslog priorities of log messages, implement
/// [`Adapter::priority`]. The easiest way to do that is to call
/// [`Adapter::with_priority`] on an existing [`Adapter`], such as
/// [`DefaultAdapter`].
/// 
/// Several convenient `From` implementations are also provided. `From<c_int>`
/// is not provided because it would be unsound (see the “safety” section of
/// the documentation for the [`from_raw`] method).
/// 
/// # Examples
/// 
/// See the documentation for [`Adapter::with_priority`] for example usage.
/// 
/// [`Adapter`]: adapter/trait.Adapter.html
/// [`Adapter::priority`]: adapter/trait.Adapter.html#method.priority
/// [`Adapter::with_priority`]: adapter/trait.Adapter.html#method.with_priority
/// [`DefaultAdapter`]: adapter/struct.DefaultAdapter.html
/// [facility]: enum.Facility.html
/// [`from_raw`]: #method.from_raw
/// [`new`]: #method.new
/// [severity level]: enum.Level.html
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
#[cfg_attr(feature = "serde", serde(from = "PrioritySerde"))]
pub struct Priority(PriorityKind);

impl Priority {
    /// Creates a new `Priority` consisting of the given `Level` and
    /// `Option<Facility>`.
    pub fn new(level: Level, facility: Option<Facility>) -> Self {
        Priority(PriorityKind::Normal(level, facility))
    }

    /// The `Level` that this `Priority` was created with.
    /// 
    /// This will be `None` if this `Priority` was created with the
    /// [`from_raw`] method.
    /// 
    /// [`from_raw`]: #method.from_raw
    pub fn level(self) -> Option<Level> {
        match self.0 {
            PriorityKind::Normal(level, _) => Some(level),
            PriorityKind::Raw(_) => None,
        }
    }

    /// The `Facility` that this `Priority` was created with, if any.
    /// 
    /// This will be `None` if this `Priority` was created without a `Facility`
    /// or if this `Priority` was created with the [`from_raw`] method.
    /// 
    /// [`from_raw`]: #method.from_raw
    pub fn facility(self) -> Option<Facility> {
        match self.0 {
            PriorityKind::Normal(_, facility) => facility,
            PriorityKind::Raw(_) => None,
        }
    }

    /// Fills in the facility from another `Priority`.
    /// 
    /// A `Priority` can contain a [`Level`], a [`Level`] and [`Facility`], or
    /// a raw numeric value. If this `Priority` contains only a [`Level`], then
    /// this method will take the [`Facility`] from the other `Priority`,
    /// creating a new, combined `Priority`.
    /// 
    /// This method simply returns `self` if `other` doesn't have a facility
    /// either, or if `other` was created using [`from_raw`].
    /// 
    /// # Example
    /// 
    /// ```
    /// use slog_syslog::{Facility, Level, Priority};
    /// 
    /// let defaults = Priority::new(Level::Notice, Some(Facility::Mail));
    /// let priority = Priority::new(Level::Err, None);
    /// let overlaid = priority.overlay(defaults);
    /// 
    /// assert_eq!(overlaid, Priority::new(Level::Err, Some(Facility::Mail)));
    /// ```
    /// 
    /// [`Facility`]: enum.Facility.html
    /// [`from_raw`]: #method.from_raw
    /// [`Level`]: enum.Level.html
    pub fn overlay(self, other: Priority) -> Priority {
        match (self.0, other.0) {
            (
                PriorityKind::Normal(level, None),
                PriorityKind::Normal(_, Some(facility)),
            ) => Priority::new(level, Some(facility)),
            _ => self,
        }
    }

    /// Creates a new `Priority` from the given raw numeric value.
    /// 
    /// # Safety
    /// 
    /// The numeric priority value must be valid for the system that the
    /// program is running on, using the `libc::LOG_*` constants. [POSIX] does
    /// not specify what happens if an incorrect numeric priority value is
    /// passed to the system `syslog` function.
    /// 
    /// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/closelog.html
    pub unsafe fn from_raw(priority: c_int) -> Self {
        Priority(PriorityKind::Raw(priority))
    }

    /// Converts this `Priority` into a raw numeric value, as accepted by the
    /// system `syslog` function.
    pub fn into_raw(self) -> c_int {
        match self.0 {
            PriorityKind::Normal(level, facility) =>
                c_int::from(level) | facility.map(c_int::from).unwrap_or(0),
            
            PriorityKind::Raw(priority) => priority,
        }
    }
}

impl PartialEq<Priority> for Priority {
    fn eq(&self, other: &Priority) -> bool {
        self.into_raw() == other.into_raw()
    }
}

impl Eq for Priority {}

impl Hash for Priority {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.into_raw().hash(state)
    }
}

impl From<Level> for Priority {
    fn from(level: Level) -> Self {
        Priority::new(level, None)
    }
}

impl From<(Level, Option<Facility>)> for Priority {
    fn from((level, facility): (Level, Option<Facility>)) -> Self {
        Priority::new(level, facility)
    }
}

impl From<(Level, Facility)> for Priority {
    fn from((level, facility): (Level, Facility)) -> Self {
        Priority::new(level, Some(facility))
    }
}

#[derive(Clone, Copy, Debug)]
enum PriorityKind {
    Normal(Level, Option<Facility>),
    Raw(c_int),
}

#[test]
fn test_into_raw() {
    use libc;

    let prio = Priority::new(Level::Warning, Some(Facility::Local3));
    assert_eq!(prio.into_raw(), libc::LOG_WARNING | libc::LOG_LOCAL3);

    let prio = Priority::new(Level::Alert, None);
    assert_eq!(prio.into_raw(), libc::LOG_ALERT);
}

#[cfg(feature = "serde")]
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum PrioritySerde {
    LevelOnly(Level),
    LevelAndFacility(Level, Facility),
}

#[cfg(feature = "serde")]
impl From<PrioritySerde> for Priority {
    fn from(priority: PrioritySerde) -> Priority {
        match priority {
            PrioritySerde::LevelOnly(level) => Priority::new(level, None),
            PrioritySerde::LevelAndFacility(level, facility) => Priority::new(level, Some(facility)),
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for Priority {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        match (self.level(), self.facility()) {
            (None, _) => return Err(ser::Error::custom("cannot serialize a `Priority` that was created with `Priority::from_raw`")),
            (Some(level), None) => PrioritySerde::LevelOnly(level),
            (Some(level), Some(facility)) => PrioritySerde::LevelAndFacility(level, facility),
        }.serialize(serializer)
    }
}
