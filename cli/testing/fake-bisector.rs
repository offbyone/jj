// Copyright 2025 The Jujutsu Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::exit;

use itertools::Itertools as _;

fn main() {
    let edit_script_path = PathBuf::from(env::var_os("BISECTION_SCRIPT").unwrap());
    let commit_to_test = env::var_os("JJ_BISECT_TARGET")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let edit_script = fs::read_to_string(&edit_script_path).unwrap();

    let mut instructions = edit_script.split('\0').collect_vec();
    if let Some(pos) = instructions.iter().position(|&i| i == "next invocation\n") {
        // Overwrite the edit script. The next time `fake-bisector` is called, it will
        // only see the part after the `next invocation` command.
        fs::write(&edit_script_path, instructions[pos + 1..].join("\0")).unwrap();
        instructions.truncate(pos);
    }
    for instruction in instructions {
        let (command, payload) = instruction.split_once('\n').unwrap_or((instruction, ""));
        let parts = command.split(' ').collect_vec();
        match parts.as_slice() {
            [""] => {}
            ["fail"] => exit(1),
            ["fail-if-target-is", bad_target_commit] => {
                if commit_to_test == *bad_target_commit {
                    exit(1)
                }
            }
            ["write", path] => {
                fs::write(path, payload).unwrap_or_else(|_| panic!("Failed to write file {path}"));
            }
            _ => {
                eprintln!("fake-bisector: unexpected command: {command}");
                exit(1)
            }
        }
    }
}
