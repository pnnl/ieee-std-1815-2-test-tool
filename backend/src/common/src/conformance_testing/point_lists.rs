//! Shared UID point lists for conformance tests.
//!
//! Each function returns a [`ConformancePoints`] struct containing the static UID slices
//! required for a given conformance test. Both the reference control station and the
//! reference outstation call these functions so the point lists stay in one place.

use crate::uids::{ai_uid::AiUid, ao_uid::AoUid, bi_uid::BiUid, bo_uid::BoUid};

/// The set of UID slices required for a single conformance test.
pub struct ConformancePoints {
    /// Analog input UIDs required for this test.
    pub ai: &'static [AiUid],
    /// Binary input UIDs required for this test.
    pub bi: &'static [BiUid],
    /// Analog output UIDs required for this test.
    pub ao: &'static [AoUid],
    /// Binary output UIDs required for this test.
    pub bo: &'static [BoUid],
}

/// Point list for the MON-001 (Monitoring) conformance test.
pub fn mon_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Monitoring_DECP_EcpConnType,
            AiUid::Monitoring_DECP_PhsConnTyp,
            AiUid::Monitoring_SYS_MMXU_ClcTotVA,
            AiUid::Monitoring_SYS_MMXU_Hz,
            AiUid::Monitoring_SYS_MMXU_TotW,
            AiUid::Monitoring_SYS_MMXU_W_phsA_mag,
            AiUid::Monitoring_SYS_MMXU_W_phsB_mag,
            AiUid::Monitoring_SYS_MMXU_W_phsC_mag,
            AiUid::Monitoring_SYS_MMXU_TotVAr,
            AiUid::Monitoring_SYS_MMXU_Var_phsA_mag,
            AiUid::Monitoring_SYS_MMXU_VAr_phsB_mag,
            AiUid::Monitoring_SYS_MMXU_VAr_phsC_mag,
            AiUid::Monitoring_SYS_MMXU_TotPF,
            AiUid::Monitoring_SYS_MMXU_TotVA,
            AiUid::Monitoring_SYS_MMXU_PhV_phsA_mag,
            AiUid::Monitoring_SYS_MMXU_PhV_phsA_ang,
            AiUid::Monitoring_SYS_MMXU_PhV_phsB_mag,
            AiUid::Monitoring_SYS_MMXU_PhV_phsB_ang,
            AiUid::Monitoring_SYS_MMXU_PhV_phsC_mag,
            AiUid::Monitoring_SYS_MMXU_PhV_phsC_ang,
            AiUid::Monitoring_SYS_MMXU_AvPPVPhs,
            AiUid::Monitoring_SYS_MMXU_A_phsA_mag,
            AiUid::Monitoring_SYS_MMXU_A_phsB_mag,
            AiUid::Monitoring_SYS_MMXU_A_phsC_mag,
        ],
        bi: &[],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the ALARM-001 conformance test.
pub fn alarm_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[],
        bi: &[
            BiUid::Alarm_LCCH1_ChLiv,
            BiUid::Alarm_CALH_GrAlm,
            BiUid::Alarm_CALH_GrWrn,
            BiUid::Alarm_CALH_GrInd,
            BiUid::Alarm_DSTO_SocHiWrn,
            BiUid::Alarm_DSTO_SocHiAlm,
            BiUid::Alarm_DSTO_SocLoAlm,
            BiUid::Alarm_DSTO_SohLoAlm,
            BiUid::Alarm_DBAT_IntnTmpHiAlm,
            BiUid::Alarm_DBAT_ExtTmpHiAlm,
            BiUid::Alarm_SYS_MMXU_TotW_range_max,
            BiUid::Alarm_SYS_MMXU_TotW_range_min,
            BiUid::Alarm_SYS_MMXU_TotVAr_range_max,
            BiUid::Alarm_SYS_MMXU_TotVAr_range_min,
            BiUid::Alarm_SYS_MMXU_TotPF_range_max,
            BiUid::Alarm_SYS_MMXU_TotPF_range_min,
            BiUid::Alarm_SYS_MMXU_PhV_phsA_range_max,
            BiUid::Alarm_SYS_MMXU_PhV_phsA_range_min,
            BiUid::Alarm_SYS_MMXU_PhV_phsB_range_max,
            BiUid::Alarm_SYS_MMXU_PhV_phsB_range_min,
            BiUid::Alarm_SYS_MMXU_PhV_phsC_range_max,
            BiUid::Alarm_SYS_MMXU_PhV_phsC_range_min,
        ],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the CONN-001 (Connect) conformance test.
pub fn conn_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[AiUid::Connect_DCTE3_WinTms, AiUid::Connect_DCTE3_RvrtTms],
        bi: &[BiUid::Monitoring_CSWI_Pos_1, BiUid::Monitoring_CSWI_Pos_2],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the SERV-001 (Return to Service / Enter Service) conformance test.
pub fn serv_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Return_to_Service_DCTE1_VHiLim,
            AiUid::Return_to_Service_DCTE1_VLoLim,
            AiUid::Return_to_Service_DCTE1_HzHiLim,
            AiUid::Return_to_Service_DCTE1_HzLoLim,
            AiUid::Return_to_Service_DCTE1_RtnDlTmms,
            AiUid::Return_to_Service_DCTE1_WinTms,
            AiUid::Return_to_Service_DCTE1_RtnRmpTmms,
            AiUid::Cease_DCTE2_WinTms,
            AiUid::Cease_DCTE2_RmpTms,
            AiUid::Cease_DCTE2_RvrtTms,
        ],
        bi: &[
            BiUid::State_DCTE_CeaEgzReqSt_6,
            BiUid::State_DCTE_CeaEgzReqSt_3,
            BiUid::State_DGEN_DEROpSt_synchronizing,
            BiUid::State_DGEN_DEROpSt_onbutdisconnectedandnotready,
            BiUid::Enter_Service_DGEN_PrmConn,
            BiUid::Enter_Service_DGEN_PrmDscon,
        ],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the OP-001 (Operational State) conformance test.
pub fn op_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[],
        bi: &[
            BiUid::State_DGEN_DEROpSt_disconnectedandinmaintenance,
            BiUid::State_DSTO_DEROpSt_disconnectedandblocked,
            BiUid::State_DCTE_CeaEgzReqSt_6,
            BiUid::State_DCTE_CeaEgzReqSt_3,
            BiUid::State_DGEN_DEROpSt_synchronizing,
            BiUid::Enter_Service_DGEN_PrmConn,
            BiUid::Enter_Service_DGEN_PrmDscon,
            BiUid::Monitoring_DGEN_DEROpSt_running,
            BiUid::Monitoring_DGEN_DEROpSt_connectedandgenerating,
            BiUid::Monitoring_DSTO_DEROpSt_connectedandconsuming,
            BiUid::Monitoring_DGEN_DEROpSt_disconnectedandavailable,
            BiUid::Monitoring_DGEN_DEROpSt_disconnectedandblocked,
        ],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the CURVE-001 conformance test.
pub fn curve_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[],
        bi: &[BiUid::Curve_MESA_CrvRef],
        ao: &[AoUid::Curve_DGSMn_InCrv],
        bo: &[],
    }
}

/// Point list for the CURVE-002 conformance test.
pub fn curve_002() -> ConformancePoints {
    ConformancePoints {
        ai: &[],
        bi: &[BiUid::Curve_MESA_CrvRef],
        ao: &[AoUid::Curve_DGSMn_InCrv],
        bo: &[],
    }
}

/// Point list for the CURVE-003 conformance test.
pub fn curve_003() -> ConformancePoints {
    ConformancePoints {
        ai: &[],
        bi: &[BiUid::Curve_MESA_CrvRef],
        ao: &[AoUid::Curve_DGSMn_InCrv],
        bo: &[],
    }
}

/// Point list for the VRT-001 (Volt Ride-Through) conformance test.
pub fn vrt_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Volt_Ride_Through_DHVT_EcpRef,
            AiUid::Volt_Ride_Through_DHVT_MMXN_Vol,
            AiUid::Volt_Ride_Through_DHVT_PTOV1_BlkRef,
            AiUid::Volt_Ride_Through_DLVT_PTUV1_BlkRef,
            AiUid::Volt_Ride_Through_DHVT_PTOV2_BlkRef,
            AiUid::Volt_Ride_Through_DLVT_PTUV2_BlkRef,
        ],
        bi: &[BiUid::Volt_Ride_Through_DHVT_Mod],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the FRT-001 (Freq Ride-Through) conformance test.
pub fn frt_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Freq_Ride_Through_DHFT_EcpRef,
            AiUid::Freq_Ride_Through_DHFT_MMXU_Hz,
            AiUid::Freq_Ride_Through_DHFT_PTOF1_BlkRef,
            AiUid::Freq_Ride_Through_DLFT_PTUF1_BlkRef,
            AiUid::Freq_Ride_Through_DHFT_PTOF2_BlkRef,
            AiUid::Freq_Ride_Through_DLFT_PTUF2_BlkRef,
        ],
        bi: &[BiUid::Freq_Ride_Through_DHFT_Mod],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the FW-001 (Freq-Watt) conformance test.
pub fn fw_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Freq_Watt_DHFW1_ModPrio,
            AiUid::Freq_Watt_DHFW1_WinTms,
            AiUid::Freq_Watt_DHFW1_RmpTms,
            AiUid::Freq_Watt_DHFW1_RvrtTms,
            AiUid::Freq_Watt_DHFW1_EcpRef,
            AiUid::Freq_Watt_DHFW1_MMXU_Hz,
            AiUid::Freq_Watt_DHFW1_HzStr,
            AiUid::Freq_Watt_DHFW1_HzStop,
            AiUid::Freq_Watt_DHFW1_WGra,
            AiUid::Freq_Watt_DHFW1_WChaGra,
            AiUid::Freq_Watt_DLFW1_HzStr,
            AiUid::Freq_Watt_DLFW1_HzStop,
            AiUid::Freq_Watt_DLFW1_WGra,
            AiUid::Freq_Watt_DLFW1_WChaGra,
            AiUid::Freq_Watt_DHFW1_ActStrDlTmms,
            AiUid::Freq_Watt_DHFW1_ActStopDlTmms,
            AiUid::Freq_Watt_DLFW1_OplTmsMax_1,
            AiUid::Freq_Watt_DHFW1_OplTmsMax_2,
            AiUid::Freq_Watt_DHFW1_RpuRte,
            AiUid::Freq_Watt_DHFW1_RpdRteMax,
            AiUid::Freq_Watt_DHFW1_RpuChaRte,
            AiUid::Freq_Watt_DHFW1_RpdChaRteMax,
            AiUid::Freq_Watt_DHFW1_RtnRmpRte,
            AiUid::Freq_Watt_DLFW1_RtnRmpRte,
            AiUid::Freq_Watt_DHFW1_ReqWLim,
            AiUid::Freq_Watt_DHFW1_SocUseMinPct,
            AiUid::Freq_Watt_DHFW1_SocUseMaxPct,
        ],
        bi: &[
            BiUid::Freq_Ride_Through_DHFT_Mod,
            BiUid::Freq_Watt_DHFW_HysEna,
            BiUid::Freq_Watt_DHFW_SnptEna,
        ],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the DRCS-001 (Dynamic Reactive Current Support) conformance test.
pub fn drcs_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Dyn_React_Curr_Supp_DRGS_ModPrio,
            AiUid::Dyn_React_Curr_Supp_DRGS_WinTms,
            AiUid::Dyn_React_Curr_Supp_DRGS_RmpTms,
            AiUid::Dyn_React_Curr_Supp_DRGS_RvrtTms,
            AiUid::Dyn_React_Curr_Supp_DRGS_EcpRef,
            AiUid::Dyn_React_Curr_Supp_DRGS_MMXN_Vol,
            AiUid::Dyn_React_Curr_Supp_DRGS_VAv,
            AiUid::Dyn_React_Curr_Supp_DRGS_ArGraMod,
            AiUid::Dyn_React_Curr_Supp_DRGS_DbVMin,
            AiUid::Dyn_React_Curr_Supp_DRGS_DbVMax,
            AiUid::Dyn_React_Curr_Supp_DRGS_ArGraSag,
            AiUid::Dyn_React_Curr_Supp_DRGS_ArGraSwl,
            AiUid::Dyn_React_Curr_Supp_DRGS_FilTms,
            AiUid::Dyn_React_Curr_Supp_DRGS_BlkZnV,
            AiUid::Dyn_React_Curr_Supp_DRGS_HysBlkZnV,
            AiUid::Dyn_React_Curr_Supp_DRGS_BlkZnTmms,
            AiUid::Dyn_React_Curr_Supp_DRGS_HoldTmms,
            AiUid::Dyn_React_Curr_Supp_DRGS_ReqA,
        ],
        bi: &[BiUid::Dyn_React_Curr_Supp_DRGS_Mod],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the DVW-001 (Dynamic Volt-Watt) conformance test.
pub fn dvw_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Dyn_Volt_Watt_DVWD_ModPrio,
            AiUid::Dyn_Volt_Watt_DVWD_WinTms,
            AiUid::Dyn_Volt_Watt_DVWD_RmpTms,
            AiUid::Dyn_Volt_Watt_DVWD_RvrtTms,
            AiUid::Dyn_Volt_Watt_DVWD_EcpRef,
            AiUid::Dyn_Volt_Watt_DVWD_MMXN_Vol,
            AiUid::Dyn_Volt_Watt_DVWD_VAv,
            AiUid::Dyn_Volt_Watt_DVWD_DelV,
            AiUid::Dyn_Volt_Watt_DVWD_DynVWGra,
            AiUid::Dyn_Volt_Watt_DVWD_VWFilTms,
            AiUid::Dyn_Volt_Watt_DVWD_DbVWLo,
            AiUid::Dyn_Volt_Watt_DVWD_DbVWInverter,
            AiUid::Dyn_Volt_Watt_DVWD_ReqWSet,
        ],
        bi: &[BiUid::Dyn_Volt_Watt_DVWD_Mod],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the APL-001 (Active Power Limit) conformance test.
pub fn apl_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Active_Power_Limit_DWMX_ModPrio,
            AiUid::Active_Power_Limit_DWMX_WinTms,
            AiUid::Active_Power_Limit_DWMX_RmpTms,
            AiUid::Active_Power_Limit_DWMX_RvrtTms,
            AiUid::Active_Power_Limit_DWMX_EcpRef,
            AiUid::Active_Power_Limit_DWMX_MMXU_TotW,
            AiUid::Active_Power_Limit_DWMX_WLimPct,
            AiUid::Active_Power_Limit_DWMN_WLimPct,
        ],
        bi: &[BiUid::Active_Power_Limit_DWMX_Mod],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the CHG-001 (Set Active Power / Charge-Discharge) conformance test.
pub fn chg_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Set_Active_Power_DWGC_ModPrio,
            AiUid::Set_Active_Power_DWGC_WinTms,
            AiUid::Set_Active_Power_DWGC_RmpTms,
            AiUid::Set_Active_Power_DWGC_RvrtTms,
            AiUid::Set_Active_Power_DWGC_GnWPctSpt,
            AiUid::Set_Active_Power_DWGC_OplTmsMax_1,
            AiUid::Set_Active_Power_DWGC_OplTmsMax_2,
            AiUid::Set_Active_Power_DWGC_RpuRte,
            AiUid::Set_Active_Power_DWGC_RpdRteMax,
            AiUid::Set_Active_Power_DWGC_RpuChaRte,
            AiUid::Set_Active_Power_DWGC_RpdChaRteMax,
            AiUid::Set_Active_Power_DWGC_SocUseMinPct,
            AiUid::Set_Active_Power_DWGC_SocUseMaxPct,
        ],
        bi: &[
            BiUid::Set_Active_Power_DWGC_Mod,
            BiUid::Set_Active_Power_DWGC_UseRmpRte,
        ],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the CCM-001 (Coordinated Charge/Discharge Management) conformance test.
pub fn ccm_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Coord_Charge_Dischg_DTCD_ModPrio,
            AiUid::Coord_Charge_Dischg_DTCD_WinTms,
            AiUid::Coord_Charge_Dischg_DTCD_RmpTms,
            AiUid::Coord_Charge_Dischg_DTCD_RvrtTms,
            AiUid::Coord_Charge_Dischg_DTCD_SocUseTgtPct,
            AiUid::Coord_Charge_Dischg_DTCD_DateTgt_Date,
            AiUid::Coord_Charge_Dischg_DTCD_DateTgt_Time,
            AiUid::Coord_Charge_Dischg_DTCD_WhReq,
            AiUid::Coord_Charge_Dischg_DTCD_ChaDurTms,
            AiUid::Coord_Charge_Dischg_DTCD_SocDate,
            AiUid::Coord_Charge_Dischg_DTCD_SocDateTms,
            AiUid::Coord_Charge_Dischg_DTCD_ChaDurMax,
            AiUid::Coord_Charge_Dischg_DTCD_DschDurMax,
        ],
        bi: &[BiUid::Coord_Charge_Dischg_DTCD_Mod],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the APR1-001 (Peak Power Limiting) conformance test.
pub fn apr1_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Peak_Power_Limiting_DWFL1_ModPrio,
            AiUid::Peak_Power_Limiting_DWFL1_WinTms,
            AiUid::Peak_Power_Limiting_DWFL1_RmpTms,
            AiUid::Peak_Power_Limiting_DWFL1_RvrtTms,
            AiUid::Peak_Power_Limiting_DWFL1_EcpRef,
            AiUid::Peak_Power_Limiting_DWFL1_MMXU_TotW,
            AiUid::Peak_Power_Limiting_DWFL1_FolWThr,
            AiUid::Peak_Power_Limiting_DWFL1_FolWPct,
            AiUid::Peak_Power_Limiting_DWFL1_RpuRte,
            AiUid::Peak_Power_Limiting_DWFL1_RpdRte,
            AiUid::Peak_Power_Limiting_DWFL1_ReqW,
        ],
        bi: &[BiUid::Peak_Power_Limiting_DWFL1_Mod],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the APR2-001 (Load Following) conformance test.
pub fn apr2_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Load_Following_DWFL2_ModPrio,
            AiUid::Load_Following_DWFL2_WinTms,
            AiUid::Load_Following_DWFL2_RmpTms,
            AiUid::Load_Following_DWFL2_RvrtTms,
            AiUid::Load_Following_DWFL2_EcpRef,
            AiUid::Load_Following_DWFL2_MMXU_TotW,
            AiUid::Load_Following_DWFL2_FolWThr,
            AiUid::Load_Following_DWFL2_FolWPct,
            AiUid::Load_Following_DWFL2_RpuRte,
            AiUid::Load_Following_DWFL2_RpdRte,
            AiUid::Load_Following_DWFL2_ReqW,
        ],
        bi: &[BiUid::Load_Following_DWFL2_Mod],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the APR3-001 (Generation Following) conformance test.
pub fn apr3_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Generation_Following_DWFL3_ModPrio,
            AiUid::Generation_Following_DWFL3_WinTms,
            AiUid::Generation_Following_DWFL3_RmpTms,
            AiUid::Generation_Following_DWFL3_RvrtTms,
            AiUid::Generation_Following_DWFL3_EcpRef,
            AiUid::Generation_Following_DWFL3_MMXU_TotW,
            AiUid::Generation_Following_DWFL3_FolWThr,
            AiUid::Generation_Following_DWFL3_FolWPct,
            AiUid::Generation_Following_DWFL3_RpuRte,
            AiUid::Generation_Following_DWFL3_RpdRte,
            AiUid::Generation_Following_DWFL3_ReqW,
        ],
        bi: &[BiUid::Generation_Following_DWFL3_Mod],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the AGC-001 (Automatic Generation Control) conformance test.
pub fn agc_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::AGC_DAGC_ModPrio,
            AiUid::AGC_DAGC_WinTms,
            AiUid::AGC_DAGC_RmpTms,
            AiUid::AGC_DAGC_RvrtTms,
            AiUid::AGC_DAGC_WSpt,
            AiUid::AGC_DAGC_RmpUpTms,
            AiUid::AGC_DAGC_RmpDnTms,
            AiUid::AGC_DAGC_RpuRte,
            AiUid::AGC_DAGC_RpdRte,
            AiUid::AGC_DAGC_RpuChaRte,
            AiUid::AGC_DAGC_RpdChaRte,
            AiUid::AGC_DAGC_SocUseMinPct,
            AiUid::AGC_DAGC_SocUseMaxPct,
            AiUid::AGC_DAGC_WMaxAvl,
            AiUid::AGC_DAGC_WMinAvl,
            AiUid::AGC_DAGC_SocExpc,
            AiUid::AGC_DAGC_SocWhExpc,
        ],
        bi: &[BiUid::AGC_DAGC_Mod, BiUid::AGC_DAGC_UseRmpRte],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the APS-001 (Active Power Smoothing) conformance test.
pub fn aps_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Active_Power_Smoothing_DWSM_ModPrio,
            AiUid::Active_Power_Smoothing_DWSM_WinTms,
            AiUid::Active_Power_Smoothing_DWSM_RmpTms,
            AiUid::Active_Power_Smoothing_DWSM_RvrtTms,
            AiUid::Active_Power_Smoothing_DWSM_EcpRef,
            AiUid::Active_Power_Smoothing_DWSM_MMXU_TotW,
            AiUid::Active_Power_Smoothing_DWSM_WSmthGra,
            AiUid::Active_Power_Smoothing_DWSM_WSmthLoLim,
            AiUid::Active_Power_Smoothing_DWSM_WSmthhiLim,
            AiUid::Active_Power_Smoothing_DWSM_FilTms,
            AiUid::Active_Power_Smoothing_DWSM_RpuRte,
            AiUid::Active_Power_Smoothing_DWSM_RpdRte,
            AiUid::Active_Power_Smoothing_DWSM_RpuChaRte,
            AiUid::Active_Power_Smoothing_DWSM_RpdChaRte,
            AiUid::Active_Power_Smoothing_DWSM_ReqWSet,
        ],
        bi: &[BiUid::Active_Power_Smoothing_DWSM_Mod],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the VW-001 (Volt-Watt curve) conformance test.
pub fn vw_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Volt_Watt_DVWC1_ModPrio,
            AiUid::Volt_Watt_DVWC1_WinTms,
            AiUid::Volt_Watt_DVWC1_RmpTms,
            AiUid::Volt_Watt_DVWC1_RvrtTms,
            AiUid::Volt_Watt_DVWC1_EcpRef,
            AiUid::Volt_Watt_DVWC1_MMXN_Vol,
            AiUid::Volt_Watt_DVWC1_VWCrv,
            AiUid::Volt_Watt_DVWC1_ReqWLim,
            AiUid::Volt_Watt_DVWC1_FilTms,
            AiUid::Volt_Watt_DVWC1_OplTmsMax_1,
            AiUid::Volt_Watt_DVWC1_OplTmsMax_2,
            AiUid::Volt_Watt_DVWC1_RpuRte,
            AiUid::Volt_Watt_DVWC1_RpdRte,
            AiUid::Volt_Watt_DVWC1_RpuChaRte,
            AiUid::Volt_Watt_DVWC1_RpdChaRte,
        ],
        bi: &[BiUid::Volt_Watt_DVWC_Mod],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the FWC-001 (Freq-Watt Curve) conformance test.
pub fn fwc_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Freq_Watt_Curve_DHFW2_ModPrio,
            AiUid::Freq_Watt_Curve_DHFW2_WinTms,
            AiUid::Freq_Watt_Curve_DHFW2_RmpTms,
            AiUid::Freq_Watt_Curve_DHFW2_RvrtTms,
            AiUid::Freq_Watt_Curve_DHFW2_EcpRef,
            AiUid::Freq_Watt_Curve_DHFW2_MMXU_Hz,
            AiUid::Freq_Watt_Curve_DHFW2_HzWCrv,
            AiUid::Freq_Watt_Curve_DHFW2_HysCrv,
            AiUid::Freq_Watt_Curve_DLFW2_HysCrv,
            AiUid::Freq_Watt_Curve_DHFW2_ActStrDlTmms,
            AiUid::Freq_Watt_Curve_DHFW2_ActStopDlTmms,
            AiUid::Freq_Watt_Curve_DHFW2_OplTmsMax_1,
            AiUid::Freq_Watt_Curve_DHFW2_OplTmsMax_2,
            AiUid::Freq_Watt_Curve_DHFW2_RpuRte,
            AiUid::Freq_Watt_Curve_DHFW2_RpdRte,
            AiUid::Freq_Watt_Curve_DHFW2_RpuChaRte,
            AiUid::Freq_Watt_Curve_DHFW2_RpdChaRte,
            AiUid::Freq_Watt_Curve_DHFW2_ReqWLim,
            AiUid::Freq_Watt_Curve_DHFW2_SocUseMinPct,
            AiUid::Freq_Watt_Curve_DHFW2_SocUseMaxPct,
        ],
        bi: &[
            BiUid::Freq_Watt_Curve_DHFW2_Mod,
            BiUid::Freq_Watt_Curve_DHFW1_HysEna,
            BiUid::Freq_Watt_Curve_DHFW1_SnptEna,
        ],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the CVAR-001 (Constant VAr) conformance test.
pub fn cvar_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[],
        bi: &[BiUid::State_DGEN_OpnLoopTau, BiUid::Const_Vars_DVAR_Mod],
        ao: &[
            AoUid::Const_Vars_DVAR_ModPrio,
            AoUid::Const_Vars_DVAR_WinTms,
            AoUid::Const_Vars_DVAR_RmpTms,
            AoUid::Const_Vars_DVAR_RvrtTms,
            AoUid::Const_Vars_DVAR_VArTgtPct,
            AoUid::Const_Vars_DVAR_OplTmsMax_1,
            AoUid::Const_Vars_DVAR_OplTmsMax_2,
        ],
        bo: &[],
    }
}

/// Point list for the FPF-001 (Constant Power Factor) conformance test.
pub fn fpf_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Constant_PF_DFPF_ModPrio,
            AiUid::Constant_PF_DFPF_WinTms,
            AiUid::Constant_PF_DFPF_RmpTms,
            AiUid::Constant_PF_DFPF_RvrtTms,
            AiUid::Constant_PF_DFPF_PFGnTgt,
            AiUid::Constant_PF_DFPF_PFLodTgt,
        ],
        bi: &[
            BiUid::State_DFPF_PFGnExtSet,
            BiUid::State_DFPF_PFLodExtSet,
            BiUid::Constant_PF_DFPF,
        ],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the VV-001 (Volt-VAr) conformance test.
pub fn vv_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Volt_VAr_DVVR_ModPrio,
            AiUid::Volt_VAr_DVVR_WinTms,
            AiUid::Volt_VAr_DVVR_RmpTms,
            AiUid::Volt_VAr_DVVR_RvrtTms,
            AiUid::Volt_VAr_DVVR_EcpRef,
            AiUid::Volt_VAr_DVVR_MMXN_Vol,
            AiUid::Volt_VAr_DVVR_VRefSet,
            AiUid::Volt_VAr_DVVR_VVArCrv,
            AiUid::Volt_VAr_DVVR_OplTmsMax_1,
            AiUid::Volt_VAr_DVVR_OplTmsMax_2,
            AiUid::Volt_VAr_DVVR_VRefTmms,
            AiUid::Volt_VAr_DVVR_ReqVAr,
        ],
        bi: &[BiUid::Volt_Var_DVVR],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the WV-001 (Watt-VAr) conformance test.
pub fn wv_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Watt_VAr_DWVR_ModPrio,
            AiUid::Watt_VAr_DWVR_WinTms,
            AiUid::Watt_VAr_DWVR_RmpTms,
            AiUid::Watt_VAr_DWVR_RvrtTms,
            AiUid::Watt_VAr_DWVR_EcpRef,
            AiUid::Watt_VAr_DWVR_MMXU_TotW,
            AiUid::Watt_VAr_DWVR_WVArCrv,
            AiUid::Watt_VAr_DWVR_OplTmsMax_1,
            AiUid::Watt_VAr_DWVR_OplTmsMax_2,
            AiUid::Watt_VAr_DWVR_ReqVAr,
        ],
        bi: &[BiUid::Watt_Var_DWVR],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the PFC-001 (PF Correction) conformance test.
pub fn pfc_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::PF_Correct_DPFC_ModPrio,
            AiUid::PF_Correct_DPFC_WinTms,
            AiUid::PF_Correct_DPFC_RmpTms,
            AiUid::PF_Correct_DPFC_RvrtTms,
            AiUid::PF_Correct_DPFC_EcpRef,
            AiUid::PF_Correct_DFPC_MMXU_TotPF,
            AiUid::PF_Correct_DPFC_PFTrg,
            AiUid::PF_Correct_DPFC_PFCorRef_rangeC_min,
            AiUid::PF_Correct_DPFC_PFCorRef_rangeC_max,
        ],
        bi: &[BiUid::PF_Correct_DPFC_Mod],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the PSIG-001 (Price Signal) conformance test.
pub fn psig_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[
            AiUid::Price_DPRG_ModPrio,
            AiUid::Price_DPRG_WinTms,
            AiUid::Price_DPRG_RmpTms,
            AiUid::Price_DPRG_RvrtTms,
            AiUid::Price_DPRG_PrcRef,
            AiUid::Price_DPRG_OplTmsMax_1,
            AiUid::Price_DPRG_OplTmsMax_2,
        ],
        bi: &[BiUid::Price_DPRG_Mod],
        ao: &[],
        bo: &[],
    }
}

/// Point list for the SCHED-001 (Backward-Compatible Scheduling) conformance test.
pub fn sched_001() -> ConformancePoints {
    ConformancePoints {
        ai: &[],
        bi: &[BiUid::BC_Scheduling_BC_FSCHn_SchdSt_2],
        ao: &[
            AoUid::BC_Scheduling_BC_FSCC_Schd,
            AoUid::BC_Scheduling_BC_FSCCn_Schd,
            AoUid::BC_Scheduling_BC_FSCHn_SchdPrio,
            AoUid::BC_Scheduling_BC_FSCHn_StrTm_Date,
            AoUid::BC_Scheduling_BC_FSCHn_StrTm_Time,
            AoUid::BC_Scheduling_BC_FSCHn_NxtStrTm,
            AoUid::BC_Scheduling_BC_FSCHn_SchdReuse,
            AoUid::BC_Scheduling_BC_FSCHn_NumEntr,
        ],
        bo: &[],
    }
}

/// Point list for the SCHED-002 (IEEE 1815.2 Scheduling) conformance test.
pub fn sched_002() -> ConformancePoints {
    ConformancePoints {
        ai: &[],
        bi: &[
            BiUid::Scheduling_FSCHn_SchdSt_2,
            BiUid::Scheduling_FSCHn_SchdSt_3,
        ],
        ao: &[
            AoUid::Scheduling_FSCC_Schd,
            AoUid::Scheduling_FSCCn_Schd,
            AoUid::Scheduling_FSCHn_SchdPrio,
            AoUid::Scheduling_FSCHn_StrTm_Date,
            AoUid::Scheduling_FSCHn_StrTm_Time,
            AoUid::Scheduling_FSCHn_NxtStrTm,
            AoUid::Scheduling_FSCHn_SchdReuse,
            AoUid::Scheduling_FSCHn_NumEntr,
        ],
        bo: &[],
    }
}
