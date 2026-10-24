window.NEOVEG_QA_REPORT = {
  "generated_at": "2026-10-24T13:41:56.829462-03:00",
  "platform": "windows",
  "outcome": "pass",
  "standards": [
    "ABNT NBR ISO/IEC 25010",
    "ABNT NBR ISO/IEC/IEEE 29119",
    "ABNT NBR ISO/IEC 12207",
    "ABNT NBR ISO/IEC 14764",
    "ABNT NBR ISO/IEC 27001"
  ],
  "pillars": {
    "Maintainability": {
      "label": "Manutenibilidade",
      "status": "pass",
      "passed": 2,
      "total": 2
    },
    "Reliability": {
      "label": "Confiabilidade",
      "status": "pass",
      "passed": 1,
      "total": 1
    },
    "Portability": {
      "label": "Portabilidade",
      "status": "pass",
      "passed": 1,
      "total": 1
    }
  },
  "steps": [
    {
      "name": "rustfmt",
      "pillar": "Maintainability",
      "standard": "ABNT NBR ISO/IEC 25010 (Manutenibilidade) / ISO/IEC 14764",
      "duration_ms": 1806,
      "status": "pass"
    },
    {
      "name": "Clippy (-D warnings)",
      "pillar": "Maintainability",
      "standard": "ABNT NBR ISO/IEC 25010 (Manutenibilidade) / ISO/IEC 14764",
      "duration_ms": 46532,
      "status": "pass"
    },
    {
      "name": "Testes do workspace",
      "pillar": "Reliability",
      "standard": "ABNT NBR ISO/IEC/IEEE 29119 (Testes) / ISO/IEC 25010 (Confiabilidade)",
      "duration_ms": 54403,
      "status": "pass"
    },
    {
      "name": "Build release do workspace",
      "pillar": "Portability",
      "standard": "ABNT NBR ISO/IEC 25010 (Portabilidade) - build nativo pela toolchain unica",
      "duration_ms": 203126,
      "status": "pass"
    }
  ],
  "environment": {
    "rustc": "rustc 1.99.1 (58cf8f9c3 2026-10-13)",
    "cargo": "cargo 1.99.1 (5f94df478 2026-08-27)"
  }
};
