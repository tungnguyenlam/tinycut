use dioxus::prelude::*;
use tinycut_core_domain::{InferenceMode, ShieldConfig};

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut mode = use_signal(|| InferenceMode::Local);
    let mut nda_private = use_signal(|| true);
    let shield_cfg = use_signal(ShieldConfig::default);

    rsx! {
        main { style: "font-family: system-ui, sans-serif; padding: 24px; max-width: 960px; margin: 0 auto;",
            h1 { "TinyCut" }
            p { "Fast, local-first AI tricks for creators — bilingual subtitles, smart cuts, and visual privacy shield." }

            section { style: "display: flex; gap: 12px; margin: 16px 0;",
                button {
                    onclick: move |_| mode.set(InferenceMode::Local),
                    "Local (Free / Offline)"
                }
                button {
                    disabled: *nda_private.read(),
                    onclick: move |_| mode.set(InferenceMode::CloudBoost),
                    "Cloud Boost (Pro)"
                }
                label { style: "display: flex; align-items: center; gap: 6px;",
                    input {
                        r#type: "checkbox",
                        checked: *nda_private.read(),
                        onchange: move |evt| {
                            let locked = evt.checked();
                            nda_private.set(locked);
                            if locked {
                                mode.set(InferenceMode::Local);
                            }
                        }
                    }
                    "NDA / Private Lock"
                }
            }

            div { style: "padding: 12px; border: 1px solid #ddd; border-radius: 8px;",
                p { "Active Engine: {mode:?}" }
                p { "Privacy Shield — Plates: {shield_cfg.read().detect_plates}, Faces: {shield_cfg.read().detect_faces}, Zero-Leak Margin: +{shield_cfg.read().spatial_padding_ratio * 100.0:.0}%" }
            }
        }
    }
}
