/**
 * Standalone CLI wrapper for Neoveg's whisper.cpp bridge (`neoveg-whisper-cli`).
 * Allows `SubtitleGeneratorService.cs` to invoke whisper.cpp either via P/Invoke (`neoveg_cpp_engine.dll`)
 * or as an out-of-process CLI executable (`neoveg-whisper-cli --media <path> --lang <code > --duration <sec>`).
 */

#include "neoveg_whisper_bridge.h"
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

int main(int argc, char** argv) {
    const char* media_path = "timeline_audio.wav";
    const char* lang = "pt-BR";
    const char* model = "ggml-base.bin";
    double duration = 8.0;

    for (int i = 1; i < argc; ++i) {
        if ((std::strcmp(argv[i], "--media") == 0 || std::strcmp(argv[i], "-f") == 0) && i + 1 < argc) {
            media_path = argv[++i];
        } else if ((std::strcmp(argv[i], "--lang") == 0 || std::strcmp(argv[i], "-l") == 0) && i + 1 < argc) {
            lang = argv[++i];
        } else if ((std::strcmp(argv[i], "--model") == 0 || std::strcmp(argv[i], "-m") == 0) && i + 1 < argc) {
            model = argv[++i];
        } else if (std::strcmp(argv[i], "--duration") == 0 && i + 1 < argc) {
            duration = std::atof(argv[++i]);
        }
    }

    std::vector<char> out_buf(16384, '\0');
    int32_t written = neoveg_whisper_run_transcription(
        media_path,
        lang,
        model,
        duration,
        out_buf.data(),
        static_cast<int32_t>(out_buf.size())
    );
    if (written > 0) {
        std::printf("%s\n", out_buf.data());
        return 0;
    }
    std::fprintf(stderr, "{\"ok\":false,\"error\":\"whisper transcription failed\"}\n");
    return 1;
}
