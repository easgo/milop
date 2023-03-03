#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

use bat::PrettyPrinter;
use clap::Parser;
use colored::Colorize;
use config::Config;
use question::{Answer, Question};
use reqwest::blocking::Client;
use serde_json::json;
use spinners::{Spinner, Spinners};
use std::process::Command;

mod config;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Description of the command to execute
    prompt: Vec<String>,

    /// Run the generated program without asking for confirmation
    #[clap(short = 'y', long)]
    force: bool,
}

fn main() {
    let cli = Cli::parse();
    let config = Config::new();

    let client = Client::new();
    let mut spinner = Spinner::new(Spinners::BouncingBar, "(milo): finding a solution".into());

    let response = client
        .post("https://api.openai.com/v1/completions")
        .json(&json!({
            "top_p": 1,
            "stop": "```",
            "temperature": 0,
            "suffix": "\n```",
            "max_tokens": 2000,
            "presence_penalty": 0,
            "frequency_penalty": 0,
            "model": "text-davinci-003",
            "prompt": build_prompt(&cli.prompt.join(" ")),
        }))
        .header("Authorization", format!("Bearer {}", config.api_key))
        .send()
        .unwrap();

    let status_code = response.status();
    if status_code.is_client_error() {
        let response_body = response.json::<serde_json::Value>().unwrap();
        let error_message = response_body["error"]["message"].as_str().unwrap();
        spinner.stop_and_persist(
            "✖".red().to_string().as_str(),
            format!("milo segfaults.").red().to_string(),
        );
        std::process::exit(1);
    } else if status_code.is_server_error() {
        spinner.stop_and_persist(
            "✖".red().to_string().as_str(),
            format!("milo is currently experiencing problems.")
                .red()
                .to_string(),
        );
        std::process::exit(1);
    }

    let code = response.json::<serde_json::Value>().unwrap()["choices"][0]["text"]
        .as_str()
        .unwrap()
        .trim()
        .to_string();

    spinner.stop_and_persist(
        "".green().to_string().as_str(),
        "(milo): solution solved.".green().to_string(),
    );

    PrettyPrinter::new()
        .input_from_bytes(code.as_bytes())
        .language("bash")
        .grid(true)
        .print()
        .unwrap();

    let should_run = if cli.force {
        true
    } else {
        Question::new(
            "(milo): run my solution? [y/n]"
                .bright_black()
                .to_string()
                .as_str(),
        )
        .yes_no()
        .until_acceptable()
        .default(Answer::YES)
        .ask()
        .expect("(milo): sorry couldn't ask question.")
            == Answer::YES
    };

    if should_run {
        config.write_to_history(code.as_str());
        spinner = Spinner::new(Spinners::BouncingBar, "(milo): executing solution.".into());

        let output = Command::new("bash")
            .arg("-c")
            .arg(code.as_str())
            .output()
            .unwrap_or_else(|_| {
                spinner.stop_and_persist(
                    "✖".red().to_string().as_str(),
                    " (milo): I am a failure.".red().to_string(),
                );
                std::process::exit(1);
            });

        if !output.status.success() {
            spinner.stop_and_persist(
                "✖".red().to_string().as_str(),
                "(milo): I segfaulted.".red().to_string(),
            );
            println!("{}", String::from_utf8_lossy(&output.stderr));
            std::process::exit(1);
        }

        spinner.stop_and_persist(
            "".green().to_string().as_str(),
            "(milo): solution is complete".green().to_string(),
        );

        println!("{}", String::from_utf8_lossy(&output.stdout));
    }
}

fn build_prompt(prompt: &str) -> String {
    let os_hint = if cfg!(target_os = "macos") {
        " (on macOS)"
    } else if cfg!(target_os = "linux") {
        " (on Linux)"
    } else {
        ""
    };

    format!("{prompt}{os_hint}:\n```bash\n#!/bin/bash\n")
}
