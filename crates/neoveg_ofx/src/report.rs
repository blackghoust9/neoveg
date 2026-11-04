//! What the process that loads a plug-in tells the program: the plug-ins a library holds, each with
//! what it described. The two sides speak JSON through the output of the child process (see
//! [`MARKER`]).

use serde::{Deserialize, Serialize};

/// The line of the output that carries the answer: a plug-in may print what it likes before it, and
/// the answer is the last line that starts with this.
pub const MARKER: &str = "NEOVEG_OFX_RESULT ";

/// A parameter a plug-in defined: its name and its kind (`OfxParamTypeDouble`, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParamReport {
    pub name: String,
    pub kind: String,
}

/// One plug-in of a library, as it described itself.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PluginReport {
    /// The identifier in reverse domain form (`com.example.invert`).
    pub identifier: String,
    /// The plug-in API it speaks (`OfxImageEffectPluginAPI`).
    pub api: String,
    pub version: [u32; 2],
    pub label: String,
    pub grouping: String,
    pub description: String,
    /// The contexts it supports (`OfxImageEffectContextFilter`, …).
    pub contexts: Vec<String>,
    pub params: Vec<ParamReport>,
    pub clips: Vec<String>,
    /// What went wrong while it loaded or described itself; `None` when all went well.
    pub problem: Option<String>,
}

/// The answer of the child process: the plug-ins, or why there are none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    Plugins(Vec<PluginReport>),
    Error(String),
}

impl Answer {
    /// The line the child prints.
    pub fn to_line(&self) -> String {
        format!(
            "{MARKER}{}",
            serde_json::to_string(self)
                .unwrap_or_else(|_| "{\"error\":\"resposta ilegível\"}".into())
        )
    }

    /// The answer in the output of the child: the last line that carries the marker.
    pub fn from_output(output: &str) -> Result<Self, String> {
        let line = output
            .lines()
            .rev()
            .find_map(|l| l.strip_prefix(MARKER))
            .ok_or_else(|| "o processo do plug-in não deu resposta".to_string())?;
        serde_json::from_str(line).map_err(|e| format!("resposta do plug-in ilegível: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_answer_is_found_after_whatever_the_plugin_printed() {
        let report = PluginReport {
            identifier: "com.example.x".into(),
            params: vec![ParamReport {
                name: "amount".into(),
                kind: "OfxParamTypeDouble".into(),
            }],
            ..PluginReport::default()
        };
        let answer = Answer::Plugins(vec![report]);
        let output = format!(
            "hello from the plug-in\n{}\ntrailing noise\n",
            answer.to_line()
        );
        assert_eq!(Answer::from_output(&output), Ok(answer));
        // The last answer wins when a plug-in prints something that looks like one
        let two = format!(
            "{}\n{}\n",
            Answer::Error("a".into()).to_line(),
            Answer::Error("b".into()).to_line()
        );
        assert_eq!(Answer::from_output(&two), Ok(Answer::Error("b".into())));
    }

    #[test]
    fn no_answer_and_a_broken_answer_are_errors() {
        assert!(Answer::from_output("nothing here").is_err());
        assert!(Answer::from_output(&format!("{MARKER}{{not json")).is_err());
    }
}
