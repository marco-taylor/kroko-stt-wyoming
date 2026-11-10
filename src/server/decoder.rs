//! Decoder policy; acoustic/streaming configuration stays with the existing engine.
use crate::models::Model;
use sherpa_onnx::OnlineRecognizerConfig;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decoder {
    pub method: &'static str,
    pub max_active_paths: i32,
    pub blank_penalty: f32,
}

impl Decoder {
    pub fn for_model(model: &Model) -> Self {
        // The quality evidence covers the official German Community-128 only.
        // Other languages and custom exports retain their previous safe default.
        let beam4 = model.language() == "de" && matches!(model.base(), Model::Community128);
        Self {
            method: if beam4 { "modified_beam_search" } else { "greedy_search" },
            max_active_paths: 4,
            blank_penalty: if beam4 { 0.75 } else { 0.0 },
        }
    }

    pub fn apply(self, config: &mut OnlineRecognizerConfig) {
        config.decoding_method = Some(self.method.into());
        config.max_active_paths = self.max_active_paths;
        config.blank_penalty = self.blank_penalty;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn german_community128_matches_confirmed_beam4() {
        let decoder = Decoder::for_model(&Model::Community128);
        let mut config = OnlineRecognizerConfig::default();
        config.enable_endpoint = false;
        decoder.apply(&mut config);
        assert_eq!(config.decoding_method.as_deref(), Some("modified_beam_search"));
        assert_eq!(config.max_active_paths, 4);
        assert_eq!(config.blank_penalty, 0.75);
        assert!(!config.enable_endpoint);
    }

    #[test]
    fn classic_and_community64_keep_greedy() {
        for model in [Model::Classic, Model::Community64] {
            assert_eq!(Decoder::for_model(&model).method, "greedy_search");
            assert_eq!(Decoder::for_model(&model).blank_penalty, 0.0);
        }
    }

    #[test]
    fn custom_model_names_do_not_enable_beam() {
        for name in ["custom/community-128", "custom/classic", "custom/nemotron"] {
            let decoder = Decoder::for_model(&Model::parse(name).unwrap());
            assert_eq!(decoder.method, "greedy_search");
            assert_eq!(decoder.blank_penalty, 0.0);
        }
    }

    #[test]
    fn unvalidated_languages_keep_greedy() {
        for model in [Model::Community64, Model::Community128] {
            let decoder = Decoder::for_model(&model.for_language("en").unwrap());
            assert_eq!(decoder.method, "greedy_search");
            assert_eq!(decoder.blank_penalty, 0.0);
        }
        assert_eq!(Decoder::for_model(&Model::Community128.for_language("de-DE").unwrap()).method, "modified_beam_search");
    }

    #[test]
    fn application_changes_only_decoder_fields() {
        let mut config = OnlineRecognizerConfig::default();
        config.feat_config.sample_rate = 16000;
        config.feat_config.feature_dim = 80;
        config.model_config.num_threads = 1;
        config.enable_endpoint = false;
        Decoder::for_model(&Model::Community128).apply(&mut config);
        assert_eq!(config.feat_config.sample_rate, 16000);
        assert_eq!(config.feat_config.feature_dim, 80);
        assert_eq!(config.model_config.num_threads, 1);
        assert!(!config.enable_endpoint);
    }
}
