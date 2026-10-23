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
QA_REPORT_JSON="${COVERAGE_DIR}/qa_report.json"
QA_REPORT_JS="${ROOT_DIR}/docs/qa_report.js"
LOGS_DIR="${ROOT_DIR}/logs"
QA_LOG="${LOGS_DIR}/neoveg.log"

# ISO/IEC 25010 & ISO/IEC 29119 quality pillars used to categorize every QA step (ABNT NBR ISO/IEC 25010 / ABNT NBR ISO/IEC 29119).
PILLARS=(Performance Reliability Usability Portability Maintainability)
# Plain function instead of an associative array: macOS ships bash 3.2.
pillar_label() {
    case "$1" in
        Performance) printf 'Eficiencia de Desempenho' ;;
        Reliability) printf 'Confiabilidade' ;;
        Usability) printf 'Usabilidade' ;;
        Portability) printf 'Portabilidade' ;;
        Maintainability) printf 'Manutenibilidade' ;;
    esac
}
STEP_NAMES=()
STEP_PILLARS=()
STEP_STANDARDS=()
STEP_STATUSES=()
STEP_DURATIONS=()
OUTCOME="fail"

qa_log() {
    local level="$1" source="$2" message="$3"
    local line
    line="$(printf '[%s] [%s] [QA/%s] %s' "$(date '+%Y-%m-%d %H:%M:%S')" "${level}" "${source}" "${message}")"
    printf '%s\n' "${line}"
    mkdir -p "${LOGS_DIR}"
    printf '%s\n' "${line}" >> "${QA_LOG}"
}

json_escape() {
    local value="${1//\\/\\\\}"
    value="${value//\"/\\\"}"
    printf '%s' "${value}"
}

docker_environment_json() {
    # Real Docker state for the dashboard (never hard-coded): CLI presence + daemon reachability.
    local client server
    if ! command -v docker >/dev/null 2>&1; then
        printf '{"installed": false, "daemon_running": false, "client_version": null, "server_version": null, "status": "Not Installed / Optional"}'
        return
    fi
    client="$(docker version --format '{{.Client.Version}}' 2>/dev/null | head -n 1)"
    if server="$(docker info --format '{{.ServerVersion}}' 2>/dev/null)" && [[ -n "${server}" ]]; then
        printf '{"installed": true, "daemon_running": true, "client_version": "%s", "server_version": "%s", "status": "Running"}'             "$(json_escape "${client}")" "$(json_escape "${server}")"
    else
        printf '{"installed": true, "daemon_running": false, "client_version": "%s", "server_version": null, "status": "Not Running / Optional"}'             "$(json_escape "${client}")"
    fi
}

write_report() {
    local steps_json="" pillars_json="" i pillar status passed total count=${#STEP_NAMES[@]}
    for ((i = 0; i < count; i++)); do
        [[ -n "${steps_json}" ]] && steps_json+=","
        steps_json+="$(printf '\n    {"name": "%s", "pillar": "%s", "standard": "%s", "status": "%s", "duration_ms": %s}' \
            "$(json_escape "${STEP_NAMES[$i]}")" "${STEP_PILLARS[$i]}" "$(json_escape "${STEP_STANDARDS[$i]}")" \
            "${STEP_STATUSES[$i]}" "${STEP_DURATIONS[$i]}")"
    done
    for pillar in "${PILLARS[@]}"; do
        passed=0 total=0 status="not_run"
        for ((i = 0; i < count; i++)); do
            [[ "${STEP_PILLARS[$i]}" == "${pillar}" ]] || continue
            total=$((total + 1))
            [[ "${STEP_STATUSES[$i]}" == "pass" ]] && passed=$((passed + 1))
            [[ "${STEP_STATUSES[$i]}" == "fail" ]] && status="fail"
        done
        if [[ "${status}" != "fail" && ${total} -gt 0 && ${passed} -eq ${total} ]]; then
            status="pass"
        fi
        [[ -n "${pillars_json}" ]] && pillars_json+=","
        pillars_json+="$(printf '\n    "%s": {"label": "%s", "status": "%s", "passed": %d, "total": %d}' \
            "${pillar}" "$(pillar_label "${pillar}")" "${status}" "${passed}" "${total}")"
    done

    local json
    json="$(cat <<EOF
{
  "generated_at": "$(date -u '+%Y-%m-%dT%H:%M:%SZ')",
  "platform": "$(uname -s | tr '[:upper:]' '[:lower:]')",
  "outcome": "${OUTCOME}",
  "standards": ["ABNT NBR ISO/IEC 25010", "ABNT NBR ISO/IEC/IEEE 29119", "ABNT NBR ISO/IEC 12207", "ABNT NBR ISO/IEC 14764", "ABNT NBR ISO/IEC 27001"],
  "pillars": {${pillars_json}
  },
  "steps": [${steps_json}
  ],
  "environment": {"docker": $(docker_environment_json)}
}
EOF
)"
    mkdir -p "${COVERAGE_DIR}"
    printf '%s\n' "${json}" > "${QA_REPORT_JSON}"
    # docs/index.html is usually opened via file://, where fetch() is blocked; a script include still loads.
    printf 'window.NEOVEG_QA_REPORT = %s;\n' "${json}" > "${QA_REPORT_JS}"
    qa_log "INFO" "Report" "QA report (${OUTCOME}) written to ${QA_REPORT_JSON} and ${QA_REPORT_JS}"
}
trap write_report EXIT

record_step() {
    local name="$1" pillar="$2" standard="$3" status="$4" duration_ms="$5"
    STEP_NAMES+=("${name}")
    STEP_PILLARS+=("${pillar}")
    STEP_STANDARDS+=("${standard}")
    STEP_STATUSES+=("${status}")
    STEP_DURATIONS+=("${duration_ms}")
}

now_ms() {
    local ns
    ns="$(date +%s%N)"
    # BSD date (macOS) has no %N and prints it literally; fall back to second resolution.
    if [[ "${ns}" =~ ^[0-9]+$ ]]; then
        printf '%s' "$(( ns / 1000000 ))"
    else
        printf '%s' "$(( $(date +%s) * 1000 ))"
    fi
}

# run <name> <pillar> <standard> <working-directory> <command...>
run() {
    local name="$1" pillar="$2" standard="$3" working_directory="$4"
    shift 4
    local started status=0
    printf '\n== %s ==\n' "${name}"
    qa_log "INFO" "${pillar}" "START '${name}' | Requisito: ${standard} | Pilar ISO/IEC 25010: $(pillar_label "${pillar}")"
    started="$(now_ms)"
    (cd "${working_directory}" && "$@") || status=$?
    local duration=$(( $(now_ms) - started ))
    if [[ ${status} -ne 0 ]]; then
        record_step "${name}" "${pillar}" "${standard}" "fail" "${duration}"
        qa_log "ERROR" "${pillar}" "FAIL '${name}' | Requisito: ${standard} | exit code ${status}"
        exit "${status}"
    fi
    record_step "${name}" "${pillar}" "${standard}" "pass" "${duration}"
    qa_log "INFO" "${pillar}" "PASS '${name}' (${duration} ms) | Requisito: ${standard}"
}

qa_log "INFO" "Suite" "Neoveg QA suite started (ABNT NBR ISO/IEC 25010 / 29119 / 12207 / 14764 / 27001)."

for tool in cmake ninja dotnet cargo clang-tidy ctest pytest gcovr; do
    if ! command -v "${tool}" >/dev/null 2>&1; then
        qa_log "ERROR" "Suite" "Required QA tool not found on PATH: ${tool}"
        exit 1
    fi
done

mkdir -p "${COVERAGE_DIR}/python" "${COVERAGE_DIR}/cpp"

MAINT="ABNT NBR ISO/IEC 25010 (Manutenibilidade) / ISO/IEC 14764"
COVERAGE_STD="ABNT NBR ISO/IEC/IEEE 29119 (cobertura) / ISO/IEC 12207"

run "Configure instrumented C++ QA build" Portability \
    "ABNT NBR ISO/IEC 25010 (Portabilidade) - build CMake multiplataforma" "${ROOT_DIR}" cmake \
    -S "${CPP_DIR}" -B "${BUILD_DIR}" -G Ninja \
    -DCMAKE_BUILD_TYPE=Debug \
    -DNEOVEG_ENABLE_SANITIZERS=ON \
    -DNEOVEG_ENABLE_COVERAGE=ON

run ".NET formatting" Maintainability "${MAINT}" "${ROOT_DIR}" \
    dotnet format "${ROOT_DIR}/Neoveg.sln" --verify-no-changes
run "Rust Clippy" Maintainability "${MAINT}" "${ROOT_DIR}" cargo clippy \
    --manifest-path "${ROOT_DIR}/rust-core/Cargo.toml" \
    --all-targets --all-features -- -D warnings
run "C++ clang-tidy" Maintainability "${MAINT}" "${ROOT_DIR}" clang-tidy \
    "${CPP_DIR}/src/neoveg_cpp_engine.cpp" \
    "${CPP_DIR}/whisper.cpp/src/neoveg_whisper_bridge.cpp" \
    -p "${BUILD_DIR}"

# The .NET project bundles the native rust-core library: build it from source first
run "Build Rust native library (release)" Portability \
    "ABNT NBR ISO/IEC 25010 (Portabilidade) - biblioteca nativa compilada a partir do codigo-fonte" "${ROOT_DIR}" \
    cargo build --release --manifest-path "${ROOT_DIR}/rust-core/Cargo.toml"
run ".NET tests with coverage collection (incl. Avalonia.Headless UI)" Usability \
    "ABNT NBR ISO/IEC/IEEE 29119 (testes de UI headless) / ISO/IEC 25010 (Usabilidade)" "${ROOT_DIR}" \
    dotnet test "${ROOT_DIR}/Neoveg.sln" \
    --collect:"XPlat Code Coverage" --results-directory "${DOTNET_RESULTS}"
run "Rust tests" Reliability "ABNT NBR ISO/IEC/IEEE 29119 / ISO/IEC 25010 (Confiabilidade)" "${ROOT_DIR}" \
    cargo test --manifest-path "${ROOT_DIR}/rust-core/Cargo.toml" --all-targets --all-features
run "Criterion waveform benchmark" Performance "ABNT NBR ISO/IEC 25010 (Eficiencia de Desempenho)" "${ROOT_DIR}" \
    cargo bench --manifest-path "${ROOT_DIR}/rust-core/Cargo.toml" --bench audio_peaks
run "Build sanitized C++ QA targets" Reliability "ABNT NBR ISO/IEC 27001 (seguranca de memoria: ASan/UBSan)" "${ROOT_DIR}" \
    cmake --build "${BUILD_DIR}" --config Debug
run "C++ tests" Reliability "ABNT NBR ISO/IEC/IEEE 29119 / ISO/IEC 27001 (FFI sob ASan/UBSan)" "${ROOT_DIR}" \
    ctest --test-dir "${BUILD_DIR}" -C Debug --output-on-failure
run "Python tests with coverage" Reliability "ABNT NBR ISO/IEC/IEEE 29119 / ISO/IEC 25010 (Confiabilidade)" "${ROOT_DIR}" \
    pytest tests python-backend \
    --cov=python-backend --cov=tests --cov-report=term-missing \
    --cov-report="xml:${PYTHON_COVERAGE}"

run "Rust coverage" Maintainability "${COVERAGE_STD}" "${ROOT_DIR}" cargo llvm-cov \
    --manifest-path "${ROOT_DIR}/rust-core/Cargo.toml" \
    --all-targets --all-features --html \
    --output-dir "${COVERAGE_DIR}/rust"
run "C++ coverage report" Maintainability "${COVERAGE_STD}" "${ROOT_DIR}" gcovr \
    --root "${ROOT_DIR}" --object-directory "${BUILD_DIR}" \
    --filter "${CPP_DIR}/src" --xml-pretty --xml "${CPP_COVERAGE}"

verify_coverage_reports() {
    if ! find "${DOTNET_RESULTS}" -name coverage.cobertura.xml -print -quit | grep -q .; then
        printf 'The .NET coverage collector did not produce a coverage.cobertura.xml report.\n' >&2
        return 1
    fi
    local report
    for report in "${PYTHON_COVERAGE}" "${CPP_COVERAGE}" "${COVERAGE_DIR}/rust/index.html"; do
        if [[ ! -f "${report}" ]]; then
            printf 'Expected coverage report was not generated: %s\n' "${report}" >&2
            return 1
        fi
    done
}
run "Coverage report verification" Maintainability "ABNT NBR ISO/IEC/IEEE 29119 (evidencia de cobertura)" "${ROOT_DIR}" \
    verify_coverage_reports

OUTCOME="pass"
qa_log "INFO" "Suite" "QA suite and coverage verification completed successfully."
