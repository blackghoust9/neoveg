#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
CPP_DIR="${ROOT_DIR}/cpp-extensions"
BUILD_DIR="${CPP_DIR}/build-qa"
COVERAGE_DIR="${ROOT_DIR}/coverage/qa"
DOTNET_RESULTS="${COVERAGE_DIR}/dotnet"
PYTHON_COVERAGE="${COVERAGE_DIR}/python/coverage.xml"
CPP_COVERAGE="${COVERAGE_DIR}/cpp/coverage.xml"

run() {
    local name="$1"
    local working_directory="$2"
    shift 2
    printf '\n== %s ==\n' "${name}"
    (cd "${working_directory}" && "$@")
}

for tool in cmake ninja dotnet cargo clang-tidy ctest pytest gcovr; do
    if ! command -v "${tool}" >/dev/null 2>&1; then
        printf 'Required QA tool not found on PATH: %s\n' "${tool}" >&2
        exit 1
    fi
done

mkdir -p "${COVERAGE_DIR}/python" "${COVERAGE_DIR}/cpp"

run "Configure instrumented C++ QA build" "${ROOT_DIR}" cmake \
    -S "${CPP_DIR}" -B "${BUILD_DIR}" -G Ninja \
    -DCMAKE_BUILD_TYPE=Debug \
    -DNEOVEG_ENABLE_SANITIZERS=ON \
    -DNEOVEG_ENABLE_COVERAGE=ON

run ".NET formatting" "${ROOT_DIR}" dotnet format "${ROOT_DIR}/Neoveg.sln" --verify-no-changes
run "Rust Clippy" "${ROOT_DIR}" cargo clippy \
    --manifest-path "${ROOT_DIR}/rust-core/Cargo.toml" \
    --all-targets --all-features
run "C++ clang-tidy" "${ROOT_DIR}" clang-tidy \
    "${CPP_DIR}/src/neoveg_cpp_engine.cpp" \
    "${CPP_DIR}/whisper.cpp/src/neoveg_whisper_bridge.cpp" \
    -p "${BUILD_DIR}"

run ".NET tests with coverage collection" "${ROOT_DIR}" dotnet test "${ROOT_DIR}/Neoveg.sln" \
    --collect:"XPlat Code Coverage" --results-directory "${DOTNET_RESULTS}"
run "Rust tests" "${ROOT_DIR}" cargo test \
    --manifest-path "${ROOT_DIR}/rust-core/Cargo.toml" --all-targets --all-features
run "Criterion waveform benchmark" "${ROOT_DIR}" cargo bench \
    --manifest-path "${ROOT_DIR}/rust-core/Cargo.toml" --bench audio_peaks
run "Build sanitized C++ QA targets" "${ROOT_DIR}" cmake --build "${BUILD_DIR}" --config Debug
run "C++ tests" "${ROOT_DIR}" ctest --test-dir "${BUILD_DIR}" -C Debug --output-on-failure
run "Python tests with coverage" "${ROOT_DIR}" pytest tests python-backend \
    --cov=python-backend --cov=tests --cov-report=term-missing \
    --cov-report="xml:${PYTHON_COVERAGE}"

run "Rust coverage" "${ROOT_DIR}" cargo llvm-cov \
    --manifest-path "${ROOT_DIR}/rust-core/Cargo.toml" \
    --all-targets --all-features --html \
    --output-dir "${COVERAGE_DIR}/rust"
run "C++ coverage report" "${ROOT_DIR}" gcovr \
    --root "${ROOT_DIR}" --object-directory "${BUILD_DIR}" \
    --filter "${CPP_DIR}/src" --xml-pretty --xml "${CPP_COVERAGE}"

if ! find "${DOTNET_RESULTS}" -name coverage.cobertura.xml -print -quit | grep -q .; then
    printf 'The .NET coverage collector did not produce a coverage.cobertura.xml report.\n' >&2
    exit 1
fi
for report in "${PYTHON_COVERAGE}" "${CPP_COVERAGE}" "${COVERAGE_DIR}/rust/index.html"; do
    if [[ ! -f "${report}" ]]; then
        printf 'Expected coverage report was not generated: %s\n' "${report}" >&2
        exit 1
    fi
done

printf '\nQA suite and coverage verification completed successfully.\n'
