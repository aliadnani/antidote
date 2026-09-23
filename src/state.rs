pub struct State {
    nam_models: Vec<ValidNamA2ModelPath>,
    current_model_index: Option<usize>,
}

impl State {
    pub fn new(nam_models_dir: &str) -> Result<Self, String> {
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

    pub fn current_model(&self) -> Option<&ValidNamA2ModelPath> {
        self.current_model_index
            .and_then(|index| self.nam_models.get(index))
    }

    pub fn next_model(&mut self) -> Option<&ValidNamA2ModelPath> {
        self.advance_model(1)
    }

    pub fn previous_model(&mut self) -> Option<&ValidNamA2ModelPath> {
        self.advance_model(-1)
    }

    fn advance_model(&mut self, direction: isize) -> Option<&ValidNamA2ModelPath> {
        let len = self.nam_models.len();

        if len == 0 {
            return None;
        }

        let current = self.current_model_index.take().unwrap_or(0) as isize;
        let next = (current + direction).rem_euclid(len as isize) as usize;

        self.current_model_index = Some(next);
        self.nam_models.get(next)
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
