#pragma once

#include <memory>

#include "nam_core/NAM/get_dsp.h"
#include "nam_core/NAM/dsp.h"

using NamA2Model = nam::DSP;

std::unique_ptr<nam::DSP> load_nam_a2_model_path();

double get_nam_a2_model_expected_sample_rate(const nam::DSP& dsp); 