use std::error::Error;
use std::io::Write;
use std::process::Command;

use assert_cmd::assert::OutputAssertExt;
use assert_cmd::cargo::CommandCargoExt;
use predicates::boolean::PredicateBooleanExt;
use predicates::str::contains;
use serial_test::serial;
use tempfile::NamedTempFile;

#[test]
#[serial]
fn runs_and_outputs_all_integration_test_services() -> Result {
    let mut docker_compose = NamedTempFile::new()?;
    writeln!(
        docker_compose,
        "services:
           db:
             image: docker.io/alpine:3.13.2
             command: sleep 10000
           hello_1:
             image: docker.io/alpine:3.13.2
           hello_2:
             image: docker.io/alpine:3.13.2
        "
    )?;
    let mut integration_tests = NamedTempFile::new()?;
    writeln!(
        integration_tests,
        "services:
           hello_1:
             command: echo \"hello 1\"
           hello_2:
             command: echo \"hello 2\"
        "
    )?;

    let mut cmd = Command::cargo_bin("docker-compose-test")?;
    cmd.arg("-f").arg(docker_compose.path()).arg("-f").arg(integration_tests.path());

    cmd.assert()
        .success()
        .stdout(contains("hello 1"))
        .stdout(contains("hello 2"));

    Ok(())
}

#[test]
#[serial]
fn runs_and_outputs_specific_integration_test_service() -> Result {
    let mut docker_compose = NamedTempFile::new()?;
    writeln!(
        docker_compose,
        "services:
           db:
             image: docker.io/alpine:3.13.2
             command: sleep 10000
           hello_1:
             image: docker.io/alpine:3.13.2
           hello_2:
             image: docker.io/alpine:3.13.2
        "
    )?;
    let mut integration_tests = NamedTempFile::new()?;
    writeln!(
        integration_tests,
        "services:
           hello_1:
             command: echo \"hello 1\"
           hello_2:
             command: echo \"hello 2\"
        "
    )?;

    let mut cmd = Command::cargo_bin("docker-compose-test")?;
    cmd.arg("-f").arg(docker_compose.path()).arg("-f").arg(integration_tests.path())
        .arg("hello_1");

    cmd.assert()
        .success()
        .stdout(contains("hello 1"))
        .stdout(contains("hello 2").not());

    Ok(())
}

#[test]
#[serial]
fn fails_and_outputs_on_test_failure() -> Result {
    let mut docker_compose = NamedTempFile::new()?;
    writeln!(
        docker_compose,
        "services:
           db:
             image: docker.io/alpine:3.13.2
             command: sleep 10000
           hello_1:
             image: docker.io/alpine:3.13.2
           hello_2:
             image: docker.io/alpine:3.13.2
        "
    )?;
    let mut integration_tests = NamedTempFile::new()?;
    writeln!(
        integration_tests,
        "services:
           hello_1:
             command: \"false\"
           hello_2:
             command: echo \"hello 2\"
        "
    )?;

    let mut cmd = Command::cargo_bin("docker-compose-test")?;
    cmd.arg("-f").arg(docker_compose.path()).arg("-f").arg(integration_tests.path()).arg("-v");

    cmd.assert()
        .failure()
        .stdout(contains("hello 2"));

    Ok(())
}

#[test]
#[serial]
fn verbose_includes_other_service_output() -> Result {
    let mut docker_compose = NamedTempFile::new()?;
    writeln!(
        docker_compose,
        "services:
           hello_1:
             image: docker.io/alpine:3.13.2
           hello_2:
             image: docker.io/alpine:3.13.2
           hello_3:
             image: docker.io/alpine:3.13.2
             command: echo \"hello \"\"3\"\"\"
        "
    )?;
    let mut integration_tests = NamedTempFile::new()?;
    writeln!(
        integration_tests,
        "services:
           hello_1:
             command: echo \"hello 1\"
           hello_2:
             command: echo \"hello 2\"
        "
    )?;

    let mut cmd = Command::cargo_bin("docker-compose-test")?;
    cmd.arg("-f").arg(docker_compose.path()).arg("-f").arg(integration_tests.path());
    cmd.arg("-v");

    cmd.assert()
        .success()
        .stdout(contains("hello 1"))
        .stdout(contains("hello 2"))
        .stdout(contains("hello 3"));

    Ok(())
}

type Result = std::result::Result<(), Box<dyn Error>>;