#[cfg_attr(test, mockall::automock)]
pub trait Watcher {
    fn watch(&mut self, path: &std::path::Path) -> Result<std::sync::mpsc::Receiver<()>, Error>;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not found: `{0}`")]
    NotFound(String),
    #[error("internal: `{0}`")]
    Internal(String),
}

pub struct WatcherImpl {
    watcher: Option<notify::RecommendedWatcher>,
}

impl WatcherImpl {
    pub fn new() -> WatcherImpl {
        WatcherImpl { watcher: None }
    }
}

impl Watcher for WatcherImpl {
    fn watch(&mut self, path: &std::path::Path) -> Result<std::sync::mpsc::Receiver<()>, Error> {
        use notify::Watcher;

        let (tx, rx) = std::sync::mpsc::channel();
        let watcher = notify::recommended_watcher(move |res| match res {
            Ok(_) => {
                let _ = tx.send(());
            }
            Err(err) => {
                log::error!("Error: {err:?}");
            }
        });
        let mut watcher = match watcher {
            Ok(watcher) => Ok(watcher),
            Err(err) => return Err(Error::Internal(err.to_string())),
        }?;
        match watcher.watch(path, notify::RecursiveMode::Recursive) {
            Ok(_) => Ok(()),
            Err(err) => match err {
                notify::Error {
                    kind: notify::ErrorKind::PathNotFound,
                    ..
                } => Err(Error::NotFound(err.to_string())),
                _ => Err(Error::Internal(err.to_string())),
            },
        }?;
        self.watcher = Some(watcher);
        return Ok(rx);
    }
}

/// mpsc::Receiver のメッセージを連続で受信した場合に、最後のメッセージのみを受信するようにリレーする。
/// 連続で受信したと判定する間隔は dur で指定する。
pub fn relay_latest<Msg: Send + 'static>(
    rx: std::sync::mpsc::Receiver<Msg>,
    dur: std::time::Duration,
) -> std::sync::mpsc::Receiver<Msg> {
    let (tx_new, rx_new) = std::sync::mpsc::channel::<Msg>();
    std::thread::spawn(move || {
        let mut last_msg;

        'outer: loop {
            match rx.recv() {
                Ok(msg) => {
                    last_msg = Some(msg);
                    'inner: loop {
                        match rx.recv_timeout(dur) {
                            Ok(msg) => {
                                last_msg = Some(msg);
                                continue 'inner;
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                if let Some(msg) = last_msg {
                                    let _ = tx_new.send(msg);
                                }
                                break 'inner;
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                break 'outer;
                            }
                        }
                    }
                }
                Err(_) => {
                    break;
                }
            }
        }
    });
    rx_new
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_latest() {
        let (tx, rx) = std::sync::mpsc::channel::<i32>();
        let rx = relay_latest(rx, std::time::Duration::from_millis(10));

        // メッセージがリレーされる
        tx.send(1).unwrap();
        assert_eq!(rx.recv(), Ok(1));

        // メッセージが連続で送信された場合、最後のメッセージのみが受信される
        tx.send(2).unwrap();
        tx.send(3).unwrap();
        tx.send(4).unwrap();
        assert_eq!(rx.recv(), Ok(4));

        // 送信側が drop された場合、メッセージが残っていても受信されない
        tx.send(5).unwrap();
        drop(tx);
        assert_eq!(rx.recv(), Err(std::sync::mpsc::RecvError));
    }
}
