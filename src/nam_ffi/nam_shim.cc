#include <memory>
#include <filesystem>
#include <iostream>

#include "nam_core/NAM/get_dsp.h"
#include "nam_core/NAM/dsp.h"
#include "nam_shim.h"

std::unique_ptr<nam::DSP> load_nam_a2_model_path() {
    std::filesystem::path model_path = "resources/fender_clean.nam";

    std::unique_ptr<nam::DSP> dsp = nam::get_dsp(model_path);

    return dsp;
}

double get_nam_a2_model_expected_sample_rate(const nam::DSP& dsp) {
    // Fallible - but whatever
    return dsp.GetExpectedSampleRate();

}