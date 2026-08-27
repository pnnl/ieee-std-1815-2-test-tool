//! Data handlers: ReadHandler implementation and conformance warnings.

use std::sync::Arc;

use common::profile::values::{EngineeringF64, TransmissionI32};
use dnp3::app::measurement::*;
use dnp3::app::{MaybeAsync, QualifierCode, ResponseHeader, Variation};
use dnp3::master::{HeaderInfo, ReadHandler, ReadType};

use crate::data_record::DataRecord;
use crate::state::ControlStationState;

/// ReadHandler implementation for the reference control station.
pub(crate) struct RefReadHandler {
    state: Arc<ControlStationState>,
    current_read_type: ReadType,
}

impl RefReadHandler {
    /// Create a new boxed read handler.
    pub fn boxed(state: Arc<ControlStationState>) -> Box<dyn ReadHandler> {
        Box::new(Self {
            state,
            current_read_type: ReadType::SinglePoll,
        })
    }

    fn read_type_str(&self) -> &'static str {
        match self.current_read_type {
            ReadType::StartupIntegrity => "integrity",
            ReadType::Unsolicited => "unsolicited",
            ReadType::SinglePoll => "response",
            ReadType::PeriodicPoll => "class_poll",
        }
    }

    fn write_record(&self, record: &DataRecord) {
        if let Some(ref writer) = self.state.data_writer {
            writer.write(record);
        }

        let json_value = serde_json::to_value(record).unwrap_or_default();

        // Store latest value
        let key = format!("{}:{}", record.data_type, record.index);
        self.state
            .latest_transmission_values
            .write()
            .unwrap()
            .insert(key, json_value.clone());

        // Push to event history
        self.state.push_event(json_value);
    }

    fn check_static_qualifier(&self, info: &HeaderInfo, type_name: &str) {
        if !info.is_event {
            match info.qualifier {
                QualifierCode::Range8 | QualifierCode::Range16 => {}
                q => {
                    tracing::warn!(
                        "Conformance: {type_name} static data using unexpected qualifier {q}"
                    );
                }
            }
        }
    }

    fn check_event_qualifier(&self, info: &HeaderInfo, type_name: &str) {
        if info.is_event {
            match info.qualifier {
                // Q07 (Count8): 8-bit limited quantity — used in Function Code (FC) 07/08 limited-quantity event responses
                // Q17 (CountAndPrefix8): 8-bit count + 8-bit prefix index — used in FC 129/130 indexed event responses
                // Q28 (CountAndPrefix16): 16-bit count + 16-bit prefix index — used in FC 129/130 indexed event responses
                QualifierCode::Count8
                | QualifierCode::CountAndPrefix8
                | QualifierCode::CountAndPrefix16 => {}
                q => {
                    tracing::warn!(
                        "Conformance: {type_name} event data using unexpected qualifier {q}"
                    );
                }
            }
        }
    }

    /// Parse numeric group and variation from a [`Variation`] value.
    ///
    /// The dnp3 library's `Display` impl formats `Variation` as `"g{group}v{var}"`
    /// (e.g. `"g40v2"`). Returns `(0, 0)` and logs a warning if the format is unexpected.
    fn parse_group_variation(variation: Variation) -> (u8, u8) {
        // Display output format is "g{group}v{var}" (e.g. "g40v2").
        let s = format!("{variation}");
        if let Some(s) = s.strip_prefix('g') {
            if let Some(pos) = s.find('v') {
                let group: u8 = s[..pos].parse().unwrap_or(0);
                let var: u8 = s[pos + 1..].parse().unwrap_or(0);
                return (group, var);
            }
        }
        tracing::warn!(
            "parse_group_variation: unexpected variation format '{variation}'; record will have group=0 var=0"
        );
        (0, 0)
    }

    /// Enrich a record using the PICS profile for metadata and scaling.
    fn enrich_and_validate(
        &self,
        record: &mut DataRecord,
        point_type: DataPointType,
        raw_value: TransmissionI32,
    ) {
        let profile_index_guard = self.state.profile_index.read().unwrap();
        let profile_index = &*profile_index_guard;

        match point_type {
            DataPointType::BinaryInput => {
                if let Some(bi) = profile_index.bi_points.get(&record.index) {
                    record.description = Some(bi.name.clone());
                    record.uid = Some(bi.iec_61850_uid.clone());
                    record.purpose = Some(bi.purpose.clone());
                } else {
                    tracing::warn!(
                        "Profile conformance: unknown binary input index {}",
                        record.index
                    );
                }
            }
            DataPointType::BinaryOutput => {
                if let Some(bo) = profile_index.bo_points.get(&record.index) {
                    record.description = Some(bo.name.clone());
                    record.uid = Some(bo.iec_61850_uid.clone());
                    record.purpose = Some(bo.purpose.clone());
                } else {
                    tracing::warn!(
                        "Profile conformance: unknown binary output index {}",
                        record.index
                    );
                }
            }
            DataPointType::AnalogInput => {
                if let Some(ai) = profile_index.ai_points.get(&record.index) {
                    record.description = Some(ai.name.clone());
                    record.uid = Some(ai.iec_61850_uid.clone());
                    record.purpose = Some(ai.purpose.clone());
                    if !ai.units.is_empty() {
                        record.units = Some(ai.units.clone());
                    }
                    record.engineering_value = Some(
                        EngineeringF64::from_transmitted(raw_value, ai.multiplier(), ai.offset).0,
                    );
                    if raw_value < ai.minimum() || raw_value > ai.maximum {
                        tracing::warn!(
                            "Profile conformance: AI {} value {} out of range [{}, {}]",
                            record.index,
                            raw_value,
                            ai.minimum(),
                            ai.maximum()
                        );
                        self.state.record_ai_range_violation(
                            record.index,
                            raw_value,
                            ai.minimum(),
                            ai.maximum(),
                        );
                    }
                } else {
                    tracing::warn!(
                        "Profile conformance: unknown analog input index {}",
                        record.index
                    );
                }
            }
            DataPointType::AnalogOutput => {
                if let Some(ao) = profile_index.ao_points.get(&record.index) {
                    record.description = Some(ao.name.clone());
                    record.uid = Some(ao.iec_61850_uid.clone());
                    record.purpose = Some(ao.purpose.clone());
                    if !ao.units.is_empty() {
                        record.units = Some(ao.units.clone());
                    }
                    record.engineering_value = Some(
                        EngineeringF64::from_transmitted(raw_value, ao.multiplier(), ao.offset).0,
                    );
                    if raw_value < ao.minimum() || raw_value > ao.maximum() {
                        tracing::warn!(
                            "Profile conformance: AO {} value {} out of range [{}, {}]",
                            record.index,
                            raw_value,
                            ao.minimum(),
                            ao.maximum()
                        );
                        self.state.record_ao_range_violation(
                            record.index,
                            raw_value,
                            ao.minimum(),
                            ao.maximum(),
                        );
                    }
                } else {
                    tracing::warn!(
                        "Profile conformance: unknown analog output index {}",
                        record.index
                    );
                }
            }
        }
    }
    /// Route an incoming AI value into the appropriate mux database if the index belongs
    /// to a curve, backward-compatible schedule, or IEEE 1815.2 schedule data point.
    ///
    /// The selector AI readback sets the active local entry. Other AI values are then
    /// stored in that entry. All entries share the same data-point AI indices, so the
    /// database's `all_points()` list provides the membership test.
    fn update_mux_database_single_point(&self, ai_index: u16, raw_value: f64) {
        let int_value = TransmissionI32(raw_value as i32);

        {
            let mut db = self.state.outstation_curve_database.write().unwrap();
            let selector_ai_index = db.selector_ai_uid() as u16;
            if ai_index == selector_ai_index {
                let Ok(curve_number) = u16::try_from(int_value.0) else {
                    tracing::warn!(
                        "Control station received invalid curve selector AI{ai_index} = {} from outstation",
                        int_value.0
                    );
                    return;
                };
                if db.set_active_entry(curve_number).is_some() {
                    tracing::debug!(
                        "Control station received selector AI{ai_index} = {curve_number}; local readback database now routes curve AI points to entry {curve_number}"
                    );
                } else {
                    tracing::warn!(
                        "Control station received out-of-range curve selector AI{ai_index} = {curve_number} from outstation"
                    );
                }
            } else if db
                .current_entry_points()
                .iter()
                .any(|v| v.index == ai_index)
            {
                tracing::debug!(
                    "Control station curve {}: received outstation readback AI{ai_index} = {}",
                    db.current_entry(),
                    int_value.0
                );
                db.update_value(ai_index, int_value);
            }
        }
        {
            let mut db = self.state.schedule_bc_database.write().unwrap();
            let selector_ai_index = db.selector_ai_uid() as u16;
            if ai_index == selector_ai_index {
                let Ok(schedule_number) = u16::try_from(int_value.0) else {
                    tracing::warn!(
                        "Control station received invalid backward-compatible schedule selector AI{ai_index} = {} from outstation",
                        int_value.0
                    );
                    return;
                };
                if db.set_active_entry(schedule_number).is_some() {
                    tracing::debug!(
                        "Control station received selector AI{ai_index} = {schedule_number}; local readback database now routes backward-compatible schedule AI points to entry {schedule_number}"
                    );
                } else {
                    tracing::warn!(
                        "Control station received out-of-range backward-compatible schedule selector AI{ai_index} = {schedule_number} from outstation"
                    );
                }
            } else if db
                .current_entry_points()
                .iter()
                .any(|v| v.index == ai_index)
            {
                db.update_value(ai_index, int_value);
            }
        }
        {
            let mut db = self.state.schedule_database.write().unwrap();
            let selector_ai_index = db.selector_ai_uid() as u16;
            if ai_index == selector_ai_index {
                let Ok(schedule_number) = u16::try_from(int_value.0) else {
                    tracing::warn!(
                        "Control station received invalid IEEE 1815.2 schedule selector AI{ai_index} = {} from outstation",
                        int_value.0
                    );
                    return;
                };
                if db.set_active_entry(schedule_number).is_some() {
                    tracing::debug!(
                        "Control station received selector AI{ai_index} = {schedule_number}; local readback database now routes IEEE 1815.2 schedule AI points to entry {schedule_number}"
                    );
                } else {
                    tracing::warn!(
                        "Control station received out-of-range IEEE 1815.2 schedule selector AI{ai_index} = {schedule_number} from outstation"
                    );
                }
            } else if db
                .current_entry_points()
                .iter()
                .any(|v| v.index == ai_index)
            {
                db.update_value(ai_index, int_value);
            }
        }
    }
}

/// Point type discriminant for enrich_and_validate.
enum DataPointType {
    BinaryInput,
    BinaryOutput,
    AnalogInput,
    AnalogOutput,
}

impl ReadHandler for RefReadHandler {
    fn begin_fragment(&mut self, read_type: ReadType, header: ResponseHeader) -> MaybeAsync<()> {
        self.current_read_type = read_type;

        tracing::debug!(
            "Fragment: read_type={:?} iin1={:#04X} iin2={:#04X}",
            read_type,
            header.iin.iin1.value,
            header.iin.iin2.value
        );

        // Update last IIN
        {
            let mut iin = self.state.last_iin.write().unwrap();
            *iin = Some((header.iin.iin1.value, header.iin.iin2.value));
        }

        MaybeAsync::ready(())
    }

    fn end_fragment(&mut self, _read_type: ReadType, _header: ResponseHeader) -> MaybeAsync<()> {
        MaybeAsync::ready(())
    }

    /// Handle binary input and binary input event objects from the outstation.
    ///
    /// **DNP3 spec:**
    /// - Group 1 Var 1 (Binary Input – packed format): static, FC 129, Qualifier 00 or 01
    /// - Group 1 Var 2 (Binary Input – with flags): static, FC 129, Qualifier 00 or 01
    /// - Group 2 Var 2 (Binary Input Event – with absolute time): FC 129 Q17/28, FC 07/08, FC 130
    /// - Group 2 Var 3 (Binary Input Event – with relative time): same as Var 2
    fn handle_binary_input(
        &mut self,
        info: HeaderInfo,
        iter: &mut dyn Iterator<Item = (BinaryInput, u16)>,
    ) {
        self.check_static_qualifier(&info, "binary_input");
        self.check_event_qualifier(&info, "binary_input");

        // Conformance: check variation
        match info.variation {
            Variation::Group1Var1
            | Variation::Group1Var2
            | Variation::Group2Var2
            | Variation::Group2Var3 => {}
            v => {
                tracing::warn!("Conformance: binary input using unexpected variation {v}");
            }
        }

        let (group, var) = Self::parse_group_variation(info.variation);

        for (binary_input, idx) in iter {
            log_flags("binary_input", idx, binary_input.flags);

            let (dnp3_time, time_quality) = DataRecord::time_fields(&binary_input.time);
            let mut record = DataRecord::new(
                self.read_type_str(),
                "binary_input",
                group,
                var,
                &format!("{}", info.qualifier),
                idx,
                serde_json::Value::Bool(binary_input.value),
                binary_input.flags.value,
                dnp3_time,
                time_quality,
            );

            self.enrich_and_validate(
                &mut record,
                DataPointType::BinaryInput,
                if binary_input.value {
                    TransmissionI32::from(1)
                } else {
                    TransmissionI32::from(0)
                },
            );

            self.write_record(&record);
        }
    }

    /// Handle binary output status objects from the outstation. TODO: document
    ///
    /// **DNP3 spec:**
    /// - Group 10 Var 2 (Binary Output – output status with flags): static, FC 129, Qualifier 00 or 01
    fn handle_binary_output_status(
        &mut self,
        info: HeaderInfo,
        iter: &mut dyn Iterator<Item = (BinaryOutputStatus, u16)>,
    ) {
        self.check_static_qualifier(&info, "binary_output");

        match info.variation {
            Variation::Group10Var2 => {} // Process normally
            Variation::Group10Var1 => {
                return; // ignore packed format
            }
            Variation::Group11Var2 => {
                return; // ignore binary output events
            }
            v => {
                tracing::warn!("Conformance: binary output status using unexpected variation {v}");
            }
        }

        let (group, var) = Self::parse_group_variation(info.variation);

        for (binary_output_status, idx) in iter {
            log_flags("binary_output_status", idx, binary_output_status.flags);

            let (dnp3_time, time_quality) = DataRecord::time_fields(&binary_output_status.time);
            let mut record = DataRecord::new(
                self.read_type_str(),
                "binary_output",
                group,
                var,
                &format!("{}", info.qualifier),
                idx,
                serde_json::Value::Bool(binary_output_status.value),
                binary_output_status.flags.value,
                dnp3_time,
                time_quality,
            );

            self.enrich_and_validate(
                &mut record,
                DataPointType::BinaryOutput,
                if binary_output_status.value {
                    TransmissionI32(1)
                } else {
                    TransmissionI32(0)
                },
            );

            self.write_record(&record);
        }
    }

    /// Handle counter and counter event objects from the outstation.
    ///
    /// **DNP3 spec:**
    /// - Group 20 Var 1 (Counter – 32-bit with flag): static, FC 129, Qualifier 00 or 01
    /// - Group 22 Var 1 / Var 2 (Counter Event – 32-bit without/with time): FC 07/08 or FC 129 Q17/28
    fn handle_counter(&mut self, info: HeaderInfo, iter: &mut dyn Iterator<Item = (Counter, u16)>) {
        self.check_static_qualifier(&info, "counter");
        self.check_event_qualifier(&info, "counter");

        match info.variation {
            Variation::Group20Var1 | Variation::Group22Var1 | Variation::Group22Var2 => {}
            v => {
                tracing::warn!("Conformance: counter using unexpected variation {v}");
            }
        }

        let (group, var) = Self::parse_group_variation(info.variation);

        for (counter, idx) in iter {
            log_flags("counter", idx, counter.flags);
            let (dnp3_time, time_quality) = DataRecord::time_fields(&counter.time);
            let record = DataRecord::new(
                self.read_type_str(),
                "counter",
                group,
                var,
                &format!("{}", info.qualifier),
                idx,
                serde_json::json!(counter.value),
                counter.flags.value,
                dnp3_time,
                time_quality,
            );

            self.write_record(&record);
        }
    }

    /// Handle frozen counter objects from the outstation.
    ///
    /// **DNP3 spec:**
    /// - Group 21 Var 1 (Frozen Counter – 32-bit with flag): FC 129, Qualifier 00 or 01
    /// - Group 21 Var 9 (Frozen Counter – 32-bit without flag): FC 129, Qualifier 00 or 01
    fn handle_frozen_counter(
        &mut self,
        info: HeaderInfo,
        iter: &mut dyn Iterator<Item = (FrozenCounter, u16)>,
    ) {
        self.check_static_qualifier(&info, "frozen_counter");

        match info.variation {
            Variation::Group21Var1 | Variation::Group21Var9 => {}
            v => {
                tracing::warn!("Conformance: frozen counter using unexpected variation {v}");
            }
        }

        let (group, var) = Self::parse_group_variation(info.variation);

        for (frozen_counter, idx) in iter {
            log_flags("frozen_counter", idx, frozen_counter.flags);
            let (dnp3_time, time_quality) = DataRecord::time_fields(&frozen_counter.time);
            let record = DataRecord::new(
                self.read_type_str(),
                "frozen_counter",
                group,
                var,
                &format!("{}", info.qualifier),
                idx,
                serde_json::json!(frozen_counter.value),
                frozen_counter.flags.value,
                dnp3_time,
                time_quality,
            );

            self.write_record(&record);
        }
    }

    /// Handle analog input and analog input event objects from the outstation.
    ///
    /// **DNP3 spec:**
    /// - Group 30 Var 1 (Analog Input – 32-bit with flag): static, FC 129, Qualifier 00 or 01
    /// - Group 32 Var 1 (Analog Input Event – 32-bit without time): FC 129 Q17/28, FC 130
    fn handle_analog_input(
        &mut self,
        info: HeaderInfo,
        iter: &mut dyn Iterator<Item = (AnalogInput, u16)>,
    ) {
        self.check_static_qualifier(&info, "analog_input");
        self.check_event_qualifier(&info, "analog_input");

        match info.variation {
            Variation::Group30Var1 | Variation::Group32Var1 => {}
            v => {
                tracing::warn!("Conformance: analog input using unexpected variation {v}");
            }
        }

        let (group, var) = Self::parse_group_variation(info.variation);

        for (analog_input, idx) in iter {
            log_flags("analog_input", idx, analog_input.flags);
            let (dnp3_time, time_quality) = DataRecord::time_fields(&analog_input.time);
            let mut record = DataRecord::new(
                self.read_type_str(),
                "analog_input",
                group,
                var,
                &format!("{}", info.qualifier),
                idx,
                serde_json::json!(analog_input.value),
                analog_input.flags.value,
                dnp3_time,
                time_quality,
            );

            self.enrich_and_validate(
                &mut record,
                DataPointType::AnalogInput,
                TransmissionI32(analog_input.value as i32),
            ); // The cast is okay here since we validate it's 32-bit above.

            // Route mux AI values (curve/schedule data points) into the readback databases.
            self.update_mux_database_single_point(idx, analog_input.value);

            self.write_record(&record);
        }
    }

    /// Handle analog output status objects from the outstation.
    ///
    /// **DNP3 spec:**
    /// - Group 40 Var 2 (Analog Output Status – 16-bit with flag): static, FC 129, Qualifier 00 or 01
    fn handle_analog_output_status(
        &mut self,
        info: HeaderInfo,
        iter: &mut dyn Iterator<Item = (AnalogOutputStatus, u16)>,
    ) {
        self.check_static_qualifier(&info, "analog_output");

        match info.variation {
            // Must be variation 1 to handle 32-bit AO values.
            Variation::Group40Var1 => {} // Process normally
            Variation::Group42Var1 => {
                return;
            } // ignore analog output events
            v => {
                tracing::warn!("Conformance: analog output status using unexpected variation {v}");
            }
        }

        let (group, var) = Self::parse_group_variation(info.variation);

        for (x, idx) in iter {
            let (dnp3_time, time_quality) = DataRecord::time_fields(&x.time);
            let mut record = DataRecord::new(
                self.read_type_str(),
                "analog_output",
                group,
                var,
                &format!("{}", info.qualifier),
                idx,
                serde_json::json!(x.value),
                x.flags.value,
                dnp3_time,
                time_quality,
            );

            self.enrich_and_validate(
                &mut record,
                DataPointType::AnalogOutput,
                TransmissionI32(x.value as i32),
            ); // The cast is okay here since we validate it's 32-bit above.

            self.write_record(&record);
        }
    }
}

/// Make it easier to read debug logs of Flags.
fn flags_to_strings(flags: &Flags) -> Vec<&str> {
    let mut result = Vec::new();

    if flags.is_set(Flags::ONLINE) {
        result.push("ONLINE");
    }
    if flags.is_set(Flags::RESTART) {
        result.push("RESTART");
    }
    if flags.is_set(Flags::COMM_LOST) {
        result.push("COMM_LOST");
    }
    if flags.is_set(Flags::REMOTE_FORCED) {
        result.push("REMOTE_FORCED");
    }
    if flags.is_set(Flags::LOCAL_FORCED) {
        result.push("LOCAL_FORCED");
    }
    if flags.is_set(Flags::CHATTER_FILTER) {
        result.push("CHATTER_FILTER");
    }
    if flags.is_set(Flags::OVER_RANGE) {
        result.push("OVER_RANGE");
    }
    if flags.is_set(Flags::ROLL_OVER) {
        result.push("ROLL_OVER");
    }
    if flags.is_set(Flags::DISCONTINUITY) {
        result.push("DISCONTINUITY");
    }
    if flags.is_set(Flags::REFERENCE_ERR) {
        result.push("REFERENCE_ERR");
    }

    result
}

fn log_flags(input_type: &str, idx: u16, flags: Flags) {
    if flags.is_set(Flags::OVER_RANGE) {
        tracing::error!(
            "Conformance: {} index {} has OVER_RANGE flag set; the value is likely invalid.",
            input_type,
            idx
        );
    } else if flags.is_set(Flags::COMM_LOST) {
        tracing::error!(
            "Conformance: {} index {} has COMM_LOST flag set; the value may be stale.",
            input_type,
            idx
        );
        // RESTART: the outstation has no new values since it restarted.
    } else if flags.is_set(Flags::ONLINE) | flags.is_set(Flags::RESTART) {
        // do nothing
    } else {
        tracing::warn!(
            "Conformance: {} index {} using unhandled flags {}.",
            input_type,
            idx,
            flags_to_strings(&flags)
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}
