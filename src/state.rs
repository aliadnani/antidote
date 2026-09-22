pub struct State {
    nam_models: Vec<ValidNamA2ModelPath>,
    current_model_index: Option<usize>,
    bypass_enabled: bool,
}

pub struct ValidNamA2ModelPath(String);

impl ValidNamA2ModelPath {
    pub fn new(path: String) -> Result<Self, String> {
        // TODO: Implement actual validation logic for the NAM A2 model;
        // 1. Check the file actually exists
        // 2. Check it ends with .nam
        // 3. NAM models are just JSON files, try parse and assert on some of its properties
        //   3.1 For NAM A2 specifically, assert that it's architercture is indeed SlimmableContainer (i.e. A2) 
        Ok(ValidNamA2ModelPath(path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}