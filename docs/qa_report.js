window.NEOVEG_QA_REPORT = {
    "generated_at":  "2026-10-23T16:13:24.8344802-03:00",
    "platform":  "windows",
    "outcome":  "pass",
    "standards":  [
                      "ABNT NBR ISO/IEC 25010",
                      "ABNT NBR ISO/IEC/IEEE 29119",
                      "ABNT NBR ISO/IEC 12207",
                      "ABNT NBR ISO/IEC 14764",
                      "ABNT NBR ISO/IEC 27001"
                  ],
    "pillars":  {
                    "Performance":  {
                                        "label":  "Eficiencia de Desempenho",
                                        "status":  "pass",
                                        "passed":  1,
                                        "total":  1
                                    },
                    "Reliability":  {
                                        "label":  "Confiabilidade",
                                        "status":  "pass",
                                        "passed":  4,
                                        "total":  4
                                    },
                    "Usability":  {
                                      "label":  "Usabilidade",
                                      "status":  "pass",
                                      "passed":  1,
                                      "total":  1
                                  },
                    "Portability":  {
                                        "label":  "Portabilidade",
                                        "status":  "pass",
                                        "passed":  2,
                                        "total":  2
                                    },
                    "Maintainability":  {
                                            "label":  "Manutenibilidade",
                                            "status":  "pass",
                                            "passed":  6,
                                            "total":  6
                                        }
                },
    "steps":  [
                  {
                      "name":  "Configure instrumented C++ QA build",
                      "pillar":  "Portability",
                      "standard":  "ABNT NBR ISO/IEC 25010 (Portabilidade) - build CMake multiplataforma",
                      "status":  "pass",
                      "duration_ms":  1043
                  },
                  {
                      "name":  ".NET formatting",
                      "pillar":  "Maintainability",
                      "standard":  "ABNT NBR ISO/IEC 25010 (Manutenibilidade) / ISO/IEC 14764",
                      "status":  "pass",
                      "duration_ms":  35525
                  },
                  {
                      "name":  "Rust Clippy",
                      "pillar":  "Maintainability",
                      "standard":  "ABNT NBR ISO/IEC 25010 (Manutenibilidade) / ISO/IEC 14764",
                      "status":  "pass",
                      "duration_ms":  2815
                  },
                  {
                      "name":  "C++ clang-tidy",
                      "pillar":  "Maintainability",
                      "standard":  "ABNT NBR ISO/IEC 25010 (Manutenibilidade) / ISO/IEC 14764",
                      "status":  "pass",
                      "duration_ms":  24306
                  },
                  {
                      "name":  "Build Rust native library (release)",
                      "pillar":  "Portability",
                      "standard":  "ABNT NBR ISO/IEC 25010 (Portabilidade) - biblioteca nativa compilada a partir do codigo-fonte",
                      "status":  "pass",
                      "duration_ms":  913
                  },
                  {
                      "name":  ".NET tests with coverage collection (incl. Avalonia.Headless UI)",
                      "pillar":  "Usability",
                      "standard":  "ABNT NBR ISO/IEC/IEEE 29119 (testes de UI headless) / ISO/IEC 25010 (Usabilidade)",
                      "status":  "pass",
                      "duration_ms":  20238
                  },
                  {
                      "name":  "Rust tests",
                      "pillar":  "Reliability",
                      "standard":  "ABNT NBR ISO/IEC/IEEE 29119 / ISO/IEC 25010 (Confiabilidade)",
                      "status":  "pass",
                      "duration_ms":  1074
                  },
                  {
                      "name":  "Criterion waveform benchmark",
                      "pillar":  "Performance",
                      "standard":  "ABNT NBR ISO/IEC 25010 (Eficiencia de Desempenho)",
                      "status":  "pass",
                      "duration_ms":  68403
                  },
                  {
                      "name":  "Build sanitized C++ QA targets",
                      "pillar":  "Reliability",
                      "standard":  "ABNT NBR ISO/IEC 27001 (seguranca de memoria: ASan/UBSan)",
                      "status":  "pass",
                      "duration_ms":  295
                  },
                  {
                      "name":  "C++ tests",
                      "pillar":  "Reliability",
                      "standard":  "ABNT NBR ISO/IEC/IEEE 29119 / ISO/IEC 27001 (FFI sob ASan/UBSan)",
                      "status":  "pass",
                      "duration_ms":  351
                  },
                  {
                      "name":  "Python tests with coverage",
                      "pillar":  "Reliability",
                      "standard":  "ABNT NBR ISO/IEC/IEEE 29119 / ISO/IEC 25010 (Confiabilidade)",
                      "status":  "pass",
                      "duration_ms":  31764
                  },
                  {
                      "name":  "Rust coverage",
                      "pillar":  "Maintainability",
                      "standard":  "ABNT NBR ISO/IEC/IEEE 29119 (cobertura) / ISO/IEC 12207",
                      "status":  "pass",
                      "duration_ms":  37324
                  },
                  {
                      "name":  "C++ coverage report",
                      "pillar":  "Maintainability",
                      "standard":  "ABNT NBR ISO/IEC/IEEE 29119 (cobertura) / ISO/IEC 12207",
                      "status":  "pass",
                      "duration_ms":  2300
                  },
                  {
                      "name":  "Coverage report verification",
                      "pillar":  "Maintainability",
                      "standard":  "ABNT NBR ISO/IEC/IEEE 29119 (evidencia de cobertura)",
                      "status":  "pass",
                      "duration_ms":  0
                  }
              ],
    "environment":  {
                        "docker":  {
                                       "installed":  false,
                                       "daemon_running":  false,
                                       "client_version":  null,
                                       "server_version":  null,
                                       "status":  "Not Installed / Optional"
                                   }
                    }
};
