//! A prototype of an OpenFX host that keeps plug-ins out of the program.
//!
//! OpenFX plug-ins are native libraries written by other people; loading one runs its code with
//! the program's rights, and a plug-in that crashes would end the editor with the user's work in
//! it. So the program never loads them: [`client::describe`] starts a child process
//! (`neoveg_ofx_host`), and the child loads the library, gives the plug-ins the host structure and
//! the suites of [`suites`], runs the actions that make a plug-in say what it is
//! ([`describe::describe_library`]) and prints the answer ([`report`]). What a crash, a hang or
//! rubbish costs the program is an error with the reason.
//!
//! What the prototype does: loads a library or a bundle, lists its image effect plug-ins and, for
//! each, reads the label, the grouping, the description, the contexts, the parameters and the clips
//! it defines. What it does not do yet: create an instance of an effect, move pictures to and from
//! the child, or draw anything (see the notes in the README).

pub mod abi;
pub mod client;
pub mod describe;
pub mod host;
pub mod report;
pub mod store;
pub mod suites;

pub use client::{default_host_exe, describe as describe_in_child, library_of, Options};
pub use host::{DefaultOfxHost, FrameBuffer, OfxHostContext};
pub use report::{Answer, ParamReport, PluginReport};
