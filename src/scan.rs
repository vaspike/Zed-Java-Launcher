use crate::model::{Entry, EntryKind, Module, Project};
use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
use tree_sitter::{Node, Parser};
use walkdir::WalkDir;

fn text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}
fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut c = node.walk();
    node.named_children(&mut c).collect()
}
fn xml_child<'a, 'i>(node: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
}
fn xml_text(node: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    xml_child(node, name)
        .and_then(|n| n.text())
        .map(|s| s.trim().to_owned())
}
fn relative(root: &Path, path: &Path) -> String {
    let p = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    if p.is_empty() {
        ".".into()
    } else {
        p
    }
}
fn ignored(name: &str) -> bool {
    matches!(
        name,
        "target"
            | "build"
            | "out"
            | "bin"
            | "node_modules"
            | ".git"
            | ".idea"
            | ".java-launcher"
            | ".worktrees"
    )
}

fn maven_modules(
    root: &Path,
    dir: &Path,
    modules: &mut Vec<Module>,
    seen: &mut BTreeSet<PathBuf>,
    warnings: &mut Vec<String>,
    parent_group_id: Option<&str>,
) -> Result<()> {
    let dir = dir
        .canonicalize()
        .with_context(|| format!("Missing module directory {}", dir.display()))?;
    if !dir.starts_with(root) {
        bail!(
            "Module outside project root is not supported: {}",
            dir.display()
        );
    }
    if !seen.insert(dir.clone()) {
        return Ok(());
    }
    let pom = dir.join("pom.xml");
    let source = fs::read_to_string(&pom).with_context(|| format!("Reading {}", pom.display()))?;
    let doc = roxmltree::Document::parse(&source)
        .with_context(|| format!("Parsing {}", pom.display()))?;
    let project = doc.root_element();
    let artifact_id =
        xml_text(project, "artifactId").context("pom.xml lacks project/artifactId")?;
    let mut group_id = xml_text(project, "groupId")
        .or_else(|| xml_child(project, "parent").and_then(|p| xml_text(p, "groupId")))
        .or_else(|| parent_group_id.map(|s| s.to_string()));
    if let Some(ref gid) = group_id {
        if gid.contains("${") {
            group_id = None;
        }
    }
    modules.push(Module {
        path: relative(root, &dir),
        artifact_id,
        packaging: xml_text(project, "packaging").unwrap_or("jar".into()),
        group_id: group_id.clone(),
    });
    if let Some(build) = xml_child(project, "build") {
        for key in ["sourceDirectory", "testSourceDirectory"] {
            if xml_child(build, key).is_some() {
                warnings.push(format!(
                    "{}: custom {key}; v0.1 scans standard src/main/java and src/test/java only",
                    relative(root, &pom)
                ));
            }
        }
    }
    if let Some(profiles) = xml_child(project, "profiles") {
        if profiles
            .children()
            .any(|p| xml_child(p, "modules").is_some())
        {
            warnings.push(format!(
                "{}: profile-activated modules are not evaluated",
                relative(root, &pom)
            ));
        }
    }
    let effective_group_id = group_id.as_deref().or(parent_group_id);
    if let Some(list) = xml_child(project, "modules") {
        for module in list.children().filter(|n| n.has_tag_name("module")) {
            let name = module.text().unwrap_or("").trim();
            if name.is_empty() || name.contains("${") {
                bail!("Unsupported module path in {}: {name}", pom.display());
            }
            maven_modules(root, &dir.join(name), modules, seen, warnings, effective_group_id)?;
        }
    }
    Ok(())
}

pub fn scan(root: &Path) -> Result<Project> {
    let root = root.canonicalize().context("Project root does not exist")?;
    if !root.is_dir() {
        bail!("Project root must be a directory");
    }
    let mut project = Project {
        root: root.clone(),
        modules: vec![],
        entries: vec![],
        warnings: vec![],
    };
    let maven = root.join("pom.xml").is_file();
    if maven {
        maven_modules(
            &root,
            &root,
            &mut project.modules,
            &mut BTreeSet::new(),
            &mut project.warnings,
            None,
        )?;
    } else {
        if [
            "build.gradle",
            "build.gradle.kts",
            "settings.gradle",
            "settings.gradle.kts",
        ]
        .iter()
        .any(|p| root.join(p).exists())
        {
            bail!(
                "Gradle is not supported in v0.1; refusing to treat this as a plain Java project"
            );
        }
        project.modules.push(Module {
            path: ".".into(),
            artifact_id: root
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            packaging: "plain".into(),
            group_id: None,
        });
    }
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_java::LANGUAGE.into())?;
    for module in &project.modules {
        let dir = root.join(&module.path);
        let dirs = if maven {
            vec![dir.join("src/main/java"), dir.join("src/test/java")]
        } else {
            vec![dir]
        };
        for src in dirs.into_iter().filter(|p| p.is_dir()) {
            for file in WalkDir::new(&src)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| !ignored(&e.file_name().to_string_lossy()))
            {
                let file = file?;
                if !file.file_type().is_file()
                    || file.path().extension().is_none_or(|s| s != "java")
                {
                    continue;
                }
                let source = fs::read_to_string(file.path())
                    .with_context(|| format!("Reading {}", file.path().display()))?;
                let tree = parser
                    .parse(&source, None)
                    .context("Java parser returned no tree")?;
                let path = relative(&root, file.path());
                if tree.root_node().has_error() {
                    project
                        .warnings
                        .push(format!("{path}: syntax errors; entries may be incomplete"));
                }
                project
                    .entries
                    .extend(analyze(tree.root_node(), &source, module, &path));
            }
        }
    }
    project.entries.sort_by(|a, b| a.id.cmp(&b.id));
    let mut ids = BTreeSet::new();
    for entry in &project.entries {
        if !ids.insert(&entry.id) {
            bail!("Duplicate entry {} (duplicate source/class or overloaded test); resolve before generating launch configs", entry.id);
        }
    }
    project.modules.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(project)
}

fn annotations(node: Node<'_>, source: &str) -> Vec<String> {
    children(node)
        .into_iter()
        .find(|n| n.kind() == "modifiers")
        .map(|mods| {
            children(mods)
                .into_iter()
                .filter(|n| matches!(n.kind(), "annotation" | "marker_annotation"))
                .filter_map(|a| a.child_by_field_name("name"))
                .map(|n| text(n, source).rsplit('.').next().unwrap_or("").to_owned())
                .collect()
        })
        .unwrap_or_default()
}
fn has_modifier(node: Node<'_>, source: &str, modifier: &str) -> bool {
    children(node)
        .into_iter()
        .find(|n| n.kind() == "modifiers")
        .is_some_and(|mods| {
            let mut cursor = mods.walk();
            let found = mods
                .children(&mut cursor)
                .any(|n| text(n, source) == modifier);
            found
        })
}
fn is_main(node: Node<'_>, source: &str) -> bool {
    if node
        .child_by_field_name("name")
        .is_none_or(|n| text(n, source) != "main")
    {
        return false;
    }
    if node
        .child_by_field_name("type")
        .is_none_or(|n| text(n, source) != "void")
    {
        return false;
    }
    if !has_modifier(node, source, "public") || !has_modifier(node, source, "static") {
        return false;
    }
    let Some(params) = node.child_by_field_name("parameters") else {
        return false;
    };
    let ps: Vec<_> = children(params)
        .into_iter()
        .filter(|n| matches!(n.kind(), "formal_parameter" | "spread_parameter"))
        .collect();
    if ps.len() != 1 {
        return false;
    }
    let p = ps[0];
    // AST nodes exclude comments; normalize only the type, not the full declaration.
    if p.kind() == "spread_parameter" {
        return children(p).iter().any(|n| {
            matches!(n.kind(), "type_identifier" | "scoped_type_identifier")
                && matches!(text(*n, source), "String" | "java.lang.String")
        });
    }
    let Some(ty) = p.child_by_field_name("type") else {
        return false;
    };
    let ty: String = text(ty, source)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    matches!(ty.as_str(), "String[]" | "java.lang.String[]")
        || (matches!(ty.as_str(), "String" | "java.lang.String")
            && p.child_by_field_name("dimensions").is_some())
}

pub fn analyze(root: Node<'_>, source: &str, module: &Module, file: &str) -> Vec<Entry> {
    let package = children(root)
        .into_iter()
        .find(|n| n.kind() == "package_declaration")
        .and_then(|n| {
            children(n)
                .into_iter()
                .find(|n| matches!(n.kind(), "identifier" | "scoped_identifier"))
        })
        .map(|n| text(n, source).to_owned())
        .unwrap_or_default();
    let mut entries = vec![];
    for class in children(root).into_iter().filter(|n| {
        matches!(
            n.kind(),
            "class_declaration" | "enum_declaration" | "record_declaration"
        )
    }) {
        visit_class(class, source, module, file, &package, None, &mut entries);
    }
    entries
}

fn visit_class(
    node: Node<'_>,
    source: &str,
    module: &Module,
    file: &str,
    package: &str,
    outer: Option<&str>,
    entries: &mut Vec<Entry>,
) {
    let Some(name) = node.child_by_field_name("name") else {
        return;
    };
    let simple = text(name, source);
    let class = if let Some(outer) = outer {
        format!("{outer}${simple}")
    } else if package.is_empty() {
        simple.to_owned()
    } else {
        format!("{package}.{simple}")
    };
    let Some(body) = node.child_by_field_name("body") else {
        return;
    };
    let methods: Vec<_> = children(body)
        .into_iter()
        .filter(|n| n.kind() == "method_declaration")
        .collect();
    let abstract_class = has_modifier(node, source, "abstract");
    let new_entry = |kind, method: Option<String>, line| {
        let suffix = match kind {
            EntryKind::TestClass => "@test".to_owned(),
            EntryKind::TestMethod => format!("#{}", method.as_deref().unwrap_or_default()),
            _ => String::new(),
        };
        Entry {
            id: format!("{}::{class}{suffix}", module.path),
            kind,
            module: module.path.clone(),
            project_name: module.artifact_id.clone(),
            class: class.clone(),
            method,
            file: file.to_owned(),
            line,
        }
    };
    if let Some(main) = methods.iter().find(|n| is_main(**n, source)) {
        let kind = if annotations(node, source)
            .iter()
            .any(|a| a == "SpringBootApplication")
        {
            EntryKind::SpringBoot
        } else {
            EntryKind::Main
        };
        entries.push(new_entry(kind, None, main.start_position().row + 1));
    }
    let tests: Vec<_> = methods
        .iter()
        .filter(|m| {
            annotations(**m, source).iter().any(|a| {
                matches!(
                    a.as_str(),
                    "Test" | "ParameterizedTest" | "RepeatedTest" | "TestFactory" | "TestTemplate"
                )
            })
        })
        .collect();
    if !tests.is_empty() && !abstract_class {
        entries.push(new_entry(
            EntryKind::TestClass,
            None,
            node.start_position().row + 1,
        ));
        let mut seen = BTreeSet::new();
        for test in tests {
            if let Some(name) = test.child_by_field_name("name") {
                let name = text(name, source).to_owned();
                // Overloads share a test selector, so expose just one method entry.
                if seen.insert(name.clone()) {
                    entries.push(new_entry(
                        EntryKind::TestMethod,
                        Some(name),
                        test.start_position().row + 1,
                    ));
                }
            }
        }
    }
    for nested in children(body).into_iter().filter(|n| {
        matches!(
            n.kind(),
            "class_declaration" | "enum_declaration" | "record_declaration"
        )
    }) {
        visit_class(nested, source, module, file, package, Some(&class), entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(s: &str) -> Vec<Entry> {
        let mut p = Parser::new();
        p.set_language(&tree_sitter_java::LANGUAGE.into()).unwrap();
        let t = p.parse(s, None).unwrap();
        analyze(
            t.root_node(),
            s,
            &Module {
                path: "api".into(),
                artifact_id: "api".into(),
                packaging: "jar".into(),
                group_id: None,
            },
            "App.java",
        )
    }
    #[test]
    fn main_variants_and_comments() {
        for params in [
            "String[] args",
            "String ... args",
            "java.lang.String[] args",
            "String args[]",
        ] {
            let s = format!("package demo; @SpringBootApplication public class App {{ public static void\nmain({params}) {{}} }}");
            let es = parse(&s);
            assert_eq!(es.len(), 1, "{params}");
            assert_eq!(es[0].kind, EntryKind::SpringBoot);
        }
        assert!(parse(
            "// public static void main(String[] args) {}\nclass App { String s = \"@Test\"; }"
        )
        .is_empty());
        assert!(parse("class App { public static int main(String[] a){return 0;} }").is_empty());
    }
    #[test]
    fn tests_and_nested_classes() {
        let es = parse("package demo; class AppTest { @ParameterizedTest @ValueSource(strings={\"a\"}) void works(String s) {} @Nested class Inner { @org.junit.jupiter.api.Test void ok() {} } }");
        assert_eq!(es.len(), 4);
        assert!(es.iter().any(|e| e.id == "api::demo.AppTest$Inner#ok"));
    }
    #[test]
    fn reactor_excludes_unlisted_modules_and_parent_artifact() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("pom.xml"), "<project><parent><artifactId>parent</artifactId></parent><artifactId>root</artifactId><modules><module>api</module></modules></project>").unwrap();
        fs::create_dir_all(root.join("api/src/main/java")).unwrap();
        fs::create_dir_all(root.join("old/src/main/java")).unwrap();
        fs::write(
            root.join("api/pom.xml"),
            "<project><artifactId>api</artifactId></project>",
        )
        .unwrap();
        for folder in ["api", "old"] {
            fs::write(
                root.join(folder).join("src/main/java/App.java"),
                "class App { public static void main(String[] args) {} }",
            )
            .unwrap();
        }
        let p = scan(root).unwrap();
        assert_eq!(p.modules.len(), 2);
        assert_eq!(p.entries.len(), 1);
        assert_eq!(p.entries[0].module, "api");
    }
}
