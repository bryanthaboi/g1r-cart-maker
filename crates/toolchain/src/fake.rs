//! A scripted Runner. Rules match on the program plus a leading run of
//! arguments; the longest match wins, so a general rule can be narrowed.

use crate::runner::{CancelToken, Invocation, Output, RunError, Runner};
use std::sync::Mutex;

struct Rule {
    program: String,
    args: Vec<String>,
    outputs: Vec<Output>,
    error: Option<RunError>,
    cancel: Option<CancelToken>,
    used: usize,
}

#[derive(Default)]
struct State {
    rules: Vec<Rule>,
    missing: Vec<String>,
    calls: Vec<Invocation>,
}

#[derive(Default)]
pub struct FakeRunner {
    state: Mutex<State>,
}

impl FakeRunner {
    pub fn new() -> Self {
        Self::default()
    }

    /// The tool is not installed: every invocation of it fails to spawn.
    pub fn missing(&self, program: &str) -> &Self {
        self.state
            .lock()
            .expect("fake runner lock")
            .missing
            .push(program.to_string());
        self
    }

    pub fn on(&self, program: &str, args: &[&str], output: Output) -> &Self {
        self.push(program, args, vec![output], None, None)
    }

    /// Each call consumes the next result; the last one repeats forever.
    pub fn on_seq(&self, program: &str, args: &[&str], outputs: Vec<Output>) -> &Self {
        self.push(program, args, outputs, None, None)
    }

    pub fn on_error(&self, program: &str, args: &[&str], error: RunError) -> &Self {
        self.push(program, args, Vec::new(), Some(error), None)
    }

    /// Flips the token when this rule is hit, standing in for a user cancel.
    pub fn on_cancel(&self, program: &str, args: &[&str], cancel: &CancelToken) -> &Self {
        self.push(
            program,
            args,
            vec![Output::cancelled()],
            None,
            Some(cancel.clone()),
        )
    }

    fn push(
        &self,
        program: &str,
        args: &[&str],
        outputs: Vec<Output>,
        error: Option<RunError>,
        cancel: Option<CancelToken>,
    ) -> &Self {
        self.state
            .lock()
            .expect("fake runner lock")
            .rules
            .push(Rule {
                program: program.to_string(),
                args: args.iter().map(|arg| arg.to_string()).collect(),
                outputs,
                error,
                cancel,
                used: 0,
            });
        self
    }

    pub fn calls(&self) -> Vec<Invocation> {
        self.state.lock().expect("fake runner lock").calls.clone()
    }

    /// Every argument array, program included, in call order.
    pub fn argv_log(&self) -> Vec<Vec<String>> {
        self.calls().iter().map(Invocation::argv).collect()
    }

    pub fn argv_for(&self, program: &str) -> Vec<Vec<String>> {
        self.calls()
            .iter()
            .filter(|call| call.program == program)
            .map(Invocation::argv)
            .collect()
    }

    /// The first argument array whose leading tokens are `args`.
    pub fn find(&self, program: &str, args: &[&str]) -> Option<Vec<String>> {
        self.calls()
            .iter()
            .find(|call| call.program == program && starts_with(&call.args, args))
            .map(Invocation::argv)
    }

    pub fn count(&self, program: &str, args: &[&str]) -> usize {
        self.calls()
            .iter()
            .filter(|call| call.program == program && starts_with(&call.args, args))
            .count()
    }
}

fn starts_with(args: &[String], prefix: &[&str]) -> bool {
    prefix.len() <= args.len() && prefix.iter().zip(args).all(|(want, got)| want == got)
}

impl Runner for FakeRunner {
    fn run(&self, invocation: &Invocation, cancel: &CancelToken) -> Result<Output, RunError> {
        let mut state = self.state.lock().expect("fake runner lock");
        state.calls.push(invocation.clone());
        if state.missing.iter().any(|name| name == &invocation.program) {
            return Err(RunError::NotFound(invocation.program.clone()));
        }
        let mut best: Option<usize> = None;
        for (index, rule) in state.rules.iter().enumerate() {
            if rule.program != invocation.program {
                continue;
            }
            let prefix: Vec<&str> = rule.args.iter().map(String::as_str).collect();
            if !starts_with(&invocation.args, &prefix) {
                continue;
            }
            let better = match best {
                Some(current) => rule.args.len() >= state.rules[current].args.len(),
                None => true,
            };
            if better {
                best = Some(index);
            }
        }
        let Some(index) = best else {
            return Err(RunError::Unscripted(invocation.argv().join(" ")));
        };
        let rule = &mut state.rules[index];
        if let Some(error) = &rule.error {
            return Err(error.clone());
        }
        if let Some(token) = &rule.cancel {
            token.cancel();
        }
        let at = rule.used.min(rule.outputs.len().saturating_sub(1));
        rule.used += 1;
        let output = rule.outputs.get(at).cloned().unwrap_or_default();
        if cancel.is_cancelled() {
            return Ok(Output::cancelled());
        }
        Ok(output)
    }
}
