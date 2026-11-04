//! Pinned, locally verified native Zipformer model profiles.
#[derive(Clone, Debug, PartialEq)]
pub enum Model {
    Classic,
    Community64,
    Community128,
    Custom(String),
}
impl Model {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value { v if v.starts_with("custom/") && v.len()>7 && v.len()<=71 && v[7..].bytes().all(|b|b.is_ascii_alphanumeric() || b==b'-' || b==b'_') => Ok(Self::Custom(v.into())), "classic"=>Ok(Self::Classic), "community-64"=>Ok(Self::Community64), "community-128"=>Ok(Self::Community128), _=>Err(format!("Invalid KROKO_MODEL={value:?}; expected classic, community-64, community-128 or custom/<name>")) }
    }
    pub fn key(&self) -> &str {
        match self {
            Self::Classic => "classic",
            Self::Community64 => "community-64",
            Self::Community128 => "community-128",
            Self::Custom(id) => id,
        }
    }
    pub fn revision(&self) -> &'static str {
        match self {
            Self::Custom(_) => "local",
            Self::Classic => "887db3d083240198c2d2b99fb66cfcfe6948ced8",
            _ => "d45212aeb212dd66083dd22710c9954f40ff8cc1",
        }
    }
    pub fn tail_samples(&self) -> usize {
        match self {
            Self::Community128 => 43520,
            _ => 23040,
        }
    }
    pub fn hashes(&self) -> [&'static str; 4] {
        match self {
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
}
