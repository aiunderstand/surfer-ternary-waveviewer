// Acceptance test for Phase D of the MRCS Studio surfer integration (NFR-4.3).
//
// Verifies that the byte sequence produced by WaveformStreamer.EncodeHeader (C#) is
// accepted by wellen's GHW reader and yields the expected hierarchy.
//
// Test circuit: two probes on Top.DUT
//   1. clk  — Input,  Binary            → std_ulogic  (NineValueBit, 9 literals)
//   2. data — Output, TernaryBalanced   → btern_ulogic (Enum, 11 literals)
//
// Byte layout derived by tracing through WaveformStreamer.EncodeHeader in
// src/MRCSStudio.Waveform/WaveformStreamer.cs.
//
// String-table indices referenced below:
//   0=<anon>(implicit) 1=std_ulogic  2=btern_ulogic
//   3=U  4=X  5=0  6=1  7=Z  8=W  9=L  10=H  11=-  12=+  13=M  14=D
//   15=Top  16=DUT  17=clk  18=data

use std::io::Cursor;
use wellen::viewers;
use wellen::LoadOptions;

fn two_probe_header() -> Vec<u8> {
    vec![
        // ── File header (16 bytes) ─────────────────────────────────────────
        // magic "GHDLwave\n" (9 bytes)
        0x47, 0x48, 0x44, 0x4C, 0x77, 0x61, 0x76, 0x65, 0x0A,
        // h[0]=16  h[1]=0  h[2]=version=1  h[3]=1(LE)  h[4]=word_len=4  h[5]=word_offset=0  h[6]=0
        0x10, 0x00, 0x01, 0x01, 0x04, 0x00, 0x00,

        // ── STR section ────────────────────────────────────────────────────
        // marker "STR\0"
        0x53, 0x54, 0x52, 0x00,
        // 4 zeros | stored_count=17 u32LE (= 18 user strings − 1) | string_size=0 i32LE (informational)
        0x00, 0x00, 0x00, 0x00,   0x11, 0x00, 0x00, 0x00,   0x00, 0x00, 0x00, 0x00,
        // idx 1  "std_ulogic"   (10 chars + 0x00)
        0x73, 0x74, 0x64, 0x5F, 0x75, 0x6C, 0x6F, 0x67, 0x69, 0x63, 0x00,
        // idx 2  "btern_ulogic" (12 chars + 0x00)
        0x62, 0x74, 0x65, 0x72, 0x6E, 0x5F, 0x75, 0x6C, 0x6F, 0x67, 0x69, 0x63, 0x00,
        // idx 3 "U"  idx 4 "X"  idx 5 "0"  idx 6 "1"  idx 7 "Z"
        0x55, 0x00,   0x58, 0x00,   0x30, 0x00,   0x31, 0x00,   0x5A, 0x00,
        // idx 8 "W"  idx 9 "L"  idx 10 "H"  idx 11 "-"
        0x57, 0x00,   0x4C, 0x00,   0x48, 0x00,   0x2D, 0x00,
        // idx 12 "+"  idx 13 "M"  idx 14 "D"
        0x2B, 0x00,   0x4D, 0x00,   0x44, 0x00,
        // idx 15 "Top"  idx 16 "DUT"  idx 17 "clk"  idx 18 "data"
        0x54, 0x6F, 0x70, 0x00,
        0x44, 0x55, 0x54, 0x00,
        0x63, 0x6C, 0x6B, 0x00,
        0x64, 0x61, 0x74, 0x61, 0x00,

        // ── TYP section ────────────────────────────────────────────────────
        // marker "TYP\0"
        0x54, 0x59, 0x50, 0x00,
        // 4 zeros | type_num=2 u32LE
        0x00, 0x00, 0x00, 0x00,   0x02, 0x00, 0x00, 0x00,
        // Type index 0: std_ulogic
        //   GhwRtik::TypeE8=23, name=idx1, 9 literals (idx 3–11): U X 0 1 Z W L H -
        //   9 literals → wellen detects as NineValueBit (matches STD_LOGIC_VALUES)
        0x17, 0x01, 0x09,   0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
        // Type index 1: btern_ulogic
        //   GhwRtik::TypeE8=23, name=idx2, 11 literals: U X - 0 + Z W L M H D
        //   11 literals → does not match 9-value or 2-value → wellen treats as generic Enum
        0x17, 0x02, 0x0B,   0x03, 0x04, 0x0B, 0x05, 0x0C, 0x07, 0x08, 0x09, 0x0D, 0x0A, 0x0E,
        // required trailing zero
        0x00,

        // ── WKT section ────────────────────────────────────────────────────
        // marker "WKT\0"
        0x57, 0x4B, 0x54, 0x00,
        // 4 zeros
        0x00, 0x00, 0x00, 0x00,
        // GhwWellKnownType::StdULogic=3, TypeId=1 (std_ulogic, type-table index 0)
        0x03, 0x01,
        // terminator
        0x00,

        // ── HIE section ────────────────────────────────────────────────────
        // marker "HIE\0"
        0x48, 0x49, 0x45, 0x00,
        // 4 zeros | num_scopes=2 u32LE | num_declared_vars=2 u32LE | max_signal_id=2 u32LE
        0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00,
        // GhwHierarchyKind::Instance=6, name=idx15 "Top"
        0x06, 0x0F,
        // GhwHierarchyKind::Instance=6, name=idx16 "DUT"
        0x06, 0x10,
        // GhwHierarchyKind::PortIn=17, name=idx17 "clk", TypeId=1 (std_ulogic), signal_id=1
        0x11, 0x11, 0x01, 0x01,
        // GhwHierarchyKind::PortOut=18, name=idx18 "data", TypeId=2 (btern_ulogic), signal_id=2
        0x12, 0x12, 0x02, 0x02,
        // GhwHierarchyKind::EndOfScope=15 (closes DUT)
        0x0F,
        // GhwHierarchyKind::EndOfScope=15 (closes Top)
        0x0F,
        // GhwHierarchyKind::End=0
        0x00,

        // ── EOH section ────────────────────────────────────────────────────
        // marker "EOH\0"
        0x45, 0x4F, 0x48, 0x00,
    ]
}

/// NFR-4.3 gate: MRCS-GHW header must parse without error.
#[test]
fn mrcs_ghw_header_parses_without_error() {
    let cursor = Cursor::new(two_probe_header());
    viewers::read_header(cursor, &LoadOptions::default())
        .expect("MRCS-GHW header must parse without error");
}

/// Top-level scope "Top" and child scope "DUT" must be present.
#[test]
fn mrcs_ghw_hierarchy_has_correct_scopes() {
    let cursor = Cursor::new(two_probe_header());
    let result = viewers::read_header(cursor, &LoadOptions::default()).unwrap();
    let h = &result.hierarchy;

    let top_ref = h
        .scopes()
        .find(|s| h[*s].name(h) == "Top")
        .expect("scope 'Top' not found");

    let _dut_ref = h[top_ref]
        .scopes(h)
        .find(|s| h[*s].name(h) == "DUT")
        .expect("scope 'DUT' not found inside 'Top'");
}

/// Two vars must exist under DUT.
#[test]
fn mrcs_ghw_hierarchy_has_two_signals() {
    let cursor = Cursor::new(two_probe_header());
    let result = viewers::read_header(cursor, &LoadOptions::default()).unwrap();
    let h = &result.hierarchy;

    let dut_ref = h
        .scopes()
        .find(|s| h[*s].name(h) == "Top")
        .and_then(|top| h[top].scopes(h).find(|s| h[*s].name(h) == "DUT"))
        .expect("DUT scope not found");

    let var_count = h[dut_ref].vars(h).count();
    assert_eq!(var_count, 2, "expected 2 vars under DUT, got {var_count}");
}

/// "clk" must be typed as std_ulogic (NineValueBit → no enum_type).
#[test]
fn mrcs_ghw_clk_is_std_ulogic() {
    let cursor = Cursor::new(two_probe_header());
    let result = viewers::read_header(cursor, &LoadOptions::default()).unwrap();
    let h = &result.hierarchy;

    let dut_ref = h
        .scopes()
        .find(|s| h[*s].name(h) == "Top")
        .and_then(|top| h[top].scopes(h).find(|s| h[*s].name(h) == "DUT"))
        .expect("DUT scope not found");

    let clk = h[dut_ref]
        .vars(h)
        .find(|v| h[*v].name(h) == "clk")
        .expect("var 'clk' not found under DUT");

    assert_eq!(
        h[clk].vhdl_type_name(h),
        Some("std_ulogic"),
        "clk must have VHDL type 'std_ulogic'"
    );
    assert!(
        h[clk].enum_type(h).is_none(),
        "clk (std_ulogic / NineValueBit) must have no enum_type"
    );
}

/// "data" must be typed as btern_ulogic (generic Enum with 11 literals).
#[test]
fn mrcs_ghw_data_is_btern_ulogic() {
    let cursor = Cursor::new(two_probe_header());
    let result = viewers::read_header(cursor, &LoadOptions::default()).unwrap();
    let h = &result.hierarchy;

    let dut_ref = h
        .scopes()
        .find(|s| h[*s].name(h) == "Top")
        .and_then(|top| h[top].scopes(h).find(|s| h[*s].name(h) == "DUT"))
        .expect("DUT scope not found");

    let data = h[dut_ref]
        .vars(h)
        .find(|v| h[*v].name(h) == "data")
        .expect("var 'data' not found under DUT");

    let (enum_name, enum_lits) = h[data]
        .enum_type(h)
        .expect("var 'data' (btern_ulogic) must have an enum_type");

    assert_eq!(enum_name, "btern_ulogic");
    assert_eq!(
        enum_lits.len(),
        11,
        "btern_ulogic must have 11 literals, got {}",
        enum_lits.len()
    );
}
