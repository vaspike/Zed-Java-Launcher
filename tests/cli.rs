use serde_json::{json, Value};
use std::{
    fs,
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};

struct Fixture {
    dir: tempfile::TempDir,
    state: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
            state: tempfile::tempdir().unwrap(),
        }
    }
    fn root(&self) -> &Path {
        self.dir.path()
    }
    fn command(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_java-launcher"));
        c.arg("--project")
            .arg(self.root())
            .env("JAVA_LAUNCHER_STATE_DIR", self.state.path());
        c
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
    fn ok(&self, args: &[&str]) -> Value {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap_or(Value::Null)
    }
    fn write(&self, name: &str, content: &str) {
        let p = self.root().join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }
    fn source(&self, folder: &str, class: &str) {
        self.write(&format!("{folder}/src/main/java/demo/{class}.java"), &format!("package demo; @SpringBootApplication public class {class} {{ public static void main(String[] args) {{}} }}"));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.run(&["stop", "--all"]);
    }
}

#[test]
fn import_and_sync_preserve_profile_and_user_configs() {
    let f = Fixture::new();
    f.write("pom.xml","<project><parent><artifactId>parent</artifactId></parent><artifactId>root</artifactId><modules><module>api</module></modules></project>");
    f.write(
        "api/pom.xml",
        "<project><artifactId>api</artifactId></project>",
    );
    f.source("api", "App");
    f.write(".vscode/launch.json", r#"{ // JSONC
      "configurations":[{"type":"java","request":"launch","name":"API","projectName":"api","mainClass":"demo.App","vmArgs":"-Xmx512m -Dspring.profiles.active=local","args":"--title 'hello world'"}]
    }"#);
    f.write(
        ".vscode/aggregated-launch.json",
        r#"{"configs":[{"name":"all","items":[{"name":"API","delay":100}]}]}"#,
    );
    f.ok(&["init", "--from-vscode", "--write"]);
    let p = f.root().join(".java-launcher/config.json");
    let c: Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
    assert_eq!(c["entries"]["api::demo.App"]["spring_profile"], "local");
    assert_eq!(
        c["entries"]["api::demo.App"]["args"],
        json!(["--title", "hello world"])
    );
    assert_eq!(c["groups"]["all"][0]["entry"], "api::demo.App");
    f.ok(&["profile", "api", "test"]);
    f.ok(&["init", "--from-vscode", "--write"]);
    let after: Value = serde_json::from_slice(&fs::read(p).unwrap()).unwrap();
    assert_eq!(after["entries"]["api::demo.App"]["spring_profile"], "test");
    f.write(
        ".zed/tasks.json",
        "[// keep backup of this comment\n{\"label\":\"My task\",\"command\":\"echo ok\"}]",
    );
    f.ok(&["sync-zed", "--write"]);
    let tasks = f.root().join(".zed/tasks.json");
    let first = fs::read(&tasks).unwrap();
    f.ok(&["sync-zed", "--write"]);
    assert_eq!(first, fs::read(&tasks).unwrap());
    let ts: Value = serde_json::from_slice(&first).unwrap();
    assert!(ts
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["label"] == "My task"));
    let ds: Value =
        serde_json::from_slice(&fs::read(f.root().join(".zed/debug.json")).unwrap()).unwrap();
    assert!(ds
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["vmArgs"] == json!(["-Xmx512m", "-Dspring.profiles.active=test"])));
    assert!(ts
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["label"] == "[Java Launcher] Open"));
    assert!(!ts
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["label"].as_str().unwrap_or("").contains("Run api")));
    assert!(f.root().join(".zed/.java-launcher-backups").exists());
}

#[test]
fn rejects_ambiguous_modules_and_does_not_publish_plain_scripts_by_default() {
    let f = Fixture::new();
    f.write("pom.xml","<project><artifactId>root</artifactId><modules><module>a</module><module>b</module></modules></project>");
    for module in ["a", "b"] {
        f.write(
            &format!("{module}/pom.xml"),
            &format!("<project><artifactId>{module}</artifactId></project>"),
        );
        f.source(module, "App");
    }
    let out = f.run(&["plan", "demo.App"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("Ambiguous"));
    f.write(
        "a/src/main/java/demo/DangerousMigration.java",
        "package demo; public class DangerousMigration { public static void main(String[] a) {} }",
    );
    let generated = f.ok(&["sync-zed"]);
    assert!(!generated["tasks"]
        .to_string()
        .contains("DangerousMigration"));
    assert!(!f.root().join(".zed").exists());
    assert!(!f.root().join(".java-launcher").exists());
}

#[test]
fn invalid_debug_file_does_not_partially_replace_tasks() {
    let f = Fixture::new();
    f.write(
        "App.java",
        "class App { public static void main(String[] a) {} }",
    );
    let tasks = "[{\"label\":\"mine\",\"command\":\"true\"}]";
    f.write(".zed/tasks.json", tasks);
    f.write(".zed/debug.json", "[invalid");
    assert!(!f
        .run(&["sync-zed", "--include-main", "--write"])
        .status
        .success());
    assert_eq!(
        fs::read_to_string(f.root().join(".zed/tasks.json")).unwrap(),
        tasks
    );
}

#[test]
fn plans_do_not_execute_builds_and_keep_maven_profiles_separate() {
    let f = Fixture::new();
    f.write(
        "pom.xml",
        "<project><artifactId>root</artifactId></project>",
    );
    f.source(".", "App");
    f.write(".java-launcher/config.json",r#"{"defaults":{"spring_profile":"local","maven_profiles":["dev"],"maven_args":["-Dmaven.antrun.skip=true"],"args":["space value","$(touch hacked)"],"vm_args":["-Xmx256m"]}}"#);
    let plan = f.ok(&["plan", "App"]);
    assert!(plan["prepare"][0]["args"]
        .as_array()
        .unwrap()
        .contains(&json!("-Pdev")));
    assert!(plan["prepare"][0]["args"]
        .as_array()
        .unwrap()
        .contains(&json!("-Dmaven.antrun.skip=true")));
    assert!(plan["launch"]["args"]
        .as_array()
        .unwrap()
        .contains(&json!("-Dspring.profiles.active=local")));
    assert!(plan["launch"]["args"]
        .as_array()
        .unwrap()
        .contains(&json!("$(touch hacked)")));
    assert!(!f.root().join("target").exists());
    assert!(!f.root().join("hacked").exists());
}

// Opt-in real JDK test, no Maven dependencies or external services needed.
#[test]
#[ignore = "requires a local JDK; run cargo test -- --ignored"]
fn real_jvm_group_lifecycle_releases_ports_and_preserves_unrelated_process() {
    let f = Fixture::new();
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/plain/src/demo");
    for name in ["Alpha.java", "Beta.java", "DemoServer.java"] {
        f.write(
            &format!("src/demo/{name}"),
            &fs::read_to_string(sample.join(name)).unwrap(),
        );
    }
    f.write(
        ".java-launcher/config.json",
        r#"{"groups":{"demo":[{"entry":".::demo.Alpha"},{"entry":".::demo.Beta","delay_ms":10}]}}"#,
    );
    let mut unrelated = Command::new("sleep").arg("60").spawn().unwrap();
    struct Cleanup(std::process::Child);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    // Move ownership to cleanup guard even when assertions fail.
    let _ = &mut unrelated;
    let mut unrelated = Cleanup(unrelated);
    f.ok(&["group", "up", "demo"]);
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut ports = vec![];
    while Instant::now() < deadline {
        ports.clear();
        let statuses = f.ok(&["ps"]);
        for status in statuses.as_array().unwrap() {
            if let Some(log) = status["log"].as_str() {
                let contents = fs::read_to_string(log).unwrap_or_default();
                if let Some(line) = contents.lines().rev().find(|l| l.starts_with("READY ")) {
                    ports.push(
                        line.split_whitespace()
                            .last()
                            .unwrap()
                            .parse::<u16>()
                            .unwrap(),
                    );
                }
            }
        }
        if ports.len() == 2 {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(ports.len(), 2, "services failed to become ready");
    for port in &ports {
        assert!(TcpStream::connect(("127.0.0.1", *port)).is_ok());
    }
    assert!(
        !f.run(&["start", "Alpha"]).status.success(),
        "duplicate start must fail"
    );
    f.ok(&["group", "down", "demo"]);
    assert_eq!(f.ok(&["ps"]), json!([]));
    for port in &ports {
        assert!(
            TcpListener::bind(("127.0.0.1", *port)).is_ok(),
            "port {port} leaked"
        );
    }
    assert!(
        unrelated.0.try_wait().unwrap().is_none(),
        "unrelated process was killed"
    );
    f.ok(&["start", "Alpha"]);
    let before = f.ok(&["ps"])[0]["supervisor_pid"].clone();
    f.ok(&["restart", "Alpha"]);
    let after = f.ok(&["ps"])[0]["supervisor_pid"].clone();
    assert_ne!(before, after);
    f.ok(&["stop", "--all"]);
}

#[test]
fn watched_pid_termination_cancels_service() {
    let f = Fixture::new();
    f.write(
        "pom.xml",
        r#"<project><modelVersion>4.0.0</modelVersion><groupId>g</groupId><artifactId>root</artifactId><version>1</version><packaging>pom</packaging><modules><module>app</module></modules></project>"#,
    );
    f.write(
        "app/pom.xml",
        r#"<project><modelVersion>4.0.0</modelVersion><groupId>g</groupId><artifactId>app</artifactId><version>1</version></project>"#,
    );
    f.source("app", "App");

    let mut dummy = Command::new("sleep").arg("10").spawn().unwrap();
    let dummy_pid = dummy.id();

    let status = f.ok(&["start", "App", "--skip-build", "--watch-pid", &dummy_pid.to_string()]);
    assert_eq!(status["entry"], "app::demo.App");
    assert_eq!(status["watch_pid"], dummy_pid);

    let ps = f.ok(&["ps"]);
    assert_eq!(ps.as_array().unwrap().len(), 1);

    dummy.kill().unwrap();
    let _ = dummy.wait();

    let deadline = Instant::now() + Duration::from_secs(4);
    let mut stopped = false;
    while Instant::now() < deadline {
        let ps = f.ok(&["ps"]);
        if ps.as_array().unwrap().is_empty() {
            stopped = true;
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(stopped, "service did not terminate after watched process exited");
}

#[test]
fn terminate_on_zed_quit_respects_config() {
    let f = Fixture::new();
    f.write(
        "pom.xml",
        r#"<project><modelVersion>4.0.0</modelVersion><groupId>g</groupId><artifactId>root</artifactId><version>1</version><packaging>pom</packaging><modules><module>app</module></modules></project>"#,
    );
    f.write(
        "app/pom.xml",
        r#"<project><modelVersion>4.0.0</modelVersion><groupId>g</groupId><artifactId>app</artifactId><version>1</version></project>"#,
    );
    f.source("app", "App");
    f.write(
        ".java-launcher/config.json",
        r#"{"defaults":{"terminate_on_zed_quit":false}}"#,
    );

    let status = f.ok(&["start", "App", "--skip-build"]);
    assert_eq!(status["entry"], "app::demo.App");
    assert!(status["watch_pid"].is_null());
}
