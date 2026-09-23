pub struct State {
    nam_models: Vec<ValidNamA2ModelPath>,
    current_model_index: Option<usize>,
}

impl State {
    fn new(nam_models_dir: &str) -> Result<Self, String> {
        let mut nam_models = Vec::new();

        for entry in std::fs::read_dir(nam_models_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "nam") {
                let valid_path = ValidNamA2ModelPath::new(path.to_string_lossy().to_string())?;

                nam_models.push(valid_path);
            }
        }

        nam_models.sort_by(|a, b| a.as_str().cmp(b.as_str()));

        if nam_models.is_empty() {
            return Err("No valid NAM A2 models found in the specified directory.".to_string());
        }

        Ok(State {
            nam_models,
            current_model_index: 0.into(),
        })
    }
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
