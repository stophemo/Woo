//! 日志初始化：控制台 + 数据目录下的 rolling 文件。
//!
//! 桌面端从 Finder / 开始菜单启动时没有可见的终端，`env_logger` 的输出会直接丢弃，
//! 用户遇到“登录失败/同步失败”时无法提供任何线索。这里在同一条日志管道上再挂一个
//! 文件 sink（`<数据目录>/logs/woo.log`，超过 2MB 自动轮转），排查问题只需要让用户
//! 提供这个文件。

use log::{Log, Metadata, Record};
use once_cell::sync::Lazy;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
const LOG_FILE_NAME: &str = "woo.log";

struct FileSink {
    path: Mutex<Option<PathBuf>>,
    file: Mutex<Option<File>>,
}

static FILE_SINK: Lazy<FileSink> = Lazy::new(|| FileSink {
    path: Mutex::new(None),
    file: Mutex::new(None),
});

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 指向数据目录下的日志文件；应在解析出数据目录后调用一次。
/// 在此之前发生的日志只写到控制台。
pub fn set_log_dir(data_dir: &Path) {
    let log_dir = data_dir.join("logs");
    if let Err(error) = std::fs::create_dir_all(&log_dir) {
        // 此时日志还没落盘，只能尽力而为。
        eprintln!("[Woo] 无法创建日志目录 {:?}: {}", log_dir, error);
        return;
    }

    {
        let mut path = lock(&FILE_SINK.path);
        *path = Some(log_dir.join(LOG_FILE_NAME));
    }
    // 让下一次写入时按新路径重新打开（同时完成轮转检查）。
    *lock(&FILE_SINK.file) = None;
}

pub fn log_file_path() -> Option<PathBuf> {
    lock(&FILE_SINK.path).clone()
}

fn open_rotating(path: &Path) -> Option<File> {
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > MAX_LOG_BYTES {
            let rotated = path.with_extension("log.1");
            let _ = std::fs::remove_file(&rotated);
            if let Err(error) = std::fs::rename(path, &rotated) {
                eprintln!("[Woo] 日志轮转失败 {:?}: {}", path, error);
            }
        }
    }

    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| {
            eprintln!("[Woo] 无法打开日志文件 {:?}: {}", path, error);
            error
        })
        .ok()
}

fn write_to_file(record: &Record) {
    let path = match lock(&FILE_SINK.path).clone() {
        Some(path) => path,
        None => return,
    };

    // 锁顺序固定为 先 path（已释放）后 file，避免与 set_log_dir 交叉持锁。
    let mut file_guard = lock(&FILE_SINK.file);
    if file_guard.is_none() {
        *file_guard = open_rotating(&path);
    }
    let Some(file) = file_guard.as_mut() else {
        return;
    };

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!(
        "[{}] [{}] [{}] {}\n",
        timestamp,
        record.level(),
        record.target(),
        record.args()
    );
    if let Err(error) = file.write_all(line.as_bytes()) {
        eprintln!("[Woo] 写日志失败: {}", error);
        return;
    }
    let _ = file.flush();
}

struct TeeLogger {
    console: env_logger::Logger,
}

impl Log for TeeLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.console.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        self.console.log(record);
        write_to_file(record);
    }

    fn flush(&self) {
        self.console.flush();
        if let Some(file) = lock(&FILE_SINK.file).as_mut() {
            let _ = file.flush();
        }
    }
}

/// 安装全局 logger。重复调用是安全的（只有第一次生效）。
pub fn init() {
    let console = env_logger::Builder::from_env(
        env_logger::Env::default().filter_or("WOO_LOG", "info"),
    )
    .format_timestamp_secs()
    .build();

    let max_level = console.filter();
    // 先装 logger 再设全局最大级别，避免被 set_boxed_logger 的默认值覆盖。
    if log::set_boxed_logger(Box::new(TeeLogger { console })).is_ok() {
        log::set_max_level(max_level);
        log::info!("[Log] Logger initialized");
    }
}
