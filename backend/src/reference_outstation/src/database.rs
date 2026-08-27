//! Database initialization from config.

use common::profile::validation::Validated;
use common::profile::values::TransmissionI32;
use dnp3::app::Timestamp;
use dnp3::app::measurement::{
    AnalogInput, AnalogOutputStatus, BinaryInput, BinaryOutputStatus, Counter, Flags,
    FrozenCounter, Time,
};
use dnp3::outstation::OutstationHandle;
use dnp3::outstation::database::*;

use common::profile::{DatabaseEntry, PicsProfile, pics_profile::EventClass as ProfileEventClass};

/// Convert a PicsProfile EventClass to a DNP3 EventClass.
fn to_dnp3_event_class(class: &ProfileEventClass) -> Option<EventClass> {
    match class {
        ProfileEventClass::Class1 => Some(EventClass::Class1),
        ProfileEventClass::Class2 => Some(EventClass::Class2),
        ProfileEventClass::Class3 => Some(EventClass::Class3),
        ProfileEventClass::None => None,
    }
}

/// Initialize the outstation database from a PicsProfile.
pub(crate) fn init_database(outstation: &OutstationHandle, profile: &Validated<PicsProfile>) {
    let synced_zero = Time::Synchronized(Timestamp::new(0));

    outstation.transaction(|db| {
        // Binary inputs from profile — initialized to true so conformance tests pass
        // before the control station issues its first BO sync.
        for bi in profile.bi.all_bi_points() {
            let event_class = to_dnp3_event_class(&bi.event_class);
            db.add(
                bi.point_index,
                event_class,
                BinaryInputConfig {
                    s_var: StaticBinaryInputVariation::Group1Var2,
                    e_var: EventBinaryInputVariation::Group2Var2,
                },
            );
            db.update(
                bi.point_index,
                &BinaryInput::new(true, Flags::ONLINE, synced_zero),
                UpdateOptions::no_event(),
            );
        }

        // Binary output status from profile
        for bo in &profile.bo.points {
            db.add(
                bo.point_index,
                Some(EventClass::Class1),
                BinaryOutputStatusConfig::default(),
            );
        }

        // Analog inputs from profile (base + equipment, excluding curves/schedules)
        for ai in profile.ai.base_ai_points() {
            let event_class = to_dnp3_event_class(&ai.event_class);
            db.add(
                ai.point_index,
                event_class,
                AnalogInputConfig {
                    s_var: StaticAnalogInputVariation::Group30Var1,
                    e_var: EventAnalogInputVariation::Group32Var1,
                    deadband: 0.0,
                },
            );

            match TransmissionI32::try_from_engineering(ai.value(), ai.multiplier(), ai.offset) {
                Ok(transmission_value) => {
                    db.update(
                        ai.point_index,
                        &AnalogInput::new(
                            f64::from(transmission_value.0),
                            Flags::ONLINE,
                            synced_zero,
                        ),
                        UpdateOptions::no_event(),
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to scale AI point {} from engineering value {}: {}",
                        ai.point_index,
                        ai.value().0,
                        e
                    );
                }
            }
        }

        // Curve AI points from profile (template entry only — curves are multiplexed, all entries share the same indices)
        if !profile.ai.curves.is_empty() {
            let curve_db = common::profile::CurveDatabase::from_profile(
                profile,
                1,
                common::uids::ai_uid::AiUid::Curve_DGSMn_InCrv,
            );
            for point in curve_db
                .template_entry()
                .iter()
                .flat_map(|e| e.all_values())
            {
                db.add(
                    point.index,
                    Some(EventClass::Class2),
                    AnalogInputConfig {
                        s_var: StaticAnalogInputVariation::Group30Var1,
                        e_var: EventAnalogInputVariation::Group32Var1,
                        deadband: 0.0,
                    },
                );
                db.update(
                    point.index,
                    &AnalogInput::new(point.value.0.into(), Flags::ONLINE, synced_zero),
                    UpdateOptions::no_event(),
                );
            }
        }

        // Schedule BC AI points from profile (template entry only — schedules are multiplexed)
        if !profile.ai.schedules_bc.is_empty() {
            let schedule_bc_db = common::profile::ScheduleBCDatabase::from_profile(
                profile,
                1,
                common::uids::ai_uid::AiUid::BC_Scheduling_BC_FSCC_Schd,
            );
            for point in schedule_bc_db
                .template_entry()
                .iter()
                .flat_map(|e| e.all_values())
            {
                db.add(
                    point.index,
                    Some(EventClass::Class2),
                    AnalogInputConfig {
                        s_var: StaticAnalogInputVariation::Group30Var1,
                        e_var: EventAnalogInputVariation::Group32Var1,
                        deadband: 0.0,
                    },
                );
                db.update(
                    point.index,
                    &AnalogInput::new(point.value.0.into(), Flags::ONLINE, synced_zero),
                    UpdateOptions::no_event(),
                );
            }
        }

        // Schedule AI points from profile (template entry only — schedules are multiplexed)
        if !profile.ai.schedules.is_empty() {
            let schedule_db = common::profile::ScheduleDatabase::from_profile(
                profile,
                1,
                common::uids::ai_uid::AiUid::Scheduling_FSCC_Schd,
            );
            for point in schedule_db
                .template_entry()
                .iter()
                .flat_map(|e| e.all_values())
            {
                db.add(
                    point.index,
                    Some(EventClass::Class2),
                    AnalogInputConfig {
                        s_var: StaticAnalogInputVariation::Group30Var1,
                        e_var: EventAnalogInputVariation::Group32Var1,
                        deadband: 0.0,
                    },
                );
                db.update(
                    point.index,
                    &AnalogInput::new(point.value.0.into(), Flags::ONLINE, synced_zero),
                    UpdateOptions::no_event(),
                );
            }
        }

        // Analog outputs from profile
        for ao in profile.ao.all_ao_points() {
            db.add(
                ao.point_index,
                Some(EventClass::Class2),
                AnalogOutputStatusConfig {
                    s_var: StaticAnalogOutputStatusVariation::Group40Var1,
                    e_var: EventAnalogOutputStatusVariation::Group42Var1,
                    deadband: 0.0,
                },
            );
        }

        // Counters and frozen counters from profile
        for ctr in &profile.ctr {
            db.add(
                ctr.point_index,
                to_dnp3_event_class(&ctr.counter_event_class),
                CounterConfig {
                    s_var: StaticCounterVariation::Group20Var1,
                    e_var: EventCounterVariation::Group22Var1,
                    deadband: 0,
                },
            );
            if ctr.frozen_counter_exists {
                db.add(
                    ctr.point_index,
                    to_dnp3_event_class(&ctr.frozen_counter_event_class),
                    FrozenCounterConfig {
                        s_var: StaticFrozenCounterVariation::Group21Var1,
                        e_var: EventFrozenCounterVariation::Group23Var1,
                        deadband: 0,
                    },
                );
            }
        }
    });

    // Set initial values for BO and AO points (outside transaction is fine for initial setup)
    outstation.transaction(|db| {
        // Binary output status initial values
        for bo in &profile.bo.points {
            db.update(
                bo.point_index,
                &BinaryOutputStatus::new(
                    false,
                    Flags::ONLINE,
                    Time::Synchronized(Timestamp::new(0)),
                ),
                UpdateOptions::no_event(),
            );
        }
        // Analog output status initial values
        for ao in profile.ao.all_ao_points() {
            db.update(
                ao.point_index,
                &AnalogOutputStatus::new(
                    ao.minimum().0 as f64,
                    Flags::ONLINE,
                    Time::Synchronized(Timestamp::new(0)),
                ),
                UpdateOptions::no_event(),
            );
        }
    });
}

/// Re-initialize the outstation database when a new scenario is loaded.
///
/// Removes all points registered for the old profile, then adds all points
/// from the new profile with fresh initial values in a single transaction.
pub(crate) fn reinit_database(
    outstation: &OutstationHandle,
    old_profile: &Validated<PicsProfile>,
    new_profile: &Validated<PicsProfile>,
) {
    outstation.transaction(|db| {
        // Remove all old binary input points
        for bi in old_profile.bi.all_bi_points() {
            <Database as Remove<BinaryInput>>::remove(db, bi.point_index);
        }

        // Remove all old binary output status points
        for bo in &old_profile.bo.points {
            <Database as Remove<BinaryOutputStatus>>::remove(db, bo.point_index);
        }

        // Remove old base AI points
        for ai in old_profile.ai.base_ai_points() {
            <Database as Remove<AnalogInput>>::remove(db, ai.point_index);
        }

        // Remove old curve AI template points
        if !old_profile.ai.curves.is_empty() {
            let curve_db = common::profile::CurveDatabase::from_profile(
                old_profile,
                1,
                common::uids::ai_uid::AiUid::Curve_DGSMn_InCrv,
            );
            for point in curve_db
                .template_entry()
                .iter()
                .flat_map(|e| e.all_values())
            {
                <Database as Remove<AnalogInput>>::remove(db, point.index);
            }
        }

        // Remove old schedule BC AI template points
        if !old_profile.ai.schedules_bc.is_empty() {
            let schedule_bc_db = common::profile::ScheduleBCDatabase::from_profile(
                old_profile,
                1,
                common::uids::ai_uid::AiUid::BC_Scheduling_BC_FSCC_Schd,
            );
            for point in schedule_bc_db
                .template_entry()
                .iter()
                .flat_map(|e| e.all_values())
            {
                <Database as Remove<AnalogInput>>::remove(db, point.index);
            }
        }

        // Remove old schedule AI template points
        if !old_profile.ai.schedules.is_empty() {
            let schedule_db = common::profile::ScheduleDatabase::from_profile(
                old_profile,
                1,
                common::uids::ai_uid::AiUid::Scheduling_FSCC_Schd,
            );
            for point in schedule_db
                .template_entry()
                .iter()
                .flat_map(|e| e.all_values())
            {
                <Database as Remove<AnalogInput>>::remove(db, point.index);
            }
        }

        // Remove all old AO status points
        for ao in old_profile.ao.all_ao_points() {
            <Database as Remove<AnalogOutputStatus>>::remove(db, ao.point_index);
        }

        // Remove all old counter and frozen counter points
        for ctr in &old_profile.ctr {
            <Database as Remove<Counter>>::remove(db, ctr.point_index);
            if ctr.frozen_counter_exists {
                <Database as Remove<FrozenCounter>>::remove(db, ctr.point_index);
            }
        }
    });

    // Add all points from the new profile with fresh initial values
    init_database(outstation, new_profile);
}

pub(crate) fn build_event_buffer_config(profile: &Validated<PicsProfile>) -> EventBufferConfig {
    let bi_count = profile.bi.all_bi_points().len() as u16;
    let bo_count = profile.bo.points.len() as u16;
    let ai_count = {
        // Curves and schedules are multiplexed: all stored entries share the same AI indices.
        // Size the event buffer for the template entry (first entry) only, not the sum of all entries.
        let curve_points = profile
            .ai
            .curves
            .first()
            .map_or(0, |c| c.iter_points().len()) as u16;
        let sched_bc_points = profile
            .ai
            .schedules_bc
            .first()
            .map_or(0, |s| s.iter_points().len()) as u16;
        let sched_points = profile
            .ai
            .schedules
            .first()
            .map_or(0, |s| s.iter_points().len()) as u16;
        profile.ai.base_ai_points().len() as u16 + curve_points + sched_bc_points + sched_points
    };
    let ao_count = profile.ao.all_ao_points().len() as u16;
    let ctr_count = profile.ctr.len() as u16;
    let frozen_ctr_count = profile
        .ctr
        .iter()
        .filter(|c| c.frozen_counter_exists)
        .count() as u16;

    EventBufferConfig::new(
        bi_count * 2,
        0,
        bo_count * 2,
        ctr_count * 2,
        frozen_ctr_count * 2,
        ai_count * 2,
        ao_count * 2,
        0,
    )
}
