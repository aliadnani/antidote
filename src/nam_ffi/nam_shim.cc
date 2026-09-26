#include <memory>
#include <filesystem>
#include <iostream>

#include "nam_core/NAM/get_dsp.h"
#include "nam_core/NAM/dsp.h"
#include "nam_shim.h"


std::unique_ptr<nam::DSP> load_nam_a2_model_path(rust::Str model_path_str) {
    std::filesystem::path model_path = std::string(model_path_str);

    std::unique_ptr<nam::DSP> dsp = nam::get_dsp(model_path);

    dsp->ResetAndPrewarm(48000.0, 64);

    return dsp;
}

double get_nam_a2_model_expected_sample_rate(const nam::DSP& dsp) {
    // Fallible - but whatever
    return dsp.GetExpectedSampleRate();

}

void process_block_with_nam_a2_model(nam::DSP& dsp, rust::Slice<const float> input, rust::Slice<float> output, const int num_frames) {
    float* _input[1] = { const_cast<float*>(input.data()) };
    float* _output[1] = { output.data() };

    dsp.process(_input, _output, num_frames);
}