use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::read_to_string;
use std::io::Write;
use std::process::{exit, Command, Stdio};

use minimal_yaml::{Yaml, YamlParseError};
use pico_args::Arguments;
use std::num::ParseIntError;
use tempfile::NamedTempFile;
use thiserror::Error;

fn main() {
    match run() {
        Ok(code) => {
            exit(code);
        }
        Err(e) => {
            eprintln!("{}", e.to_string());
            exit(1);
        }
    }
}

fn run() -> Result<i32> {
    let mut args = Arguments::from_env();
    if args.contains(["-h", "--help"]) {
        help();
        return Ok(0);
    }
    let mut args = parse_args(args)?;

    let mut other_services = HashSet::new();
    for file in args.files.iter().take(args.files.len() - 1) {
        let contents = read_to_string(file)?;
        let services = read_services(file, &contents)?;
        for (key, _) in services {
            other_services.insert(key.to_owned());
        }
    }

    let integration_file = args.files.last().unwrap();
    let contents = read_to_string(integration_file)?;
    let test_services = read_services(integration_file, &contents)?;
    let wait_services = services_to_wait(&args.service_names, &test_services)?;
    let mut overlay = NamedTempFile::new()?;

    if !args.service_names.is_empty() {
        // Only enable the selected services
        write_service_overlay(
            &mut overlay,
            &mut other_services,
            &args.service_names,
            &test_services,
        )?;

        args.files
            .push(os_to_str_or_err(overlay.path().as_os_str())?);
    }

    up(&args)?;

    let code = wait_and_log(&args, &wait_services, &other_services)?;

    down(&args)?;

    Ok(code)
}

fn parse_args(mut args: Arguments) -> Result<Args> {
    let project_name = args
        .opt_value_from_str(["-p", "--project-name"])?
        .map(|p| Ok(p))
        .unwrap_or_else(|| current_dir_name())?;

    let verbose = args.contains(["-v", "--verbose"]);

    let mut files: Vec<String> = args.values_from_str(["-f", "--file"])?;
    if files.is_empty() {
        files.push("docker-compose.yml".to_owned());
        files.push("docker-compose.integration-tests.yml".to_owned());
    }
    let service_names = args
        .finish()
        .into_iter()
        .map(|s| os_to_str_or_err(&s))
        .collect::<Result<Vec<String>>>()?;

    let docker = std::env::var("DOCKER").unwrap_or("docker".to_owned());
    let docker_compose = std::env::var("DOCKER_COMPOSE").unwrap_or("docker-compose".to_owned());

    Ok(Args {
        project_name,
        verbose,
        files,
        service_names,
        docker,
        docker_compose,
    })
}

fn current_dir_name() -> Result<String> {
    let dir = std::env::current_dir()?;
    let name = dir.file_name().ok_or(Error::Custom(
        "failed to get file name on current dir".to_owned(),
    ))?;
    os_to_str_or_err(name)
}

fn os_to_str_or_err(s: &OsStr) -> Result<String> {
    Ok(s.to_str()
        .ok_or(Error::Custom("invalid name".to_owned()))?
        .to_owned())
}

fn services_to_wait(service_names: &[String], services: &[(&str, Yaml)]) -> Result<Vec<String>> {
    if service_names.is_empty() {
        let result: Vec<String> = services.iter().map(|&(key, _)| key.to_owned()).collect();
        Ok(result)
    } else {
        Ok(service_names.to_owned())
    }
}

fn write_service_overlay(
    file: &mut NamedTempFile,
    other_services: &mut HashSet<String>,
    service_names: &[String],
    services: &[(&str, Yaml)],
) -> Result<()> {
    writeln!(file, "services:")?;
    for (key, value) in services {
        if service_names.iter().any(|n| n == key) {
            writeln!(file, "  {}:\n    {}", key, value)?;
        } else {
            other_services.remove(key.to_owned());
            writeln!(file, "  {0}:\n    command: echo \"disabled {0}\"", key)?;
        }
    }

    Ok(())
}

fn up(args: &Args) -> Result<()> {
    let mut cmd = Command::new(&args.docker_compose);
    apply_args(&mut cmd, &args).arg("up").arg("-d");
    cmd.spawn()?.wait()?;

    Ok(())
}

fn down(args: &Args) -> Result<()> {
    let mut cmd = Command::new(&args.docker_compose);
    apply_args(&mut cmd, &args).arg("down").arg("-t").arg("0");
    cmd.spawn()?.wait()?;

    Ok(())
}

fn apply_args<'a>(cmd: &'a mut Command, args: &Args) -> &'a mut Command {
    cmd.arg("-p").arg(&args.project_name);
    for file in args.files.iter() {
        cmd.arg("-f").arg(file);
    }
    if !args.verbose {
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
    }
    cmd
}

fn wait_and_log(
    args: &Args,
    wait_services: &[String],
    other_services: &HashSet<String>,
) -> Result<i32> {
    let mut status = 0;
    for service in wait_services {
        let result = wait(args, service)?;
        if result != 0 {
            status = result;
        }
    }
    if args.verbose {
        for service in other_services.iter() {
            if !wait_services.contains(service) {
                log(args, service)?;
            }
        }
    }
    for service in wait_services {
        log(args, service)?;
    }

    Ok(status)
}

fn service_name(args: &Args, service: &str) -> String {
    format!("{}_{}_1", args.project_name, service)
}

fn wait(args: &Args, service: &str) -> Result<i32> {
    let name = service_name(args, service);
    let output = Command::new(&args.docker).arg("wait").arg(&name).output()?;
    let code: i32 = String::from_utf8_lossy(&output.stdout).trim_end().parse()?;
    Ok(code)
}

fn log(args: &Args, service: &str) -> Result<()> {
    let name = service_name(args, service);
    Command::new(&args.docker)
        .arg("logs")
        .arg(&name)
        .spawn()?
        .wait()?;
    Ok(())
}

fn read_services<'a>(name: &str, contents: &'a str) -> Result<Vec<(&'a str, Yaml<'a>)>> {
    let yaml = minimal_yaml::parse(&contents)?;
    let services = services(name, yaml)?;
    Ok(services)
}

fn services<'a>(name: &str, contents: Yaml<'a>) -> Result<Vec<(&'a str, Yaml<'a>)>> {
    if let Yaml::Mapping(entries) = contents {
        let services = entries.into_iter().find(|entry| {
            if let Yaml::Scalar(s) = entry.key {
                s == "services"
            } else {
                false
            }
        });
        if let Some(services) = services {
            if let Yaml::Mapping(services) = services.value {
                let result: Vec<_> = services
                    .into_iter()
                    .map(|entry| {
                        if let Yaml::Scalar(key) = entry.key {
                            Ok((key, entry.value))
                        } else {
                            Err(Error::Custom(format!("failed to parse file {}", name)))
                        }
                    })
                    .collect::<Result<_>>()?;
                Ok(result)
            } else {
                Err(Error::Custom(format!("failed to parse file {}", name)))
            }
        } else {
            Ok(vec![])
        }
    } else {
        Err(Error::Custom(format!("failed to parse file {}", name)))
    }
}

fn help() {
    println!("Helper to run docker-compose for integration tests
    USAGE:
        docker-compose-test [service..]
    OPTIONS:
        -h, --help\tShow this help message and exit
        -f, file --file file\tSpecify an alternate compose file (default: docker-compose.yml & docker-compose.integration-tests.yml).
         \t\tIt's expected your tests services are declared in the last file.
        -p PROJECT_NAME, --project-name PROJECT_NAME\tSpecify an alternate project name (default: directory name)
        -v, --verbose\tShow all output (default: only shows test output)
    ")
}

struct Args {
    project_name: String,
    verbose: bool,
    files: Vec<String>,
    service_names: Vec<String>,
    docker: String,
    docker_compose: String,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Custom(String),
    #[error("{0}")]
    IoError(#[from] std::io::Error),
    #[error("{0}")]
    ArgError(#[from] pico_args::Error),
    #[error("{0}")]
    YamlError(#[from] YamlParseError),
    #[error("{0}")]
    ParseError(#[from] ParseIntError),
}

pub type Result<T> = std::result::Result<T, Error>;
