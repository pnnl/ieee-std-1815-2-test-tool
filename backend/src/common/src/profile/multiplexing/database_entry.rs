use crate::profile::{AiValue, BiValue, PointValue, multiplexing::point_value::PointIndex};

/// Trait required for entries stored in an [`IndexedEntryDatabase`](crate::profile::indexed_db::IndexedEntryDatabase).
pub trait DatabaseEntry: Clone {
    /// Return all AI values for this entry (header fields + data slots).
    /// Values are raw transmitted integers; apply scaling before use in control logic.
    fn all_ai_values(&self) -> Vec<AiValue>;
    fn all_bi_values(&self) -> Vec<BiValue>;
    /// Iterate over the stored AI values mutably, in the same order as `all_ai_values`.
    fn all_ai_values_mut(&mut self) -> impl Iterator<Item = &mut AiValue>;
    /// Iterate over the stored BI values mutably, in the same order as `all_bi_values`.
    fn all_bi_values_mut(&mut self) -> impl Iterator<Item = &mut BiValue>;
    fn all_values(&self) -> Vec<PointValue> {
        let ai_values = self.all_ai_values().into_iter().map(PointValue::Ai);
        let bi_values = self.all_bi_values().into_iter().map(PointValue::Bi);
        ai_values.chain(bi_values).collect::<Vec<PointValue>>()
    }
    /// Mark an existing point key as explicitly written in the internal tracker.
    fn mark_written(&mut self, point_key: PointIndex);
    /// Update the stored value for the typed point. Returns `true` if the point was found.
    /// Mark the point as written only when a matching point is found.
    fn update_value(&mut self, point: PointValue) -> bool {
        let point_key = point.key();
        let found = match point {
            PointValue::Ai(new_value) => {
                if let Some(ai) = self
                    .all_ai_values_mut()
                    .find(|ai| ai.index == new_value.index)
                {
                    ai.value = new_value.value;
                    true
                } else {
                    false
                }
            }
            PointValue::Bi(new_value) => {
                if let Some(bi) = self
                    .all_bi_values_mut()
                    .find(|bi| bi.index == new_value.index)
                {
                    bi.value = new_value.value;
                    true
                } else {
                    false
                }
            }
        };
        if found {
            self.mark_written(point_key);
        }
        found
    }
    /// Return a blank copy of this entry: same vector structure and point keys as `self`,
    /// analog values zeroed to 0, binary values set to false, and the written tracker empty.
    fn create_blank_instance(&self) -> Self;
    /// Return only the point values that have been explicitly written (via `update_value`).
    fn received_values(&self) -> Vec<PointValue>;
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::collections::HashSet;

    use super::*;

    #[derive(Clone)]
    pub(crate) struct MixedEntry {
        ai: AiValue,
        bi: BiValue,
        written: HashSet<PointIndex>,
    }

    impl MixedEntry {
        pub(crate) fn new(ai_index: u16, bi_index: u16) -> Self {
            Self {
                ai: AiValue::new(ai_index, 0),
                bi: BiValue {
                    index: bi_index,
                    value: false,
                },
                written: HashSet::new(),
            }
        }
    }

    impl DatabaseEntry for MixedEntry {
        fn all_ai_values(&self) -> Vec<AiValue> {
            vec![self.ai]
        }

        fn all_bi_values(&self) -> Vec<BiValue> {
            vec![self.bi]
        }

        fn all_ai_values_mut(&mut self) -> impl Iterator<Item = &mut AiValue> {
            std::iter::once(&mut self.ai)
        }

        fn all_bi_values_mut(&mut self) -> impl Iterator<Item = &mut BiValue> {
            std::iter::once(&mut self.bi)
        }

        fn mark_written(&mut self, point_key: PointIndex) {
            self.written.insert(point_key);
        }

        fn create_blank_instance(&self) -> Self {
            Self {
                ai: AiValue::new(self.ai.index, 0),
                bi: BiValue {
                    index: self.bi.index,
                    value: false,
                },
                written: HashSet::new(),
            }
        }

        fn received_values(&self) -> Vec<PointValue> {
            self.all_values()
                .into_iter()
                .filter(|point| self.written.contains(&point.key()))
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{test_support::MixedEntry, *};

    #[test]
    fn mixed_entry_updates_analog_and_binary_points_independently() {
        let initial_binary = BiValue {
            index: 7,
            value: false,
        };
        let mut entry = MixedEntry::new(7, 7);
        let analog = PointValue::Ai(AiValue::new(7, 42));
        let binary = PointValue::Bi(BiValue {
            index: 7,
            value: true,
        });

        assert!(entry.update_value(analog));
        assert_eq!(
            entry.all_values(),
            vec![analog, PointValue::Bi(initial_binary)]
        );
        assert_eq!(entry.received_values(), vec![analog]);

        assert!(entry.update_value(binary));
        assert_eq!(entry.received_values(), vec![analog, binary]);
    }
}
