//! Language-qualified model identities and pinned official package catalogue.
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
pub struct CatalogEntry {
    pub language: String,
    pub model: String,
    pub filename: String,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Deserialize)]
pub struct Catalog {
    pub revision: String,
    pub models: Vec<CatalogEntry>,
}
pub fn catalog() -> &'static Catalog {
    static VALUE: OnceLock<Catalog> = OnceLock::new();
    VALUE.get_or_init(|| {
        serde_json::from_str(include_str!("model_catalog.json"))
            .expect("embedded official catalogue is valid")
    })
}
pub fn normalize_language(value: &str) -> Result<String, String> {
    let parts: Vec<_> = value.split('-').collect();
    if parts.is_empty()
        || parts.len() > 2
        || parts[0].len() != 2
        || !parts[0].bytes().all(|b| b.is_ascii_alphabetic())
        || (parts.len() == 2
            && (parts[1].len() != 2 || !parts[1].bytes().all(|b| b.is_ascii_alphabetic())))
    {
        return Err("LANGUAGE must be a two-letter language code or language-region alias".into());
    }
    let lang = parts[0].to_ascii_lowercase();
    Ok(if lang == "iw" { "he".into() } else { lang })
}
#[derive(Clone, Debug, PartialEq)]
pub enum Model {
    Classic,
    Community64,
    Community128,
    Custom(String),
    Localized { language: String, model: Box<Model> },
}
impl Model {
    pub fn base(&self) -> &Self {
        match self {
            Self::Localized { model, .. } => model.base(),
            _ => self,
        }
    }
    pub fn language(&self) -> &str {
        match self {
            Self::Localized { language, .. } => language,
            _ => "de",
        }
    }
    pub fn identity(&self) -> String {
        format!("{}/{}", self.language(), self.key())
    }
    pub fn is_custom(&self) -> bool {
        matches!(self.base(), Self::Custom(_))
    }
    pub fn for_language(&self, value: &str) -> Result<Self, String> {
        let language = normalize_language(value)?;
        let result = if language == "de" {
            self.base().clone()
        } else {
            Self::Localized {
                language: language.clone(),
                model: Box::new(self.base().clone()),
            }
        };
        if !result.is_custom()
            && !(result.language() == "de" && matches!(result.base(), Self::Classic))
            && result.catalog_entry().is_none()
        {
            return Err(format!("Unsupported LANGUAGE={} MODEL={}: no pinned official package; no language fallback",result.language(),result.key()));
        }
        Ok(result)
    }
    pub fn catalog_entry(&self) -> Option<&'static CatalogEntry> {
        catalog()
            .models
            .iter()
            .find(|e| e.language == self.language() && e.model == self.key())
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value { v if v.starts_with("custom/") && v.len()>7 && v.len()<=71 && v[7..].bytes().all(|b|b.is_ascii_alphanumeric() || b==b'-' || b==b'_') => Ok(Self::Custom(v.into())), "classic"=>Ok(Self::Classic), "community-64"=>Ok(Self::Community64), "community-128"=>Ok(Self::Community128), _=>Err(format!("Invalid KROKO_MODEL={value:?}; expected classic, community-64, community-128 or custom/<name>")) }
    }
    pub fn key(&self) -> &str {
        match self.base() {
            Self::Localized { .. } => unreachable!(),
            Self::Classic => "classic",
            Self::Community64 => "community-64",
            Self::Community128 => "community-128",
            Self::Custom(id) => id,
        }
    }
    pub fn revision(&self) -> &'static str {
        match self.base() {
            Self::Custom(_) => "local",
            Self::Classic => "887db3d083240198c2d2b99fb66cfcfe6948ced8",
            _ => catalog().revision.as_str(),
        }
    }
    pub fn tail_samples(&self) -> usize {
        match self.base() {
            Self::Community128 => 43520,
            _ => 23040,
        }
    }
    pub fn hashes(&self) -> [&'static str; 4] {
        if self.language() != "de" {
            return [""; 4];
        }
        match self.base() {
            Self::Localized { .. } => unreachable!(),
            Self::Classic => [
                "6e83993d6967ec7a3498b055b7e85ace85b5d64d1b1e8773cb29a43a11f5edb5",
                "94a29592b403c53fa2231b478637da1ab4abcef7f5e46e432098416a4a3ed562",
                "28356bff070aea51ab1d725a3278e81d19f9300f860d3248a7014292264df15a",
                "86e8370994ff2c01149ba8c4f8709aa93cdc18914b27a717e291e96faf39a6eb",
            ],
            Self::Community64 => [
                "c973664ab297df99faa5b7fc1eb8addabe468a323682cb40af98dc0ec13e985d",
                "57cfda432138d2a552acfac9a052d8f2157eb9a93774d0cbf997601a2cb0aae5",
                "a9e3759823edbe3e78c5cc114384757ebcb43db3b9cb9ce1e13b4be873e116bf",
                "540e317d5b171908ab109dbe9faa37ae8b05e1fa9b8c87fcc40654efba466965",
            ],
            Self::Custom(_) => [""; 4],
            Self::Community128 => [
                "a30969d9e5b003aebed89873118b8f3428f0f15dafc29933e041a7a4f7104eb8",
                "523bd78c36d191d1dcfee976df906e240dddffeee8c4a99df8a5777ff421a28b",
                "3df99212ca3cdb3394eb0d3f318444b95448e2cb5b4140c1a60814bddb3ee750",
                "540e317d5b171908ab109dbe9faa37ae8b05e1fa9b8c87fcc40654efba466965",
            ],
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profiles_are_explicit() {
        assert!(Model::parse("ungueltig").is_err());
        for key in ["classic", "community-64", "community-128"] {
            let m = Model::parse(key).unwrap();
            assert_eq!(m.key(), key);
            assert!(m.hashes().iter().all(|h| h.len() == 64));
        }
        assert_eq!(Model::Community128.tail_samples(), 43520);
    }
    #[test]
    fn official_catalog_and_language_identity_are_explicit() {
        let mut seen = std::collections::HashSet::new();
        for e in &catalog().models {
            assert!(seen.insert((&e.language, &e.model)));
            assert_eq!(e.sha256.len(), 64);
            assert!(e.bytes > 0 && e.filename.ends_with(".data"));
        }
        assert_eq!(catalog().models.len(), 19);
        for kind in [Model::Community64, Model::Community128] {
            let en = kind.for_language("en-US").unwrap();
            assert_ne!(en, kind);
            assert_ne!(en.identity(), kind.identity());
            assert!(en
                .catalog_entry()
                .unwrap()
                .filename
                .starts_with("Kroko-EN-"));
        }
        assert!(Model::Classic.for_language("en").is_err());
        assert!(Model::Community128.for_language("sv").is_err());
        assert!(Model::Community64.for_language("sv").is_ok());
        assert!(Model::Community64.for_language("zz").is_err());
        assert!(Model::Community128.for_language("../de").is_err());
        assert_eq!(
            Model::Community128.for_language("de-DE").unwrap(),
            Model::Community128
        );
    }
}
