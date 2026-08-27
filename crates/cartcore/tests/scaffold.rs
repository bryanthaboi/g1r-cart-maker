//! New Cart must produce exactly what `cartkit scaffold` produces.

use cartcore::scaffold::{engine_range, scaffold_into, ScaffoldOptions};
use cartcore::workflow::{render, stamped_cart_id, WorkflowOptions};
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scaffold")
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures().join(name)).unwrap_or_else(|_| panic!("fixture {}", name))
}

fn demo_options() -> ScaffoldOptions {
    let engine = fixture("engine_version.txt");
    ScaffoldOptions {
        title: Some("Demo Cart".into()),
        author: Some("someone".into()),
        summary: Some("A demo cart.".into()),
        base: "gold".into(),
        shell: Some("#2f6f4f".into()),
        seal: "sealed+".into(),
        github: Some("someone/demo-cart".into()),
        engine: engine.trim().to_string(),
        ..ScaffoldOptions::new("demo_cart")
    }
}

#[test]
fn scaffold_matches_cartkit() {
    let dir = tempdir::TempDir::new("cartcore-scaffold").expect("temp dir");
    let dest = dir.path().join("demo_cart");
    scaffold_into(&dest, &demo_options()).expect("scaffold");

    for name in ["cart.json", "README.md", "CHANGELOG.md"] {
        let ours = std::fs::read_to_string(dest.join(name)).expect("written");
        assert_eq!(ours, fixture(name), "{} differs from cartkit", name);
    }
    let ours = std::fs::read(dest.join("label.png")).expect("label art");
    assert_eq!(
        ours,
        std::fs::read(fixtures().join("label.png")).expect("fixture"),
        "placeholder art differs from cartkit"
    );
    let ours =
        std::fs::read_to_string(dest.join(".github/workflows/release.yml")).expect("workflow");
    assert_eq!(
        ours,
        fixture("release.yml"),
        "workflow differs from cartkit"
    );
    assert!(dest.join(".gitignore").is_file());
}

#[test]
fn scaffold_refuses_an_existing_directory() {
    let dir = tempdir::TempDir::new("cartcore-scaffold").expect("temp dir");
    let dest = dir.path().join("demo_cart");
    scaffold_into(&dest, &demo_options()).expect("scaffold");
    assert!(scaffold_into(&dest, &demo_options()).is_err());
    let forced = ScaffoldOptions {
        force: true,
        ..demo_options()
    };
    assert!(scaffold_into(&dest, &forced).is_ok());
}

#[test]
fn scaffold_rejects_bad_input() {
    for bad in ["no spaces", "", &"x".repeat(65)] {
        assert!(scaffold_into(Path::new("/nonexistent"), &ScaffoldOptions::new(bad)).is_err());
    }
    let bad_shell = ScaffoldOptions {
        shell: Some("8b1a1a".into()),
        ..ScaffoldOptions::new("ok")
    };
    assert!(scaffold_into(Path::new("/nonexistent"), &bad_shell).is_err());
    let bad_base = ScaffoldOptions {
        base: "nonesuch".into(),
        ..ScaffoldOptions::new("ok")
    };
    assert!(scaffold_into(Path::new("/nonexistent"), &bad_base).is_err());
    let bad_github = ScaffoldOptions {
        github: Some("not a repo!".into()),
        ..ScaffoldOptions::new("ok")
    };
    assert!(scaffold_into(Path::new("/nonexistent"), &bad_github).is_err());
}

#[test]
fn engine_ranges_follow_the_major() {
    assert_eq!(engine_range("0.0.0-dev"), ">=0.0.0-dev <1.0.0");
    assert_eq!(engine_range("1.4.2"), ">=1.4.2 <2.0.0");
    assert_eq!(engine_range("12.0.0"), ">=12.0.0 <13.0.0");
}

#[test]
fn workflow_keeps_its_load_bearing_steps() {
    let body = render(&WorkflowOptions::new("demo_cart"));
    let parsed = yaml_rust2::YamlLoader::load_from_str(&body).expect("workflow yaml parses");
    let doc = &parsed[0];
    assert_eq!(doc["env"]["CART_ID"].as_str(), Some("demo_cart"));
    assert_eq!(stamped_cart_id(&body).as_deref(), Some("demo_cart"));

    let steps = doc["jobs"]["release"]["steps"]
        .as_vec()
        .expect("release steps");
    let names: Vec<&str> = steps
        .iter()
        .filter_map(|step| step["name"].as_str())
        .collect();
    assert!(names.contains(&"Check the tag against cart.json"));
    assert!(names.contains(&"Validate every pin"));
    assert!(names.contains(&"Pack the cart"));
    assert!(body.contains("validate . --online --strict"));
    assert!(body.contains("selftest --quiet"));
    assert!(body.contains("does not match cart.json version"));
    assert!(body.contains("sha256sums.txt"));
    assert_eq!(doc["permissions"]["contents"].as_str(), Some("write"));
    assert_eq!(
        doc["concurrency"]["cancel-in-progress"].as_bool(),
        Some(false)
    );

    let retargeted = render(&WorkflowOptions {
        cartkit_ref: "v1.2.3".into(),
        ..WorkflowOptions::new("demo_cart")
    });
    assert!(retargeted.contains("CARTKIT_REF: v1.2.3"));
}
