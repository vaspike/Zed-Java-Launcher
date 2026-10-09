# 开发态 classpath 仍解析到本地仓库旧 jar

状态：只分析，未改代码。触发场景是 UIS `BusinessApplication`：VS Code 能启动，Zed / java-launcher 不能。

## 结论

应该改。这是启动策略和「改了上游模块就能跑」这个开发循环相反，不是 UIS 特例。

现在每次启动会把入口模块和 `-am` 上游编译进各自的 `target/classes`，但 `java -cp` 里的上游依赖仍是本地仓库里上次 `install` 的 jar。入口模块自己的 `target/classes` 会被插到 classpath 最前面，所以上游一改、入口不改时，只有上游是旧的。

不要把策略改成 `mvn install`。UIS 这种固定版本（`V0.6.1`）不是 `SNAPSHOT`，`install` 会覆盖本机仓库里那份已发布构件，同机器其他检出都会吃到。

## 现场

`SettleAccountsOrderMapper.xml` 的 `resultType` 是：

`com.bestwin.uis.accounting.vo.settle.ListMysettleAccountResponse`

该类在 `bestwinbase`，包已从 `vo.settleaccountsservice` 挪到 `vo.settle`。

| 位置 | 结果 |
| --- | --- |
| `bestwinbase/target/classes`（本次 `test-compile`，2026-10-08 11:36） | 有新类 |
| `~/.m2` 实际路径 `/Users/river/tools/mvn_repo/com/bestwin/uis/bestwinbase/V0.6.1/bestwinbase-V0.6.1.jar`（2025-07-16） | 只有旧包 `vo.settleaccountsservice` |
| 对照 | 当前 `target/classes` 有 20 个 class 不在这份 jar 里；jar 里多出来的就是上述两个旧包 class |

MyBatis 解析 XML 时按字符串 `Class.forName`，于是：

`ClassNotFoundException: com.bestwin.uis.accounting.vo.settle.ListMysettleAccountResponse`

VS Code / IDEA 对 reactor 模块依赖用的是模块输出目录，所以同一份源码能起来。

当时 java-launcher 的 plan 是：

- prepare 1：`mvn -pl business -am test-compile -DskipTests`
- prepare 2：`mvn -pl business -am dependency:3.8.1:build-classpath -DincludeScope=runtime`
- classpath 文件：`~/.local/state/java-launcher/.../36a3687c942c6c16442a.classpath`
- `classes` 只追加了 `business/target/classes`
- 该文件第 92 项就是上面那份 `bestwinbase-V0.6.1.jar`

## 代码在做什么

`src/plan.rs` 的 `build()` 对 Maven 工程：

1. 若 `options.build` 不为 false：`mvn -pl <module> -am test-compile -DskipTests`。注释写明 package 阶段插件不执行。
2. 再起一次独立的 `mvn`，同样 `-pl -am`，目标是 `org.apache.maven.plugins:maven-dependency-plugin:3.8.1:build-classpath`，`-DincludeScope=runtime`，写到 state 目录的 `*.classpath`。
3. 只把入口模块的 `target/classes` 放进 `plan.classes`。
4. `resolve_classpath()` 把 `plan.classes` 放在 classpath 文件内容之前，拼成 `java -cp`。

第二次命令带 `-am` 的注释是对的，要保留这个约束：

> UIS 装进本地仓库的模块 POM，父版本仍是字面量 `${uis.version}`。脱离 reactor、单独再解析一次会失败。

`-am` 解决的是 POM 插值，不是 class 文件从哪来。

## 为什么第二次调用必然落到仓库 jar

本机 Maven 3.9.9（`/opt/homebrew/Cellar/maven/3.9.9/libexec/lib/maven-core-3.9.9.jar`）的 `org.apache.maven.ReactorReader.find` 只有在同一次 Maven 会话里才会把 reactor 构件指到编译输出：

- 这次会话已经走过 `package` / `install` / `deploy`，且 artifact file 存在：用打包产物。
- 还没 package，且是 test 构件，并且走过 `test-compile`：用 `target/test-classes`。
- 还没 package，走过 `compile`，且 artifact 的 `type` 属于 `COMPILE_PHASE_TYPES`：用 `target/classes`。
- 否则返回 null，解析器退回本地仓库。

`COMPILE_PHASE_TYPES` 包含 `jar`（以及 `war`、`ear` 等）。普通 jar 模块在「同一次会话里已经 compile」时会得到 `target/classes`。

现在 compile 和 `build-classpath` 是两次进程。第二次没有 lifecycle，`MavenProject.hasLifecyclePhase("compile")` 为 false，`find()` 返回 null。`BuildClasspathMojo.appendArtifactPath` 写的是 `artifact.getFile()`，也就是仓库里的 jar。

因此：

- 只把上游 `target/classes` **追加**在 jar 后面不够。classpath 先匹配先生效，jar 里还在的旧 class 会盖住目录里的新 class。这次是类挪了包、jar 里没有，所以是 `ClassNotFoundException`；若只改方法体，会变成更难查的旧行为。
- 第一次如果改成 `package`，第二次仍是新会话，`ReactorReader` 不会把上次留下的 `target/*.jar` 当成本次 reactor 产物，还是回退仓库。
- `mvn install` 能让仓库 jar 变新，但会污染固定版本构件，不该做进插件。

## 建议改法

优先在 `resolve_classpath`（或写 classpath 之后、启动之前）做显式替换，保留现在的两次调用和 `-am`：

1. `build-classpath` 的结果只信任为「外部依赖 + 当前误解析到仓库的 reactor 构件」。
2. 对本次 `-am` 真正参与编译的上游模块：若 `target/classes` 存在，用该目录**换掉**对应的仓库 jar，不要只插到前面。
3. 不要把整个 reactor 的兄弟模块都加进来。UIS 里 `accounting`、`warehouse` 等和 `business` 是并列应用，不是它的依赖。
4. `build == false`、这次没有编译时，继续用仓库 jar。不要用一份可能过期的 `target/classes` 去换一份明确的安装产物。
5. 入口模块维持现状即可：它本来就不是自己的依赖，现在已经前置了 `target/classes`。

替换时按 GAV 对上 jar 路径（`.../<artifactId>/<version>/<artifactId>-<version>.jar`），不要用 `starts_with(artifactId)`。`bestwin` 会误伤 `bestwinbase`。版本必须以 Maven 解析后的值为准；原始 POM 里是 `${uis.version}` / `${project.version}`，浅解析对不上。

备选、改动更小：合成一次 Maven 调用，例如

`mvn -pl business -am test-compile dependency:build-classpath ...`

同一次会话里 3.9.9 的 `ReactorReader` 会自己返回 `target/classes`。缺点是绑死这个版本的行为；以后谁再把命令拆开，这个坑会原样回来。显式替换更稳。

## 顺手要记住的第二个坑

`-am` 时每个上游模块都会执行 `build-classpath`，默认覆盖同一个 `-Dmdep.outputFile`。`-pl business -am` 时 `business` 在拓扑序最后，所以这次文件内容碰巧是 `business` 的 runtime classpath。只要最后写入的不是入口模块，整份 classpath 就是别人的。

并行 `-T` 会让写入顺序不确定。现在没传 `-T`。显式替换时要以入口模块那份依赖为准，不能假设文件没被上游覆盖过。

`includeScope=runtime`（runtime + compile，不含 provided / test）对现在这种 `java -cp` 起 Spring Boot 应用是合理的。测试入口走的是 `mvn test`，不要混进这套 classpath。

## 回归

两模块夹具即可，不要依赖真实 UIS：

- A 依赖 B。
- B 新增一个类，不 `install`。
- 启动 A。
- 断言最终 classpath 含 `B/target/classes`，且不含 `B-<version>.jar`。
- 再断言一个只存在于旧 jar、源码已删除的类不会被加载（防止「目录在前、jar 仍在」的半修）。

可用这次 UIS 的现象当手工验收：不 `install`，Zed 启动 `BusinessApplication` 能越过 `SettleAccountsOrderMapper.xml` 对 `ListMysettleAccountResponse` 的解析。
