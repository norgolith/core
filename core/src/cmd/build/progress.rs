use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

pub fn make_bar(mp: &MultiProgress, len: u64, msg: &str) -> ProgressBar {
    let bar = mp.add(ProgressBar::new(len));
    bar.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg} [{bar:30.cyan/blue}] {pos}/{len}")
            .unwrap()
            .progress_chars("=> "),
    );
    bar.enable_steady_tick(std::time::Duration::from_millis(120));
    bar.set_message(msg.to_string());
    bar
}

pub fn make_spinner(mp: &MultiProgress, msg: &str) -> ProgressBar {
    let bar = mp.add(ProgressBar::new_spinner());
    bar.set_style(ProgressStyle::with_template("{spinner:.green} {msg}").unwrap());
    bar.enable_steady_tick(std::time::Duration::from_millis(120));
    bar.set_message(msg.to_string());
    bar
}
