use crate::message::Message;
use crate::translation::{TranslationPreference, ValueKind, VariableInfo};
use crate::wave_container::{ScopeId, VarId, VariableMeta};
use eyre::Result;
use std::borrow::Cow;
use surfer_translation_types::{TranslationResult, Translator, ValueRepr, VariableType, VariableValue};

pub struct EnumTranslator {}

impl Translator<VarId, ScopeId, Message> for EnumTranslator {
    fn name(&self) -> String {
        "Enum".to_string()
    }

    fn translate(&self, meta: &VariableMeta, value: &VariableValue) -> Result<TranslationResult> {
        let str_value = match value {
            VariableValue::BigUint(v) => Cow::Owned(format!(
                "{v:0width$b}",
                width = meta.num_bits.unwrap() as usize
            )),
            VariableValue::String(s) => Cow::Borrowed(s),
        };
        
        // EnumArray: combined signal — split into per-element chunks
        if meta.variable_type == Some(VariableType::EnumArray) {
            let bits_per_element = meta.enum_map.keys()
                .next()
                .map(|k| k.len())
                .unwrap_or(1);
            let s = str_value.as_str();
            let n_elements = s.len() / bits_per_element;
            let mut result = String::new();
            let mut any_error = false;
            for i in 0..n_elements {
                let chunk = &s[i * bits_per_element..(i + 1) * bits_per_element];
                match meta.enum_map.get(chunk) {
                    Some(name) => result.push_str(name),
                    None => {
                        result.push_str(&format!("ERROR({chunk})"));
                    any_error = true;
                }
                }
            }
            return Ok(TranslationResult {
                val: ValueRepr::String(result),
                kind: if any_error { ValueKind::Warn } else { ValueKind::Normal },
                subfields: vec![],
            });
        }

        // Scalar enum — existing logic unchanged
        let (kind, name) = meta
            .enum_map
            .get(str_value.as_str())
            .map(|s| (ValueKind::Normal, s.clone()))
            .unwrap_or((ValueKind::Warn, format!("ERROR ({str_value})")));
        Ok(TranslationResult {
            val: ValueRepr::String(name),
            kind,
            subfields: vec![],
        })
    }

    fn variable_info(&self, _variable: &VariableMeta) -> eyre::Result<VariableInfo> {
        Ok(VariableInfo::Bits)
    }

    fn translates(&self, variable: &VariableMeta) -> Result<TranslationPreference> {
        // Quick fix for TVL
        let excluded = matches!(
            variable.variable_type_name.as_deref(),
            Some("btern_ulogic") | Some("btern_ulogic_vector") |
            Some("btern_logic")  | Some("btern_logic_vector")  |
            Some("kleene") | Some("kleene_vector")
        );
        if variable.enum_map.is_empty()  || excluded {
            Ok(TranslationPreference::No)
        } else {
            Ok(TranslationPreference::Prefer)
        }
    }
}
