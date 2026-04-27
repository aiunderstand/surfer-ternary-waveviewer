use extism_pdk::{plugin_fn, FnResult};
pub use surfer_translation_types::plugin_types::TranslateParams;
use surfer_translation_types::{
    TranslationPreference, TranslationResult, TriRail, ValueKind, ValueRepr,
    VariableInfo, VariableMeta, VariableValue,
};

// GHDL encodes BTERN_ULOGIC as a 4-bit ordinal.
//
//  Index │ Literal │ TriRail │ ValueKind   │ Meaning
//  ──────┼─────────┼─────────┼─────────────┼──────────────────
//   0    │ 'U'     │ Mid     │ Undef       │ Uninitialized
//   1    │ 'X'     │ Mid     │ Undef       │ Forcing Unknown
//   2    │ '-'     │ Low     │ Normal      │ Forcing − (low)
//   3    │ '0'     │ Mid     │ Normal      │ Forcing 0 (zero)
//   4    │ '+'     │ High    │ Normal      │ Forcing + (high)
//   5    │ 'Z'     │ Mid     │ HighImp     │ High Impedance
//   6    │ 'W'     │ Mid     │ Undef       │ Weak Unknown
//   7    │ 'L'     │ Low     │ Weak        │ Weak −
//   8    │ 'M'     │ Mid     │ Weak        │ Weak 0
//   9    │ 'H'     │ High    │ Weak        │ Weak +
//  10    │ 'D'     │ Mid     │ DontCare    │ Don't care
//
// GHDL encodes KLEENE as a 2-bit ordinal.
//
//  Index │ Literal │ TriRail │ ValueKind   │ Meaning
//  ──────┼─────────┼─────────┼─────────────┼──────────────────
//   0    │ false   │ Low     │ Normal      │ Logical false
//   1    │ unk     │ Mid     │ Warn        │ Unknown / neither
//   2    │ true    │ High    │ Normal      │ Logical true
//
// BTERN_ULOGIC_VECTOR / BTERN_LOGIC_VECTOR:
// GHW stores elements in downto order (highest index first).
// Each element is 4 bits. The collapsed view shows all literals
// concatenated as a string, same as Binary format for std_logic_vector.

#[plugin_fn]
pub fn name() -> FnResult<String> {
    Ok("Balanced Ternary".to_string())
}

#[plugin_fn]
pub fn translates(variable: VariableMeta<(), ()>) -> FnResult<TranslationPreference> {
    let pref = match variable.variable_type_name.as_deref() {
        Some("btern_ulogic")        => TranslationPreference::Prefer,
        Some("btern_logic")         => TranslationPreference::Prefer,
        Some("btern_ulogic_vector") => TranslationPreference::Prefer,
        Some("btern_logic_vector")  => TranslationPreference::Prefer,
        Some("kleene")              => TranslationPreference::Prefer,
        Some("kleene_vector")       => TranslationPreference::Prefer,
        _                           => TranslationPreference::No,
    };
    Ok(pref)
}

#[plugin_fn]
pub fn variable_info(variable: VariableMeta<(), ()>) -> FnResult<VariableInfo> {
    match variable.variable_type_name.as_deref() {
        Some("btern_ulogic") | Some("btern_logic") | Some("kleene") => {
            Ok(VariableInfo::Bool)
        }
        _ => Ok(VariableInfo::Bits),
    }
}

#[plugin_fn]
pub fn translate(
    TranslateParams { variable, value }: TranslateParams,
) -> FnResult<TranslationResult> {
    let result = match variable.variable_type_name.as_deref() {
        Some("btern_ulogic_vector") | Some("btern_logic_vector") => {
            let num_bits = variable.num_bits.unwrap_or(0);
            translate_btern_vector(value, num_bits)
        }
        Some("kleene_vector") => {
            let num_bits = variable.num_bits.unwrap_or(0);
            translate_kleene_vector(value, num_bits)
        }
        Some("kleene") => translate_kleene(value),
        Some("btern_ulogic") | Some("btern_logic") => translate_btern(value),
        _ => TranslationResult::single_string("?", ValueKind::Undef),
    };
    Ok(result)
}

// ── scalar BTERN_(U)LOGIC ──────────────────────────────────────

fn translate_btern(value: VariableValue) -> TranslationResult {
    let raw = to_raw(value, 4);

    if raw.chars().any(|c| c != '0' && c != '1') {
        return TranslationResult {
            val: ValueRepr::Trit(TriRail::Mid, "?".to_string()),
            kind: ValueKind::Undef,
            subfields: vec![],
        };
    }

    let idx = u8::from_str_radix(&raw, 2).unwrap_or(0);
    let (rail, label, kind) = btern_ordinal(idx);
    TranslationResult {
        val: ValueRepr::Trit(rail, label.to_string()),
        kind,
        subfields: vec![],
    }
}

// ── vector BTERN_(U)LOGIC_VECTOR ───────────────────────────────

fn translate_btern_vector(value: VariableValue, num_bits: u32) -> TranslationResult {
    let raw = to_raw(value, num_bits);
    let n_elements = num_bits as usize / 4;

    // Build a string of ternary literals, one per element.
    // If any element has non-binary characters, mark that element as '?'.
    let mut display = String::with_capacity(n_elements);
    let mut worst_kind = ValueKind::Normal;

    for i in 0..n_elements {
        let start = i * 4;
        let end   = start + 4;
        let chunk = if end <= raw.len() { &raw[start..end] } else { "xxxx" };

        if chunk.chars().any(|c| c != '0' && c != '1') {
            display.push('?');
            worst_kind = ValueKind::Undef;
        } else {
            let idx = u8::from_str_radix(chunk, 2).unwrap_or(0);
            let (_, label, kind) = btern_ordinal(idx);
        display.push_str(label);
            worst_kind = worse_kind(worst_kind, kind);
        }
    }

    TranslationResult {
        val: ValueRepr::String(display),
        kind: worst_kind,
        subfields: vec![],
    }
}

// ── scalar KLEENE ────────────────────────────────────────────

fn translate_kleene(value: VariableValue) -> TranslationResult {
    let raw = to_raw(value, 2);
 
    if raw.chars().any(|c| c != '0' && c != '1') {
        return TranslationResult {
            val: ValueRepr::Trit(TriRail::Mid, "?".to_string()),
            kind: ValueKind::Undef,
            subfields: vec![],
        };
    }
 
    let idx = u8::from_str_radix(&raw, 2).unwrap_or(0);
    let (rail, label, kind) = kleene_ordinal(idx);
    TranslationResult {
        val: ValueRepr::Trit(rail, label.to_string()),
        kind,
        subfields: vec![],
    }
}

// ── vector KLEENE_VECTOR ─────────────────────────────────────
 
fn translate_kleene_vector(value: VariableValue, num_bits: u32) -> TranslationResult {
    let raw = to_raw(value, num_bits);
    let n_elements = num_bits as usize / 2;

    let labels: Vec<&str> = (0..n_elements)
        .map(|i| {
            let start = i * 2;
            let end   = start + 2;
            let chunk = if end <= raw.len() { &raw[start..end] } else { "xx" };

            if chunk.chars().any(|c| c != '0' && c != '1') {
                "?"
            } else {
                let idx = u8::from_str_radix(chunk, 2).unwrap_or(0);
                let (_, label, _) = kleene_ordinal(idx);
            label
            }
        })
        .collect();

    TranslationResult {
        val: ValueRepr::String(labels.join("")),
        kind: ValueKind::Normal,
        subfields: vec![],
    }
}

// ── helpers ──────────────────────────────────────────────────

/// Map a BTERN_ULOGIC ordinal to (TriRail, label, ValueKind).
fn btern_ordinal(idx: u8) -> (TriRail, &'static str, ValueKind) {
    match idx {
        0  => (TriRail::Mid,  "U",  ValueKind::Undef),
        1  => (TriRail::Mid,  "X",  ValueKind::Undef),
        2  => (TriRail::Low,  "-",  ValueKind::Normal),
        3  => (TriRail::Mid,  "0",  ValueKind::Normal),
        4  => (TriRail::High, "+",  ValueKind::Normal),
        5  => (TriRail::Mid,  "Z",  ValueKind::HighImp),
        6  => (TriRail::Mid,  "W",  ValueKind::Undef),
        7  => (TriRail::Low,  "L",  ValueKind::Weak),
        8  => (TriRail::Mid,  "M",  ValueKind::Weak),
        9  => (TriRail::High, "H",  ValueKind::Weak),
        10 => (TriRail::Mid,  "D",  ValueKind::DontCare),
        _  => (TriRail::Mid,  "?",  ValueKind::Undef),
    }
}

fn kleene_ordinal(idx: u8) -> (TriRail, &'static str, ValueKind) {
    match idx {
        0 => (TriRail::Low,  "false", ValueKind::Normal),
        1 => (TriRail::Mid,  "unk",   ValueKind::Normal),
        2 => (TriRail::High, "true",  ValueKind::Normal),
        _ => (TriRail::Mid,  "?",     ValueKind::Undef),
    }
}

/// Convert a VariableValue to a raw binary string of the given width.
fn to_raw(value: VariableValue, num_bits: u32) -> String {
    match value {
        VariableValue::String(s) => s,
        VariableValue::BigUint(n) => format!("{:0>width$b}", n, width = num_bits as usize),
    }
}

/// Returns the more "alarming" of two ValueKinds, used to color the
/// collapsed vector row based on its worst element.
fn worse_kind(a: ValueKind, b: ValueKind) -> ValueKind {
    if severity(a) >= severity(b) { a } else { b }
}

fn severity(k: ValueKind) -> u8 {
    match k {
        ValueKind::Undef    => 5,
        ValueKind::HighImp  => 4,
        ValueKind::Warn     => 3,
        ValueKind::Weak     => 2,
        ValueKind::DontCare => 1,
        _                   => 0,
    }
}