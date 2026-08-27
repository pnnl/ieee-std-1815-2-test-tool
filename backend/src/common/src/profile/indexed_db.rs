//! Generic indexed entry database used by CurveDatabase, ScheduleBCDatabase, and ScheduleDatabase.

use std::collections::HashMap;

use crate::profile::{validation::ValidationErrors, values::TransmissionI32};

use super::profile::AiPoint;

/// A single AI point: its DNP3 index paired with its raw transmitted integer value.
///
/// These are the values as transmitted over DNP3 (Group 30 Var 1, 32-bit signed integer).
/// They must be scaled using the profile's multiplier and offset before being used
/// for any functional control logic.
///
/// Primarily used as the value type for entries in an [`IndexedEntryDatabase`].
#[derive(Debug, Clone, Copy)]
pub struct AiValue {
    /// The DNP3 AI point index.
    pub index: u16,
    /// The transmitted value for this point.
    pub value: TransmissionI32,
}

impl AiValue {
    pub fn new(index: u16, value: i32) -> Self {
        Self {
            index,
            value: TransmissionI32(value),
        }
    }
}

impl TryFrom<&AiPoint> for AiValue {
    /// Convert an `AiPoint` from the profile into a runtime `AiValue`.
    /// The raw value is taken from `point.value`, cast to `i32`, defaulting to `0` if absent.
    fn try_from(point: &AiPoint) -> Result<Self, ValidationErrors> {
        Ok(Self {
            index: point.point_index,
            value: TransmissionI32::try_from_engineering(
                point.value(),
                point.multiplier(),
                point.offset,
            )?,
        })
    }

    type Error = ValidationErrors;
}

/// Trait required for entries stored in an [`IndexedEntryDatabase`].
pub trait DatabaseEntry: Clone {
    /// Return all AI values for this entry (header fields + data slots).
    /// Values are raw transmitted integers; apply scaling before use in control logic.
    fn all_values(&self) -> Vec<AiValue>;
    /// Update the stored raw transmitted value for `ai_index`. Returns `true` if the index was found.
    /// Implementations must also mark the AI index as written in their internal tracker.
    fn update_value(&mut self, ai_index: u16, new_value: TransmissionI32) -> bool;
    /// Return a blank copy of this entry: same vector structure and AI indices as `self`,
    /// all values zeroed to 0, and the written tracker empty.
    fn blank(&self) -> Self;
    /// Return only the AI values that have been explicitly written (via `update_value`).
    fn received_values(&self) -> Vec<AiValue>;
}

/// Generic container for 1-based indexed database entries.
///
/// Stores a collection of entries keyed by a 1-based number, plus a flat list of
/// AI values for the currently selected entry. Call `set_current_entry_points` whenever
/// the active entry changes to keep `all_points` in sync. Individual value updates via
/// `update_ai_value` are also reflected immediately.
pub struct IndexedEntryDatabase<E: DatabaseEntry> {
    entries: HashMap<u16, E>,
    entry_points: Vec<AiValue>,
    max_entries: u16,
}

impl<E: DatabaseEntry> IndexedEntryDatabase<E> {
    /// Construct an empty database. Call `insert` to populate entries, then
    /// `set_current_entry_points(1)` to initialise the point list.
    pub(super) fn new(entries: HashMap<u16, E>, max_entries: u16) -> Self {
        Self {
            entries,
            entry_points: Vec::new(),
            max_entries,
        }
    }

    /// All AI values for the currently selected entry, kept in sync with any updates.
    pub fn current_entry_points(&self) -> Vec<AiValue> {
        self.entry_points.clone()
    }

    /// Look up an entry by its 1-based number.
    pub fn get(&self, number: u16) -> Option<&E> {
        self.entries.get(&number)
    }

    /// Look up an entry mutably by its 1-based number.
    pub fn get_mut(&mut self, number: u16) -> Option<&mut E> {
        self.entries.get_mut(&number)
    }

    /// Return entry 1, used as the template for creating blank entries with matching AI indices.
    pub fn template_entry(&self) -> Option<&E> {
        self.entries.get(&1)
    }

    /// Maximum number of entries this database supports.
    pub fn max_entries(&self) -> u16 {
        self.max_entries
    }

    /// Update the stored raw transmitted value for `ai_index` in the entry at `entry_number`.
    /// If `entry_number` is the currently selected entry, also updates `entry_points` in place.
    /// Does nothing if the entry number or AI index does not exist.
    pub fn update_ai_value(
        &mut self,
        entry_number: u16,
        ai_index: u16,
        new_value: TransmissionI32,
    ) {
        if let Some(entry) = self.entries.get_mut(&entry_number) {
            if entry.update_value(ai_index, new_value) {
                if let Some(point) = self.entry_points.iter_mut().find(|p| p.index == ai_index) {
                    point.value = new_value;
                }
            }
        }
    }

    /// Return a reference to the entry at `number`, creating a blank entry from the template
    /// if it does not yet exist. Does not change `entry_points`; call `set_current_entry_points`
    /// after switching entries.
    /// Returns `None` if `number` is out of range (0 or greater than `max_entries`) or if no
    /// template entry (entry 1) is present.
    pub fn get_or_create_entry(&mut self, number: u16) -> Option<&E> {
        if number == 0 || number > self.max_entries {
            return None;
        }
        if !self.entries.contains_key(&number) {
            let blank = self.entries.get(&1)?.blank();
            self.entries.insert(number, blank);
        }
        self.entries.get(&number)
    }

    /// Replace `entry_points` with the current values of the entry at `number`.
    /// Call this whenever the active entry changes.
    pub(super) fn set_current_entry_points(&mut self, number: u16) {
        if let Some(entry) = self.entries.get(&number) {
            self.entry_points = entry.all_values();
        }
    }

    /// Number of entries in the database.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Insert an entry at the given number (used during construction).
    pub(super) fn insert(&mut self, number: u16, entry: E) {
        self.entries.insert(number, entry);
    }
}
