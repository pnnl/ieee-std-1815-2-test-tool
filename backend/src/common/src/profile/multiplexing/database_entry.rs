use crate::profile::{indexed_db::AiValue, values::TransmissionI32};

/// Trait required for entries stored in an [`IndexedEntryDatabase`](crate::profile::indexed_db::IndexedEntryDatabase).
pub trait DatabaseEntry: Clone {
    /// Return all AI values for this entry (header fields + data slots).
    /// Values are raw transmitted integers; apply scaling before use in control logic.
    fn all_values(&self) -> Vec<AiValue>;
    /// Iterate over the stored AI values mutably, in the same order as `all_values`.
    fn all_values_mut(&mut self) -> impl Iterator<Item = &mut AiValue>;
    /// Mark an existing AI index as explicitly written in the internal tracker.
    fn mark_written(&mut self, ai_index: u16);
    /// Update the stored raw transmitted value for `ai_index`. Returns `true` if the index was found.
    /// Mark the AI index as written only when a matching point is found.
    fn update_value(&mut self, ai_index: u16, new_value: TransmissionI32) -> bool {
        if let Some(field) = self.all_values_mut().find(|field| field.index == ai_index) {
            field.value = new_value;
        } else {
            return false;
        }
        self.mark_written(ai_index);
        true
    }
    /// Return a blank copy of this entry: same vector structure and AI indices as `self`,
    /// all values zeroed to 0, and the written tracker empty.
    fn create_blank_instance(&self) -> Self;
    /// Return only the AI values that have been explicitly written (via `update_value`).
    fn received_values(&self) -> Vec<AiValue>;
}
