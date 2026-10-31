//! Map a corpus row's `method` and `options` columns onto
//! [`openkey_core::Options`].
//!
//! The accepted flag vocabulary is the reference harness's
//! (`scripts/parity/harness.cpp`, `applyOptions`), so a script that runs against
//! the oracle runs unchanged against the port. Unknown flags are errors: a typo
//! must not silently produce a default-configuration case.
//!
//! One flag is refused rather than guessed: `send-key-step-by-step` is host
//! state (`openkey_platform::HostSettings`, ADR-0002 D7), not engine state, and
//! the reference harness does not model it either (`OpenKey.mm:770-782`).

use openkey_core::{InputMethod, Options};

use crate::corpus::Case;

pub fn options(case: &Case) -> Result<Options, String> {
    let mut options = Options {
        input_method: method(&case.method)?,
        ..Options::default()
    };

    for flag in case
        .options
        .split(',')
        .map(str::trim)
        .filter(|flag| !flag.is_empty() && *flag != "-")
    {
        match flag {
            "spelling" => options.check_spelling = true,
            "no-spelling" => options.check_spelling = false,
            "modern-orthography" => options.modern_orthography = true,
            "quick-telex" => options.quick_telex = true,
            "restore-if-wrong-spelling" => options.restore_if_invalid = true,
            "allow-zfwj" => options.allow_consonant_zfwj = true,
            "quick-start-consonant" => options.quick_start_consonant = true,
            "quick-end-consonant" => options.quick_end_consonant = true,
            "smart-switch-key" => options.smart_switch_key = true,
            "no-smart-switch-key" => options.smart_switch_key = false,
            "macro" => options.use_macro = true,
            "no-macro" => options.use_macro = false,
            "macro-in-english" => options.use_macro_in_english = true,
            "auto-caps-macro" => options.auto_caps_macro = true,
            "upper-case-first" => options.upper_case_first_char = true,
            "temp-off-spelling" => options.temp_off_spelling = true,
            "free-mark" => options.free_mark = true,
            "send-key-step-by-step" => {
                return Err(
                    "send-key-step-by-step is host state (openkey_platform::HostSettings), \
                     not engine state (ADR-0002 D7), and the reference harness does not model \
                     it (OpenKey.mm:770-782); no case may request it"
                        .to_string(),
                );
            }
            other => return Err(format!("unknown option flag {other:?}")),
        }
    }
    Ok(options)
}

fn method(name: &str) -> Result<InputMethod, String> {
    match name {
        "telex" => Ok(InputMethod::Telex),
        "vni" => Ok(InputMethod::Vni),
        "simple_telex_1" => Ok(InputMethod::SimpleTelex1),
        "simple_telex_2" => Ok(InputMethod::SimpleTelex2),
        other => Err(format!(
            "unknown method {other:?} (telex, vni, simple_telex_1, simple_telex_2)"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn case(method: &str, flags: &str) -> Case {
        Case {
            id: "t".to_string(),
            method: method.to_string(),
            options: flags.to_string(),
            input: String::new(),
            input_raw: String::new(),
            expected: None,
            source: PathBuf::from("t.tsv"),
            line: 1,
        }
    }

    #[test]
    fn maps_methods_and_flags() {
        let mapped = options(&case("vni", "modern-orthography,quick-telex,no-spelling")).unwrap();
        assert_eq!(mapped.input_method, InputMethod::Vni);
        assert!(mapped.modern_orthography);
        assert!(mapped.quick_telex);
        assert!(!mapped.check_spelling);

        let mapped = options(&case("simple_telex_2", "-")).unwrap();
        assert_eq!(mapped.input_method, InputMethod::SimpleTelex2);
    }

    #[test]
    fn a_dash_and_an_empty_column_both_mean_defaults() {
        assert_eq!(options(&case("telex", "-")).unwrap(), Options::default());
        assert_eq!(options(&case("telex", "")).unwrap(), Options::default());
    }

    #[test]
    fn rejects_unknown_methods_and_flags() {
        assert!(
            options(&case("telexx", "-"))
                .unwrap_err()
                .contains("unknown method")
        );
        assert!(
            options(&case("telex", "speling"))
                .unwrap_err()
                .contains("unknown option flag")
        );
    }

    #[test]
    fn maps_free_mark_and_refuses_host_state() {
        assert!(options(&case("telex", "free-mark")).unwrap().free_mark);
        let err = options(&case("telex", "send-key-step-by-step")).unwrap_err();
        assert!(err.contains("host state"), "{err}");
    }
}
