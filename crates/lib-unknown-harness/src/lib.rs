// SPDX-License-Identifier: MPL-2.0
//! 动态编译运行 harness（仅开发期使用）：将代码片段生成临时 Cargo 工程并编译执行，支持超时、批量与缓存。
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Child;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use lib_unknown_errors::{ErrType, Result, bail, err};

/// 获取动态测试的根目录 `target/dyn_tests`。
fn base_dyn_tests_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap()
        .join("target")
        .join("dyn_tests")
}

/// 单次动态测试的结果，`Display` 输出彩色摘要（超长输出自动截断）。
///
/// # Examples
///
/// ```rust,no_run
/// use lib_unknown_harness::{BatchRunner, Task};
///
/// let task = Task::new("hello", r#"fn main() { println!("hi"); }"#);
/// let results = BatchRunner::new().run([task]);
/// assert_eq!(results.len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct RunnerResult {
    /// 被测程序标准输出全文。
    pub stdout: String,
    /// 被测程序标准错误全文。
    pub stderr: String,
    /// 被测程序退出码；系统级失败时为 `-2`，默认构造时为 `-1`。
    pub exit_code: i32,
    /// 编译耗时。
    pub build_duration: Duration,
    /// 运行耗时。
    pub run_duration: Duration,
    /// 被测程序是否以退出码 0 正常结束。
    pub ok: bool,
    /// 系统级错误；`None` 表示无系统级错误。
    ///
    /// 当其为 `None` 且 [`RunnerResult::ok`] 为 `false` 时，表示被测程序自身返回了非零退出码。
    pub system_err: Option<lib_unknown_errors::Error>,
}

impl Default for RunnerResult {
    fn default() -> Self {
        Self {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: -1,
            build_duration: Duration::ZERO,
            run_duration: Duration::ZERO,
            ok: false,
            system_err: None,
        }
    }
}

fn truncate_output<'a>(text: &'a str, max_lines: usize) -> String {
    const MAX_LINE_CHARS: usize = 300;
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }
    let truncate_line = |line: &'a str| -> Cow<'a, str> {
        // 快路径：总字节数 <= MAX_LINE_CHARS 时，UTF-8 字符数必然 <= MAX_LINE_CHARS，零拷贝返回
        if line.len() <= MAX_LINE_CHARS {
            return Cow::Borrowed(line);
        }
        let half = MAX_LINE_CHARS / 2;
        let mut chars = line.chars();
        let head: String = chars.by_ref().take(half).collect();
        let tail_rev: Vec<char> = chars.by_ref().rev().take(half).collect();
        // 若双向截取后迭代器已空，说明总字符数 <= MAX_LINE_CHARS，无需截断
        if chars.next().is_none() {
            return Cow::Borrowed(line);
        }
        let tail: String = tail_rev.into_iter().rev().collect();
        Cow::Owned(format!("{head} ... [单行超长截断] ... {tail}"))
    };
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max_lines {
        return lines
            .into_iter()
            .map(truncate_line)
            .collect::<Vec<_>>()
            .join("\n");
    }
    let half = max_lines / 2;
    let head = lines[..half]
        .iter()
        .copied()
        .map(truncate_line)
        .collect::<Vec<_>>()
        .join("\n");
    let tail = lines[lines.len() - half..]
        .iter()
        .copied()
        .map(truncate_line)
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{}\n\n... [已截断 {} 行] ...\n\n{}",
        head,
        lines.len() - max_lines,
        tail
    )
}

impl fmt::Display for RunnerResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let green = "\x1b[32m";
        let red = "\x1b[31m";
        let yellow = "\x1b[33m";
        let cyan = "\x1b[36m";
        let reset = "\x1b[0m";
        let (status_color, status_text) = match &self.system_err {
            None if self.ok => (green, "SUCCESS"),
            None => (red, "FAILED (Runtime Error)"),
            Some(e) => match e.err_type() {
                ErrType::Timeout(_) => (yellow, "TIMEOUT (TLE)"),
                ErrType::CompileFailed => (red, "COMPILE ERROR (CE)"),
                ErrType::CargoFailed => (red, "CARGO ERROR (CE)"),
                ErrType::ProcessSpawnFailed => (red, "SYSTEM ERROR"),
                _ => (red, "SYSTEM ERROR"),
            },
        };
        writeln!(f, "{status_color}=== [Result: {}] ==={reset}", status_text)?;
        writeln!(
            f,
            "> Build: {cyan}{:?}{reset} | Run: {cyan}{:?}{reset}",
            self.build_duration, self.run_duration
        )?;
        writeln!(f, "> Exit Code: {status_color}{}{reset}", self.exit_code)?;
        let stdout_trimmed = self.stdout.trim();
        if !stdout_trimmed.is_empty() {
            writeln!(
                f,
                "{}--- Stdout (size: {} bytes) ---{}",
                yellow,
                self.stdout.len(),
                reset
            )?;
            writeln!(f, "{}", truncate_output(stdout_trimmed, 20))?;
        }
        let clean_stderr = self
            .stderr
            .lines()
            .filter(|l| !l.trim().starts_with("Compiling "))
            .filter(|l| !l.trim().starts_with("Finished "))
            .collect::<Vec<_>>()
            .join("\n");
        let stderr_trimmed = clean_stderr.trim();
        if !stderr_trimmed.is_empty() {
            writeln!(
                f,
                "{}--- Stderr (size: {} bytes) ---{}",
                red,
                clean_stderr.len(),
                reset
            )?;
            writeln!(f, "{}", truncate_output(stderr_trimmed, 40))?;
        }
        writeln!(f, "{status_color}=========================={reset}")?;
        Ok(())
    }
}

/// 单个动态测试任务：`main.rs` 内容、依赖与运行配置的 Builder。
///
/// # Examples
///
/// ```rust
/// use lib_unknown_harness::Task;
///
/// let task = Task::new("hello", r#"fn main() { println!("hi"); }"#)
///     .with_deps(r#"serde = "1""#)
///     .with_env("RUST_BACKTRACE", "1")
///     .with_release(false);
/// assert_eq!(task.tag, "hello");
/// ```
#[derive(Clone)]
pub struct Task {
    /// 任务标识，用于结果回调用例区分；`From` 元组缺省时为 `"unnamed_task"`。
    pub tag: String,
    /// 待编译运行的 `main.rs` 源码。
    pub main_code: String,
    /// 追加到临时工程 `[dependencies]` 的依赖声明片段。
    pub deps: String,
    /// 注入子进程的环境变量。
    pub envs: HashMap<String, String>,
    /// 写入临时工程 `.cargo/config.toml` 的配置，无则为 `None`。
    pub cargo_config: Option<String>,
    /// 是否以 `--release` 编译运行。
    pub is_release: bool,
}

impl Task {
    /// 以标识与 `main.rs` 源码创建任务，其余配置取默认值。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use lib_unknown_harness::Task;
    ///
    /// let task = Task::new("t", "fn main() {}");
    /// assert_eq!(task.tag, "t");
    /// ```
    pub fn new(tag: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            main_code: code.into(),
            deps: String::new(),
            envs: HashMap::new(),
            cargo_config: None,
            is_release: false,
        }
    }

    /// 设置 `[dependencies]` 依赖声明片段。
    pub fn with_deps(mut self, deps: impl Into<String>) -> Self {
        self.deps = deps.into();
        self
    }

    /// 注入单个环境变量。
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.envs.insert(key.into(), value.into());
        self
    }

    /// 批量注入环境变量。
    pub fn with_envs<I, K, V>(mut self, envs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        for (k, v) in envs {
            self.envs.insert(k.into(), v.into());
        }
        self
    }

    /// 设置写入临时工程 `.cargo/config.toml` 的配置。
    pub fn with_cargo_config(mut self, config: impl Into<String>) -> Self {
        self.cargo_config = Some(config.into());
        self
    }

    /// 开启或关闭 release 模式。
    pub fn with_release(mut self, release: bool) -> Self {
        self.is_release = release;
        self
    }
}

impl From<(String, String, Option<String>)> for Task {
    fn from(tuple: (String, String, Option<String>)) -> Self {
        Self {
            main_code: tuple.0,
            deps: tuple.1,
            tag: tuple.2.unwrap_or_else(|| "unnamed_task".to_string()),
            envs: HashMap::new(),
            cargo_config: None,
            is_release: false,
        }
    }
}

/// 批量运行器：串行执行多个 [`Task`]，可选超时、仅编译、依赖缓存与逐任务回调。
///
/// # Examples
///
/// ```rust,no_run
/// use lib_unknown_harness::{BatchRunner, Task};
///
/// let tasks = [Task::new("a", "fn main() {}"), Task::new("b", "fn main() {}")];
/// let results = BatchRunner::new().only_build(true).run(tasks);
/// assert_eq!(results.len(), 2);
/// ```
pub struct BatchRunner {
    timeout: Option<Duration>,
    only_build: bool,
    use_cache: bool,
    on_result: Option<Box<dyn FnMut(String, RunnerResult)>>,
}

impl Default for BatchRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl BatchRunner {
    /// 以默认配置（无超时、编译并运行、不缓存、无回调）创建运行器。
    pub fn new() -> Self {
        Self {
            timeout: None,
            only_build: false,
            use_cache: false,
            on_result: None,
        }
    }

    /// 设置单个任务编译 + 运行的总超时，超时后子进程被终止，结果记为 [`ErrType::Timeout`]。
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = Some(d);
        self
    }

    /// 仅编译不运行。
    pub fn only_build(mut self, b: bool) -> Self {
        self.only_build = b;
        self
    }

    /// 启用首任务依赖预热缓存（先空跑一次编译，后续任务复用编译缓存）。
    pub fn use_cache(mut self, c: bool) -> Self {
        self.use_cache = c;
        self
    }

    /// 设置每个任务完成后的回调（参数为任务标识与结果）。
    pub fn on_result<F>(mut self, f: F) -> Self
    where
        F: FnMut(String, RunnerResult) + 'static,
    {
        self.on_result = Some(Box::new(f));
        self
    }

    pub(crate) fn with_boxed_callback(
        mut self,
        cb: Option<Box<dyn FnMut(String, RunnerResult)>>,
    ) -> Self {
        self.on_result = cb;
        self
    }

    /// 串行执行全部任务，返回按输入顺序的 `(标识, 结果)` 列表；空输入直接返回空列表。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lib_unknown_harness::{BatchRunner, Task};
    ///
    /// let results = BatchRunner::new().run([Task::new("a", "fn main() {}")]);
    /// assert!(results[0].1.ok);
    /// ```
    pub fn run<I, T>(mut self, tasks: I) -> Vec<(String, RunnerResult)>
    where
        I: IntoIterator<Item = T>,
        T: Into<Task>,
    {
        let tasks: Vec<Task> = tasks.into_iter().map(Into::into).collect();
        if tasks.is_empty() {
            return vec![];
        }
        if self.use_cache {
            self.run_with_cache(tasks)
        } else {
            self.run_standard(tasks)
        }
    }

    fn resolve_tag(tag: String, index: usize) -> String {
        if tag == "unnamed_task" {
            format!("task_{}", index)
        } else {
            tag
        }
    }

    fn execute_with_runner(&self, runner: &Runner) -> RunnerResult {
        if self.only_build {
            runner.build(self.timeout)
        } else {
            runner.run(self.timeout)
        }
    }

    fn record_result(
        &mut self,
        results: &mut Vec<(String, RunnerResult)>,
        tag: String,
        res: RunnerResult,
    ) {
        if let Some(ref mut callback) = self.on_result {
            callback(tag.clone(), res.clone());
        }
        results.push((tag, res));
    }

    fn run_standard(&mut self, tasks: Vec<Task>) -> Vec<(String, RunnerResult)> {
        let mut results = Vec::with_capacity(tasks.len());
        for (index, task) in tasks.into_iter().enumerate() {
            let tag = Self::resolve_tag(task.tag, index);
            let mut runner = Runner::new(&task.main_code, &task.deps);
            runner.envs(task.envs);
            runner.release(task.is_release);
            if let Some(config) = task.cargo_config {
                runner.cargo_config(config);
            }
            let res = self.execute_with_runner(&runner);
            self.record_result(&mut results, tag, res);
        }
        results
    }

    fn run_with_cache(&mut self, tasks: Vec<Task>) -> Vec<(String, RunnerResult)> {
        let mut results = Vec::with_capacity(tasks.len() + 1);
        let first_deps = &tasks[0].deps;
        let mut runner = Runner::new("fn main() {}", first_deps);
        runner.release(tasks[0].is_release);
        if let Some(config) = &tasks[0].cargo_config {
            runner.cargo_config(config.clone());
        }
        let init_res = runner.build(self.timeout);
        self.record_result(&mut results, "init_cache".to_string(), init_res);

        for (index, task) in tasks.into_iter().enumerate() {
            let tag = Self::resolve_tag(task.tag, index);
            runner.reset_deps(&task.deps);
            runner.reset_main_code(&task.main_code);
            runner.clear_envs();
            runner.envs(task.envs);
            runner.release(task.is_release);
            if let Some(config) = task.cargo_config {
                runner.cargo_config(config);
            } else {
                runner.clear_cargo_config();
            }
            let res = self.execute_with_runner(&runner);
            self.record_result(&mut results, tag, res);
        }
        results
    }
}

/// 运行单个动态测试任务的便捷入口（等价于单任务 `BatchRunner`）。
///
/// # Examples
///
/// ```rust,no_run
/// use lib_unknown_harness::run;
///
/// let res = run("fn main() {}", "", None, true);
/// assert!(res.ok);
/// ```
pub fn run(
    main_code: &str,
    deps: &str,
    timeout: Option<Duration>,
    only_build: bool,
) -> RunnerResult {
    let mut runner = Runner::new(main_code, deps);
    runner.release(true);
    if only_build {
        runner.build(timeout)
    } else {
        runner.run(timeout)
    }
}

/// 批量运行的便捷入口（不使用依赖预热缓存）。
///
/// # Examples
///
/// ```rust,no_run
/// use lib_unknown_harness::{Task, run_batch};
///
/// let results = run_batch([Task::new("a", "fn main() {}")], None, None, None);
/// assert_eq!(results.len(), 1);
/// ```
pub fn run_batch<I, T>(
    task: I,
    task_timeout: Option<Duration>,
    only_build: Option<bool>,
    on_result: Option<Box<dyn FnMut(String, RunnerResult)>>,
) -> Vec<(String, RunnerResult)>
where
    I: IntoIterator<Item = T>,
    T: Into<Task>,
{
    let mut runner = BatchRunner::new()
        .only_build(only_build.unwrap_or(false))
        .use_cache(false)
        .with_boxed_callback(on_result);
    if let Some(t) = task_timeout {
        runner = runner.timeout(t);
    }
    runner.run(task)
}

/// 批量运行的便捷入口（启用首任务依赖预热缓存，首个结果为 `"init_cache"`）。
///
/// # Examples
///
/// ```rust,no_run
/// use lib_unknown_harness::{Task, run_batch_use_cache};
///
/// let results = run_batch_use_cache([Task::new("a", "fn main() {}")], None, None, None);
/// assert_eq!(results[0].0, "init_cache");
/// ```
pub fn run_batch_use_cache<I, T>(
    task: I,
    task_timeout: Option<Duration>,
    only_build: Option<bool>,
    on_result: Option<Box<dyn FnMut(String, RunnerResult)>>,
) -> Vec<(String, RunnerResult)>
where
    I: IntoIterator<Item = T>,
    T: Into<Task>,
{
    let mut runner = BatchRunner::new()
        .only_build(only_build.unwrap_or(false))
        .use_cache(true)
        .with_boxed_callback(on_result);
    if let Some(t) = task_timeout {
        runner = runner.timeout(t);
    }
    runner.run(task)
}

/// 单个临时 Cargo 工程的句柄：在 `target/dyn_tests/test_<id>/` 下生成工程并编译执行，`Drop` 时释放心跳锁。
pub struct Runner {
    /// 待编译运行的 `main.rs` 源码。
    pub main_code: String,
    /// `[dependencies]` 依赖声明片段。
    pub deps: String,
    dir: PathBuf,
    project_name: String,
    /// 心跳锁守卫：仅通过 RAII 在 `Drop` 时释放，故字段不被直接读取。
    _lock: TestLock,
    envs: HashMap<String, String>,
    cargo_config: Option<String>,
    /// 是否以 `--release` 编译运行。
    pub is_release: bool,
}

impl Runner {
    /// 在 `target/dyn_tests` 下创建以纳秒时间戳 + 随机数命名的临时工程并同步文件。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lib_unknown_harness::Runner;
    ///
    /// let runner = Runner::new("fn main() {}", "");
    /// let res = runner.build(None);
    /// assert!(res.ok);
    /// ```
    ///
    /// # Panics
    ///
    /// - 系统时钟早于 UNIX 纪元、无法获取当前目录或工程目录/文件创建失败时 panic。
    pub fn new(main_code: &str, deps: &str) -> Self {
        static C: AtomicUsize = AtomicUsize::new(0);
        let c = C.fetch_add(1, Ordering::Relaxed);
        let id = format!(
            "{}_{}_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            std::process::id(),
            c
        );
        let project_name = format!("dyn_test_{}", id);
        let dir = base_dyn_tests_dir().join(format!("test_{}", id));
        let src_dir = dir.join("src");
        fs::create_dir_all(&src_dir).expect("无法创建共享测试目录");
        let instance = Self {
            main_code: main_code.to_string(),
            deps: deps.to_string(),
            dir,
            project_name: project_name.clone(),
            _lock: TestLock::new(&project_name),
            envs: HashMap::new(),
            cargo_config: None,
            is_release: false,
        };
        instance.sync_files();
        instance
    }

    /// 设置是否以 `--release` 编译运行。
    pub fn release(&mut self, release: bool) -> &mut Self {
        self.is_release = release;
        self
    }

    /// 注入单个子进程环境变量。
    pub fn env(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.envs.insert(key.into(), value.into());
        self
    }

    /// 批量注入子进程环境变量。
    pub fn envs<I, K, V>(&mut self, envs: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        for (k, v) in envs {
            self.envs.insert(k.into(), v.into());
        }
        self
    }

    /// 清空已注入的环境变量。
    pub fn clear_envs(&mut self) -> &mut Self {
        self.envs.clear();
        self
    }

    /// 设置 `.cargo/config.toml` 内容并立即同步到工程目录。
    pub fn cargo_config(&mut self, config: impl Into<String>) -> &mut Self {
        let new_config = config.into();
        if self.cargo_config.as_deref() != Some(new_config.as_str()) {
            self.cargo_config = Some(new_config);
            self.sync_cargo_config();
        }
        self
    }

    /// 清除 cargo 配置（内存值置空并删除已写入的配置文件，忽略删除失败）。
    pub fn clear_cargo_config(&mut self) -> &mut Self {
        if self.cargo_config.take().is_some() {
            let config_path = self.dir.join(".cargo").join("config.toml");
            if config_path.exists() {
                let _ = fs::remove_file(config_path);
            }
        }
        self
    }

    fn sync_cargo_toml(&self) {
        // 显式声明空的 [workspace] 表，防止父级目录的 Cargo Workspace 捕获此临时工程
        let cargo_toml = format!(
            r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"
[workspace]
[dependencies]
{}
"#,
            self.project_name, self.deps
        );
        fs::write(self.dir.join("Cargo.toml"), cargo_toml).expect("写入 Cargo.toml 失败");
    }

    fn sync_main_rs(&self) {
        fs::write(self.dir.join("src").join("main.rs"), &self.main_code)
            .expect("写入 main.rs 失败");
    }

    fn sync_cargo_config(&self) {
        if let Some(config_content) = &self.cargo_config {
            let cargo_dir = self.dir.join(".cargo");
            if !cargo_dir.exists() {
                fs::create_dir_all(&cargo_dir).expect("无法创建 .cargo 目录");
            }
            fs::write(cargo_dir.join("config.toml"), config_content)
                .expect("写入 .cargo/config.toml 失败");
        }
    }

    /// 将当前 `Cargo.toml` / `main.rs` / cargo 配置同步到工程目录。
    ///
    /// # Panics
    ///
    /// - 文件写入失败（磁盘不可写、权限不足）时 panic。
    pub fn sync_files(&self) {
        self.sync_cargo_toml();
        self.sync_main_rs();
        self.sync_cargo_config();
    }

    /// 判断环境变量是否属于父级 `cargo test` / `cargo build` 注入的易污染变量。
    fn is_polluting_parent_env(key: &str) -> bool {
        key.starts_with("CARGO_PKG_")
            || key.starts_with("CARGO_BIN_")
            || key.starts_with("CARGO_CRATE_")
            || key.starts_with("CARGO_FEATURES_")
            || key.starts_with("CARGO_CFG_")
            || matches!(
                key,
                "CARGO_MANIFEST_DIR"
                    | "CARGO_MANIFEST_PATH"
                    | "CARGO_MAKEFLAGS"
                    | "CARGO_PRIMARY_PACKAGE"
                    | "CARGO_BUILD_TARGET"
                    | "CARGO_BUILD_TARGET_DIR"
                    | "CARGO_ENCODED_RUSTFLAGS"
                    | "RUSTFLAGS"
                    | "CARGO_ENCODED_RUSTDOCFLAGS"
                    | "RUSTDOCFLAGS"
                    | "OUT_DIR"
                    | "DEPS_DIR"
            )
    }

    /// 为子进程应用环境隔离：清理宿主 Cargo 的污染变量，并注入用户配置的 `self.envs`。
    fn configure_isolated_cmd(&self, cmd: &mut Command) {
        cmd.current_dir(&self.dir);
        for (k, _) in std::env::vars_os() {
            if let Some(k_str) = k.to_str()
                && Self::is_polluting_parent_env(k_str)
            {
                cmd.env_remove(&k);
            }
        }
        cmd.envs(&self.envs);
    }

    /// 构造已隔离环境的 `cargo` 命令（优先沿用父进程的 `CARGO` 工具链路径）。
    fn new_cargo_cmd(&self) -> Command {
        let cargo_bin = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut cmd = Command::new(cargo_bin);
        self.configure_isolated_cmd(&mut cmd);
        cmd
    }

    fn execute_cmd(cmd: &mut Command, timeout: Option<Duration>) -> Result<(Output, Duration)> {
        let start_time = Instant::now();
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        let mut child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| err!(ErrType::ProcessSpawnFailed, "{}", e))?;
        let mut stdout_pipe = child.stdout.take().expect("无法获取 stdout");
        let mut stderr_pipe = child.stderr.take().expect("无法获取 stderr");
        let stdout_handle = thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stdout_pipe.read_to_end(&mut buf);
            buf
        });
        let stderr_handle = thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stderr_pipe.read_to_end(&mut buf);
            buf
        });
        match timeout {
            None => {
                let status = child
                    .wait()
                    .map_err(|e| err!(ErrType::ProcessSpawnFailed, "{}", e))?;
                let stdout = stdout_handle.join().unwrap_or_default();
                let stderr = stderr_handle.join().unwrap_or_default();
                Ok((
                    Output {
                        status,
                        stdout,
                        stderr,
                    },
                    start_time.elapsed(),
                ))
            }
            Some(limit) => {
                let mut sleep_dur = Duration::from_micros(500);
                const MAX_SLEEP: Duration = Duration::from_millis(10);
                loop {
                    if let Ok(Some(status)) = child.try_wait() {
                        let stdout = stdout_handle.join().unwrap_or_default();
                        let stderr = stderr_handle.join().unwrap_or_default();
                        return Ok((
                            Output {
                                status,
                                stdout,
                                stderr,
                            },
                            start_time.elapsed(),
                        ));
                    }
                    let elapsed = start_time.elapsed();
                    if elapsed >= limit {
                        kill_process_tree(&mut child);
                        let _ = child.wait();
                        let _ = stdout_handle.join();
                        let _ = stderr_handle.join();
                        bail!(ErrType::Timeout(limit));
                    }
                    let remaining = limit - elapsed;
                    thread::sleep(sleep_dur.min(remaining));
                    sleep_dur = (sleep_dur * 2).min(MAX_SLEEP);
                }
            }
        }
    }

    fn fill_result_from_cmd(
        result: &mut RunnerResult,
        cmd_res: Result<(Output, Duration)>,
        is_build: bool,
        fail_err_type: Option<ErrType>,
    ) {
        match cmd_res {
            Ok((output, duration)) => {
                if is_build {
                    result.build_duration = duration;
                } else {
                    result.run_duration = duration;
                }
                result.stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                result.stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                result.exit_code = output.status.code().unwrap_or(-1);
                result.ok = output.status.success();
                if !result.ok
                    && let Some(err_type) = fail_err_type
                {
                    result.system_err = Some(err!(err_type));
                }
            }
            Err(sys_err) => {
                result.system_err = Some(sys_err);
                result.exit_code = -2;
            }
        }
    }

    /// 执行 `cargo build`（`is_release` 为真时加 `--release`），超时则终止并记为 [`ErrType::Timeout`]；自身不 panic，失败均体现在返回的 [`RunnerResult`] 中。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lib_unknown_harness::Runner;
    ///
    /// let res = Runner::new("fn main() {}", "").build(None);
    /// assert!(res.ok);
    /// ```
    pub fn build(&self, timeout: Option<Duration>) -> RunnerResult {
        let mut result = RunnerResult::default();
        let mut cmd = self.new_cargo_cmd();
        cmd.arg("build").arg("-q");
        if self.is_release {
            cmd.arg("--release");
        }
        Self::fill_result_from_cmd(
            &mut result,
            Self::execute_cmd(&mut cmd, timeout),
            true,
            Some(ErrType::CompileFailed),
        );
        result
    }

    /// 先编译后直接运行产物；编译失败时直接返回编译结果，不启动进程。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lib_unknown_harness::Runner;
    ///
    /// let res = Runner::new(r#"fn main() { println!("hi"); }"#, "").run(None);
    /// assert!(res.ok);
    /// ```
    pub fn run(&self, timeout: Option<Duration>) -> RunnerResult {
        let build_result = self.build(timeout);
        if !build_result.ok {
            return build_result;
        }
        // 扣除编译阶段已消耗的时间，确保编译 + 运行总时长不超过 timeout
        let remaining_timeout =
            timeout.map(|limit| limit.saturating_sub(build_result.build_duration));
        self.run_no_build(remaining_timeout, Some(build_result))
    }

    /// 计算当前配置下产物可执行文件的预期路径（不保证文件已存在）。
    ///
    /// 路径由以下因素决定（优先级从高到低）：
    /// - 环境变量 `CARGO_TARGET_DIR`（优先读取注入的 `self.envs`，其次回退至当前进程环境变量，相对路径基于工程目录解析）；
    /// - `.cargo/config.toml` 的 `[build]` 段：`target-dir` 指定输出根目录
    ///   （相对路径基于工程目录解析），`target` 指定交叉编译三元组
    ///   （输出目录为 `<工程>/target/<三元组>`）；
    /// - 默认输出目录 `<工程>/target`；
    /// - [`is_release`](Self::is_release) 决定使用 `release` 还是 `debug` profile 子目录；
    /// - 当前运行平台决定可执行文件后缀（Windows 为 `.exe`，否则无后缀）。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lib_unknown_harness::Runner;
    ///
    /// let runner = Runner::new("fn main() {}", "");
    /// let build_res = runner.build(None);
    /// assert!(build_res.ok);
    ///
    /// let path = runner.bin_path();
    /// assert!(path.exists(), "产物路径应在编译成功后存在: {}", path.display());
    /// ```
    pub fn bin_path(&self) -> PathBuf {
        let mut target_dir = self.dir.join("target");
        let env_target_dir = self
            .envs
            .get("CARGO_TARGET_DIR")
            .cloned()
            .or_else(|| std::env::var("CARGO_TARGET_DIR").ok())
            .filter(|s| !s.is_empty());
        // Cargo 的优先级：CARGO_TARGET_DIR 高于 config.toml 的 [build] 配置。
        if let Some(env_dir) = env_target_dir {
            let path = PathBuf::from(env_dir);
            target_dir = if path.is_absolute() {
                path
            } else {
                self.dir.join(path)
            };
        } else if let Some(cfg_str) = &self.cargo_config {
            let mut in_build_section = false;
            let mut cfg_target_dir: Option<PathBuf> = None;
            let mut cfg_target_triple: Option<&str> = None;
            for line in cfg_str.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if line.starts_with('[') {
                    in_build_section = line == "[build]";
                    continue;
                }
                if !in_build_section {
                    continue;
                }
                let Some((key, val)) = line.split_once('=') else {
                    continue;
                };
                let key = key.trim();
                if key != "target" && key != "target-dir" {
                    continue;
                }
                let val_no_comment = val.split('#').next().unwrap_or(val);
                let value = val_no_comment
                    .trim()
                    .trim_matches(|c| c == '"' || c == '\'');
                if value.is_empty() {
                    continue;
                }
                if key == "target-dir" {
                    let path = PathBuf::from(value);
                    cfg_target_dir = Some(if path.is_absolute() {
                        path
                    } else {
                        self.dir.join(path)
                    });
                } else {
                    // `build.target` 是交叉编译三元组，输出位于 `<target-dir>/<triple>`。
                    cfg_target_triple = Some(value);
                }
            }
            if let Some(dir) = cfg_target_dir {
                target_dir = dir;
            }
            if let Some(triple) = cfg_target_triple {
                target_dir = target_dir.join(triple);
            }
        }
        let exe_name = if cfg!(windows) {
            format!("{}.exe", self.project_name)
        } else {
            self.project_name.clone()
        };
        let profile_dir = if self.is_release { "release" } else { "debug" };
        target_dir.join(profile_dir).join(exe_name)
    }

    /// 不重新编译，直接运行已有产物（产物缺失时返回 [`ErrType::ProcessSpawnFailed`] 结果）。
    /// `build_result` 用于透传编译耗时，无则置零。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lib_unknown_harness::Runner;
    ///
    /// let runner = Runner::new("fn main() {}", "");
    /// let build_res = runner.build(None);
    /// assert!(build_res.ok);
    ///
    /// let res = runner.run_no_build(None, None);
    /// assert!(res.ok);
    /// ```
    pub fn run_no_build(
        &self,
        timeout: Option<Duration>,
        build_result: Option<RunnerResult>,
    ) -> RunnerResult {
        let bin_path = self.bin_path();
        let build_result = build_result.unwrap_or_default();
        let mut result = RunnerResult {
            build_duration: build_result.build_duration,
            ..Default::default()
        };
        if !bin_path.exists() {
            result.system_err = Some(err!(
                ErrType::ProcessSpawnFailed,
                "测试可执行文件未找到。\n预期路径: {}\n请检查编译是否真的成功，或者 target/profile 逻辑是否正确。",
                bin_path.display()
            ));
            result.exit_code = -2;
            return result;
        }
        let mut cmd = Command::new(&bin_path);
        self.configure_isolated_cmd(&mut cmd);
        Self::fill_result_from_cmd(
            &mut result,
            Self::execute_cmd(&mut cmd, timeout),
            false,
            None,
        );
        result
    }

    /// 在工程目录下执行自定义 `cargo <args>` 命令（如 `cargo test`、`cargo check`、
    /// `cargo clippy` 等），超时则终止并记为 [`ErrType::Timeout`]；
    /// 自身不 panic，失败均体现在返回的 [`RunnerResult`] 中。
    ///
    /// 耗时记为 [`RunnerResult::run_duration`]，[`RunnerResult::build_duration`] 恒为 `Duration::ZERO`。
    ///
    /// # Note: 不透传 `is_release`
    ///
    /// 与 [`build`](Self::build) 不同，本方法**不会**根据 `is_release` 自动追加
    /// `--release`，`args` 将原样透传给 `cargo`。需要 release 行为时请调用者显式传入：
    /// `runner.cargo(&["test", "--release"], None)`。
    ///
    /// # 错误语义
    ///
    /// - `cargo` 进程启动失败 → [`ErrType::ProcessSpawnFailed`]；
    /// - 超时被终止 → [`ErrType::Timeout`]；
    /// - 正常退出但退出码非零（如 `check`/`test` 未通过）→ [`ErrType::CargoFailed`]，
    ///   此时 [`RunnerResult::ok`] 为 `false`，`stdout`/`stderr`/`exit_code` 保留子进程输出供诊断；
    /// - 退出码为 0 → `ok == true`，`system_err.is_none()`。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use lib_unknown_harness::Runner;
    ///
    /// let runner = Runner::new("fn main() {}", "");
    /// let res = runner.cargo(&["check"], None);
    /// assert!(res.ok);
    ///
    /// // release 行为不会自动追加，需显式传入：
    /// let res = runner.cargo(&["test", "--release"], None);
    /// assert!(res.ok);
    /// ```
    pub fn cargo(&self, args: &[&str], timeout: Option<Duration>) -> RunnerResult {
        let mut result = RunnerResult::default();
        let mut cmd = self.new_cargo_cmd();
        cmd.args(args);
        Self::fill_result_from_cmd(
            &mut result,
            Self::execute_cmd(&mut cmd, timeout),
            false,
            Some(ErrType::CargoFailed),
        );
        result
    }

    /// 替换 `main.rs` 内容并按需同步到工程目录（供缓存复用时复写任务代码，仅更新 `main.rs` 不触碰 `Cargo.toml`）。
    ///
    /// # Panics
    ///
    /// - 文件写入失败时 panic。
    pub fn reset_main_code(&mut self, main_code: &str) {
        if self.main_code != main_code {
            self.main_code.clear();
            self.main_code.push_str(main_code);
            self.sync_main_rs();
        }
    }

    /// 替换依赖声明并按需同步到工程目录（依赖未变化时不重写 `Cargo.toml`，保护增量编译缓存）。
    ///
    /// # Panics
    ///
    /// - 文件写入失败时 panic。
    pub fn reset_deps(&mut self, deps: &str) {
        if self.deps != deps {
            self.deps.clear();
            self.deps.push_str(deps);
            self.sync_cargo_toml();
        }
    }
}

/// 工程心跳锁：在 `target/dyn_tests/lock/` 下创建锁文件并由后台线程每 500ms 续写，`Drop` 时停线程删文件，用于识别僵尸工程。
pub struct TestLock {
    lock_path: PathBuf,
    stop_signal: Arc<AtomicBool>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl TestLock {
    /// 创建锁文件并启动心跳线程。
    ///
    /// # Panics
    ///
    /// - 无法获取当前目录、锁目录/锁文件创建失败，或心跳线程无法启动时 panic。
    pub fn new(id: &str) -> Self {
        let lock_dir = base_dyn_tests_dir().join("lock");
        fs::create_dir_all(&lock_dir).expect("无法创建锁目录");
        let lock_file = lock_dir.join(format!("lock_{}", id));
        let mut initial_file = fs::File::create(&lock_file).expect("无法创建锁文件");
        let _ = write!(initial_file, "heartbeat: 0");
        drop(initial_file);

        let stop_signal = Arc::new(AtomicBool::new(false));
        let thread_stop_signal = Arc::clone(&stop_signal);
        let thread_lock_file = lock_file.clone();
        let thread_handle = thread::spawn(move || {
            let mut counter = 1u64;
            let heartbeat_interval = Duration::from_millis(500);
            while !thread_stop_signal.load(Ordering::Acquire) {
                thread::park_timeout(heartbeat_interval);
                if thread_stop_signal.load(Ordering::Acquire) {
                    break;
                }
                if let Ok(mut file) = fs::File::create(&thread_lock_file) {
                    let _ = write!(file, "heartbeat: {}", counter);
                }
                counter += 1;
            }
        });
        Self {
            lock_path: lock_file,
            stop_signal,
            thread_handle: Some(thread_handle),
        }
    }
}

impl Drop for TestLock {
    fn drop(&mut self) {
        self.stop_signal.store(true, Ordering::Release);
        if let Some(handle) = self.thread_handle.take() {
            // 立即唤醒 park_timeout 中的心跳线程，避免主线程阻塞等待睡眠周期结束
            handle.thread().unpark();
            let _ = handle.join();
        }
        if self.lock_path.exists() {
            let _ = fs::remove_file(&self.lock_path);
        }
    }
}

/// 清理全部动态测试工程：等待活跃锁释放（心跳停滞超 1500ms 的视为僵尸并清除其锁），再删除 `target/dyn_tests` 目录。
///
/// # Examples
///
/// ```rust,no_run
/// use lib_unknown_harness::clear_project;
///
/// clear_project(None);
/// ```
///
/// # Panics
///
/// - 无法获取当前目录时 panic；超时仅打印警告，不 panic。
pub fn clear_project(timeout: Option<Duration>) {
    let base_dir = base_dyn_tests_dir();
    let lock_dir = base_dir.join("lock");
    let start_time = Instant::now();
    let zombie_threshold = Duration::from_millis(1500);
    if lock_dir.exists() {
        loop {
            let mut active_locks = 0;
            if let Ok(entries) = fs::read_dir(&lock_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    let is_zombie = match fs::metadata(&path).and_then(|m| m.modified()) {
                        Ok(modified_time) => {
                            modified_time.elapsed().unwrap_or(Duration::ZERO) > zombie_threshold
                        }
                        Err(_) => true,
                    };
                    if is_zombie {
                        let _ = fs::remove_file(&path);
                    } else {
                        active_locks += 1;
                    }
                }
            }
            if active_locks == 0 {
                break;
            }
            if let Some(limit) = timeout
                && start_time.elapsed() >= limit
            {
                eprintln!("警告: 清除动态测试环境超时！");
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
    if base_dir.exists() {
        let _ = fs::remove_dir_all(base_dir);
    }
}

/// 跨平台强杀整棵进程树
fn kill_process_tree(child: &mut Child) {
    #[cfg(any(unix, windows))]
    let pid = child.id();
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
        }
        if let Ok(pgid) = i32::try_from(pid) {
            unsafe {
                let _ = kill(-pgid, 9);
            }
        }
        let _ = child.kill();
    }

    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = child.kill();
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = child.kill();
    }
}
